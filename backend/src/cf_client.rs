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
type ProblemCache = Arc<Mutex<HashMap<i32, (Instant, Vec<ContestProblem>)>>>;
/// Type alias for the player-solved-problems cache (keyed by handle)
type SolvedCache = Arc<Mutex<HashMap<String, (Instant, HashSet<String>)>>>;

// ---------------------------------------------------------------------------
// Static problem database – loaded once at startup from embedded JSON
// ---------------------------------------------------------------------------

/// Difficulty bands.  The `difficulty` field in `Band` mode is one of these ids.
/// Ranges are CF ratings (inclusive on both ends).
pub const BANDS: &[(u8, &str, u32, u32)] = &[
    (0, "Super Easy", 800,  1200),
    (1, "Easy",       1201, 1500),
    (2, "Medium",     1501, 1900),
    (3, "Hard",       1901, 2400),
    (4, "Very Hard",  2401, 9999),
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
    /// band id 0-4 (-1 if unclassified)
    #[serde(rename = "b")]
    pub band: i8,
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
    /// Cache for user.status solved-set lookups
    solved_cache: SolvedCache,
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
            solved_cache: Arc::new(Mutex::new(HashMap::new())),
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
    // Player solved-set  (live CF API with 10-min per-handle cache)
    // -----------------------------------------------------------------------

    pub async fn fetch_player_solved(
        &self,
        handle: &str,
    ) -> Result<HashSet<String>, Box<dyn Error + Send + Sync>> {
        // 1. Check cache (10 min TTL)
        {
            let cache = self.solved_cache.lock().await;
            if let Some((timestamp, solved)) = cache.get(handle) {
                if timestamp.elapsed() < Duration::from_secs(600) {
                    return Ok(solved.clone());
                }
            }
        }

        // 2. Fetch last 5 000 submissions from CF
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

        // 3. Update cache
        {
            let mut cache = self.solved_cache.lock().await;
            cache.insert(handle.to_string(), (Instant::now(), solved.clone()));
        }

        Ok(solved)
    }

    // -----------------------------------------------------------------------
    // Problem selection  (in-memory – zero CF API calls for the pool lookup)
    // -----------------------------------------------------------------------

    /// Pick a random unsolved problem for the given difficulty + mode.
    ///
    /// - `difficulty`: CF rating (Cf mode) OR band id 0–4 (Band mode)
    /// - `mode`:       which difficulty system to use
    /// - `handle`:     used to fetch the player's solved set (CF API, cached 10 min)
    pub async fn pick_problem(
        &self,
        difficulty: u32,
        mode: DifficultyMode,
        handle: &str,
    ) -> Result<StaticProblem, Box<dyn Error + Send + Sync>> {
        // 1. Get the in-memory pool — instant, zero I/O
        let pool = self.problem_db.pool(difficulty, &mode);

        if pool.is_empty() {
            return Err(format!(
                "No problems in database for difficulty={} mode={:?}",
                difficulty, mode
            )
            .into());
        }

        // 2. Fetch solved set (CF API, cached 10 min per handle)
        let solved = self.fetch_player_solved(handle).await.unwrap_or_default();

        // 3. Filter out already-solved problems
        let unsolved: Vec<&StaticProblem> = pool
            .iter()
            .copied()
            .filter(|p| !solved.contains(&format!("{}-{}", p.contest_id, p.index)))
            .collect();

        let candidates = if unsolved.is_empty() {
            tracing::warn!(
                "Player {} has solved all {} problems at difficulty={} mode={:?} — reusing full pool",
                handle, pool.len(), difficulty, mode
            );
            pool
        } else {
            unsolved
        };

        let mut rng = rand::thread_rng();
        candidates
            .choose(&mut rng)
            .map(|p| (*p).clone())
            .ok_or_else(|| "No problems available".into())
    }
}
