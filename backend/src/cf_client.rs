use rand::seq::SliceRandom;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::state::DifficultyMode;

// ---------------------------------------------------------------------------
// Cache type aliases
// ---------------------------------------------------------------------------

/// Type alias for the contest-problem cache (keyed by contest_id)
type ProblemCache = Arc<Mutex<HashMap<i32, (std::time::Instant, Vec<ContestProblem>)>>>;

// ---------------------------------------------------------------------------
// Static problem database – loaded once at startup from embedded JSON
// ---------------------------------------------------------------------------

/// Difficulty bands for Band mode.  `difficulty` field is one of these ids.
/// Ranges are **clist.by** ratings (inclusive on both ends), NOT CF ratings.
pub const BANDS: &[(u8, &str, u32, u32)] = &[
    (0, "Super Easy", 0,    300),
    (1, "Easy",       301,  600),
    (2, "Medium",     601,  1000),
    (3, "Hard",       1001, 1500),
    (4, "Very Hard",  1501, 9999),
];

/// A pre-scraped Codeforces problem from `backend/data/problems.json`.
/// Short field names match the JSON keys produced by `scripts/build_problem_db.py`.
#[derive(Debug, Deserialize, Clone)]
pub struct StaticProblem {
    /// contestId
    #[serde(rename = "c")]
    pub contest_id: i32,
    /// problem index (e.g. "A", "B", "C1")
    #[serde(rename = "i")]
    pub index: String,
    /// problem name
    #[serde(rename = "n")]
    pub name: String,
    /// CF rating (e.g. 800, 1500, 2400)
    #[serde(rename = "r")]
    pub rating: u32,
    /// solve count from CF problemStatistics
    #[serde(rename = "s")]
    pub solved_count: u64,
    /// division tag: Div1/Div2/Div3/Div4/Educational/Global/Other
    #[serde(rename = "d")]
    pub division: String,
    /// band id 0-4 based on clist rating (-1 if unclassified)
    #[serde(rename = "b")]
    pub band: i8,
    /// clist.by rating (-1 if unavailable)
    #[serde(rename = "l")]
    pub clist_rating: i32,
}

/// In-memory problem database built from the embedded JSON at startup.
pub struct ProblemDb {
    /// All problems in insertion order (sorted by rating asc, solved_count desc)
    problems: Vec<StaticProblem>,
    /// problem indices grouped by exact CF rating
    by_rating: HashMap<u32, Vec<usize>>,
    /// problem indices grouped by band id
    by_band: HashMap<i8, Vec<usize>>,
}

impl ProblemDb {
    fn new() -> Self {
        // Embedded at compile time — zero runtime I/O, works in Docker with no extra files.
        static RAW: &[u8] = include_bytes!("../data/problems.json");

        let problems: Vec<StaticProblem> = serde_json::from_slice(RAW)
            .expect("Failed to parse embedded problems.json — re-run scripts/build_problem_db.py and recompile");

        let mut by_rating: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut by_band:   HashMap<i8,  Vec<usize>> = HashMap::new();

        for (idx, p) in problems.iter().enumerate() {
            by_rating.entry(p.rating).or_default().push(idx);
            by_band.entry(p.band).or_default().push(idx);
        }

        tracing::info!(
            "ProblemDb loaded: {} problems, {} rating levels, {} bands",
            problems.len(),
            by_rating.len(),
            by_band.len()
        );

        Self { problems, by_rating, by_band }
    }

    /// Return the problem pool for the given difficulty + mode.
    fn pool(&self, difficulty: u32, mode: &DifficultyMode) -> Vec<&StaticProblem> {
        match mode {
            DifficultyMode::Cf => {
                self.by_rating
                    .get(&difficulty)
                    .map(|idxs| idxs.iter().map(|&i| &self.problems[i]).collect())
                    .unwrap_or_default()
            }
            DifficultyMode::Band => {
                let band_id = difficulty as i8;
                self.by_band
                    .get(&band_id)
                    .map(|idxs| idxs.iter().map(|&i| &self.problems[i]).collect())
                    .unwrap_or_default()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CFClient
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CFClient {
    client: Client,
    /// Cache for contest.standings results (used only by /api/contest/:id endpoint)
    contest_cache: ProblemCache,
    /// Static problem database (shared across all clones via Arc)
    problem_db: Arc<ProblemDb>,
}

// Problem shape returned by contest.standings – used only for /api/contest/:id
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContestProblem {
    #[serde(rename = "contestId")]
    pub contest_id: Option<i32>,
    pub index: String,
    pub name: String,
    pub rating: Option<i32>,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ContestStandingsResponse {
    status: String,
    result: ContestStandingsResult,
}

#[derive(Debug, Deserialize)]
struct ContestStandingsResult {
    problems: Vec<ContestProblem>,
}

#[derive(Debug, Deserialize)]
struct UserStatusResponse {
    status: String,
    result: Vec<Submission>,
}

#[derive(Debug, Deserialize)]
pub struct Submission {
    pub verdict: Option<String>,
    pub problem: SubmissionProblem,
    /// Unix timestamp (seconds) when submission was created on Codeforces
    #[serde(rename = "creationTimeSeconds")]
    pub creation_time_seconds: Option<i64>,
}

/// Minimal problem shape used only within user.status responses.
#[derive(Debug, Deserialize)]
pub struct SubmissionProblem {
    #[serde(rename = "contestId")]
    pub contest_id: Option<i32>,
    pub index: String,
}

impl Default for CFClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CFClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
            contest_cache: Arc::new(Mutex::new(HashMap::new())),
            problem_db: Arc::new(ProblemDb::new()),
        }
    }

    // -----------------------------------------------------------------------
    // Contest problems (used only by /api/contest/:id – not gameplay)
    // -----------------------------------------------------------------------

    pub async fn fetch_contest_problems(
        &self,
        contest_id: i32,
    ) -> Result<Vec<ContestProblem>, Box<dyn Error + Send + Sync>> {
        // 1. Check Cache (5 min TTL)
        {
            let cache = self.contest_cache.lock().await;
            if let Some((timestamp, problems)) = cache.get(&contest_id) {
                if timestamp.elapsed() < Duration::from_secs(300) {
                    return Ok(problems.clone());
                }
            }
        }

        // 2. Fetch from CF API – contest.standings with count=1 to get just the problem list
        let url = format!(
            "https://codeforces.com/api/contest.standings?contestId={}&from=1&count=1",
            contest_id
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<ContestStandingsResponse>()
            .await?;

        if resp.status != "OK" {
            return Err("Failed to fetch contest problems".into());
        }

        let problems = resp.result.problems;

        // 3. Update Cache
        {
            let mut cache = self.contest_cache.lock().await;
            cache.insert(contest_id, (Instant::now(), problems.clone()));
        }

        Ok(problems)
    }

    // -----------------------------------------------------------------------
    // Submission verification  (live CF API – irreplaceable)
    // -----------------------------------------------------------------------

    pub async fn verify_submission(
        &self,
        handle: &str,
        contest_id: i32,
        index: &str,
        locked_since_unix: Option<u64>,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // Fetch last 50 submissions — CF API returns newest first.
        let encoded_handle = urlencoding::encode(handle);
        let url = format!(
            "https://codeforces.com/api/user.status?handle={}&from=1&count=50",
            encoded_handle
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<UserStatusResponse>()
            .await?;

        if resp.status != "OK" {
            return Err("Failed to fetch user status".into());
        }

        for submission in resp.result {
            if let Some(verdict) = submission.verdict {
                if verdict == "OK"
                    && submission.problem.contest_id == Some(contest_id)
                    && submission.problem.index == index
                {
                    // SECURITY: reject pre-solved problems (30 s clock-skew tolerance)
                    if let Some(lock_time) = locked_since_unix {
                        if let Some(creation_time) = submission.creation_time_seconds {
                            if (creation_time as u64) + 30 < lock_time {
                                continue;
                            }
                        }
                    }
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    // -----------------------------------------------------------------------
    // Player solved-set  (live CF API – called once per game start)
    // -----------------------------------------------------------------------

    pub async fn fetch_player_solved(
        &self,
        handle: &str,
    ) -> Result<HashSet<String>, Box<dyn Error + Send + Sync>> {
        let encoded_handle = urlencoding::encode(handle);
        let url = format!(
            "https://codeforces.com/api/user.status?handle={}&from=1&count=5000",
            encoded_handle
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<UserStatusResponse>()
            .await?;

        if resp.status != "OK" {
            return Err("Failed to fetch user submissions".into());
        }

        let mut solved = HashSet::new();
        for sub in resp.result {
            if sub.verdict.as_deref() == Some("OK") {
                if let Some(cid) = sub.problem.contest_id {
                    solved.insert(format!("{}-{}", cid, sub.problem.index));
                }
            }
        }

        Ok(solved)
    }

    // -----------------------------------------------------------------------
    // Player solved-set with retry  (robust fetch at game start)
    // -----------------------------------------------------------------------

    /// Fetch player's solved problems with retry logic.
    /// Returns `(solved_set, success)`. On total failure, returns `(empty, false)`.
    /// Called at game start to ensure we never serve already-solved problems.
    pub async fn fetch_player_solved_with_retry(
        &self,
        handle: &str,
        max_retries: u32,
    ) -> (HashSet<String>, bool) {
        for attempt in 0..=max_retries {
            match self.fetch_player_solved(handle).await {
                Ok(solved) => return (solved, true),
                Err(e) => {
                    tracing::warn!(
                        "fetch_player_solved('{}') attempt {}/{} failed: {}",
                        handle, attempt + 1, max_retries + 1, e
                    );
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_secs(
                            2u64.saturating_pow(attempt),
                        ))
                        .await;
                    }
                }
            }
        }
        tracing::error!(
            "fetch_player_solved('{}') failed after {} retries — solved set unavailable",
            handle, max_retries + 1
        );
        (HashSet::new(), false)
    }

    // -----------------------------------------------------------------------
    // Problem selection  (in-memory – zero CF API calls for the pool lookup)
    // -----------------------------------------------------------------------

    /// Pick a random unsolved problem for the given difficulty + mode.
    ///
    /// Strategy:
    /// 1. Try the exact target difficulty — filter out solved problems.
    /// 2. If all problems at this level are solved, try adjacent levels
    ///    (next band in Band mode, ±100 rating in CF mode).
    /// 3. If truly every problem in the DB is solved, reuse the target pool.
    pub fn pick_problem(
        &self,
        difficulty: u32,
        mode: DifficultyMode,
        solved_set: &HashSet<String>,
    ) -> Result<StaticProblem, Box<dyn Error + Send + Sync>> {
        // 1. Try the exact target difficulty
        if let Some(p) = self.try_pick_unsolved(difficulty, &mode, solved_set) {
            return Ok(p);
        }

        // 2. All problems at this level are solved — try adjacent levels
        tracing::info!(
            "All unsolved problems exhausted at difficulty={} mode={:?} — trying adjacent levels",
            difficulty, mode
        );

        let fallback = match mode {
            DifficultyMode::Band => {
                let mut found = None;
                // Try bands outward: +1, -1, +2, -2, …
                for offset in 1..=4i32 {
                    for &dir in &[1, -1] {
                        let cand = difficulty as i32 + offset * dir;
                        if (0..=4).contains(&cand) {
                            if let Some(p) = self.try_pick_unsolved(cand as u32, &mode, solved_set) {
                                tracing::info!("Fallback: serving band {} instead of {}", cand, difficulty);
                                found = Some(p);
                                break;
                            }
                        }
                    }
                    if found.is_some() { break; }
                }
                found
            }
            DifficultyMode::Cf => {
                let mut found = None;
                // Try ratings outward: ±100, ±200, …
                for offset in 1..=27i32 {
                    for &dir in &[1, -1] {
                        let cand = difficulty as i32 + offset * 100 * dir;
                        if (800..=3500).contains(&cand) {
                            if let Some(p) = self.try_pick_unsolved(cand as u32, &mode, solved_set) {
                                tracing::info!("Fallback: serving rating {} instead of {}", cand, difficulty);
                                found = Some(p);
                                break;
                            }
                        }
                    }
                    if found.is_some() { break; }
                }
                found
            }
        };

        if let Some(p) = fallback {
            return Ok(p);
        }

        // 3. Every single problem in the DB is solved — reuse target pool
        tracing::warn!(
            "Player has solved ALL problems in the database — reusing target pool at difficulty={}",
            difficulty
        );
        let pool = self.problem_db.pool(difficulty, &mode);
        if pool.is_empty() {
            return Err(format!(
                "No problems in database for difficulty={} mode={:?}",
                difficulty, mode
            )
            .into());
        }
        let mut rng = rand::thread_rng();
        pool.choose(&mut rng)
            .map(|p| (*p).clone())
            .ok_or_else(|| "No problems available".into())
    }

    /// Try to pick a random unsolved problem from the pool at the given difficulty.
    /// Returns `None` if the pool is empty or every problem in it is already solved.
    fn try_pick_unsolved(
        &self,
        difficulty: u32,
        mode: &DifficultyMode,
        solved_set: &HashSet<String>,
    ) -> Option<StaticProblem> {
        let pool = self.problem_db.pool(difficulty, mode);
        if pool.is_empty() {
            return None;
        }
        let unsolved: Vec<&StaticProblem> = pool
            .iter()
            .copied()
            .filter(|p| !solved_set.contains(&format!("{}-{}", p.contest_id, p.index)))
            .collect();
        if unsolved.is_empty() {
            return None;
        }
        let mut rng = rand::thread_rng();
        unsolved.choose(&mut rng).map(|p| (*p).clone())
    }
}
