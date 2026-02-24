use rand::seq::SliceRandom;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Type alias for the contest-problem cache (keyed by contest_id)
type ProblemCache = Arc<Mutex<HashMap<i32, (Instant, Vec<Problem>)>>>;
/// Type alias for the user existence cache
type UserCache = Arc<Mutex<HashMap<String, (Instant, bool)>>>;
/// Type alias for the problem-by-rating cache (keyed by rating)
type RatingProblemCache = Arc<Mutex<HashMap<u32, (Instant, Vec<Problem>)>>>;
/// Type alias for the player-solved-problems cache (keyed by handle)
type SolvedCache = Arc<Mutex<HashMap<String, (Instant, HashSet<String>)>>>;

#[derive(Clone)]
pub struct CFClient {
    client: Client,
    cache: ProblemCache,
    user_cache: UserCache,
    rating_cache: RatingProblemCache,
    solved_cache: SolvedCache,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Problem {
    #[serde(rename = "contestId")]
    pub contest_id: Option<i32>,
    pub index: String,
    pub name: String,
    pub rating: Option<i32>,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContestStandingsResponse {
    status: String,
    result: ContestStandingsResult,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContestStandingsResult {
    problems: Vec<Problem>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserStatusResponse {
    status: String,
    result: Vec<Submission>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Submission {
    id: i64,
    pub verdict: Option<String>,
    pub problem: Problem,
    /// Unix timestamp (seconds) when submission was created on Codeforces
    #[serde(rename = "creationTimeSeconds")]
    pub creation_time_seconds: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProblemsetResponse {
    status: String,
    result: ProblemsetResult,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProblemsetResult {
    problems: Vec<Problem>,
}

impl Default for CFClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CFClient {
    pub fn new() -> Self {
        Self {
            // 15 second timeout to prevent hanging if CF is slow/down
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
            cache: Arc::new(Mutex::new(HashMap::new())),
            user_cache: Arc::new(Mutex::new(HashMap::new())),
            rating_cache: Arc::new(Mutex::new(HashMap::new())),
            solved_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn fetch_contest_problems(
        &self,
        contest_id: i32,
    ) -> Result<Vec<Problem>, Box<dyn Error + Send + Sync>> {
        // 1. Check Cache
        {
            let cache = self.cache.lock().await;
            if let Some((timestamp, problems)) = cache.get(&contest_id) {
                if timestamp.elapsed() < Duration::from_secs(300) {
                    // 5 min cache
                    return Ok(problems.clone());
                }
            }
        }

        // 2. Fetch from API
        // Use contest.standings with count=1 to just get the problem list (and 1 row of standings, negligible)
        // usage: https://codeforces.com/api/contest.standings?contestId=566&from=1&count=1
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
            let mut cache = self.cache.lock().await;
            cache.insert(contest_id, (Instant::now(), problems.clone()));
        }

        Ok(problems)
    }

    pub async fn verify_submission(
        &self,
        handle: &str,
        contest_id: i32,
        index: &str,
        locked_since_unix: Option<u64>,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // Fetch last 50 submissions — CF API returns newest first.
        // With active_problem commitment, the player's solve is always recent
        // so 50 is more than sufficient for verification.
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
                    // SECURITY: Verify submission was created AFTER the player got locked.
                    // This prevents pre-solving exploits where a player submits a solution
                    // before the game starts and then commits that problem when locked.
                    if let Some(lock_time) = locked_since_unix {
                        if let Some(creation_time) = submission.creation_time_seconds {
                            // Allow 30 seconds of clock skew tolerance
                            if (creation_time as u64) + 30 < lock_time {
                                continue; // Submission is too old, skip it
                            }
                        }
                        // If creation_time is missing, accept it (defensive)
                    }
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Verify if a Codeforces user handle exists
    pub async fn verify_user_exists(
        &self,
        handle: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // 1. Check Cache
        {
            let cache = self.user_cache.lock().await;
            if let Some((timestamp, exists)) = cache.get(handle) {
                if timestamp.elapsed() < Duration::from_secs(600) {
                    // 10 min cache
                    return Ok(*exists);
                }
            }
        }

        let encoded_handle = urlencoding::encode(handle);
        let url = format!("https://codeforces.com/api/user.info?handles={}", encoded_handle);

        let resp = self.client.get(&url).send().await?;
        let mut exists = false;

        // Check if we got a valid response
        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            // If status is "OK", user exists
            if body.get("status").and_then(|v| v.as_str()) == Some("OK") {
                exists = true;
            }
        }

        // 2. Update Cache
        {
            let mut cache = self.user_cache.lock().await;
            cache.insert(handle.to_string(), (Instant::now(), exists));
        }

        Ok(exists)
    }

    /// Fetch all problems from Codeforces and cache by rating.
    /// Returns problems at the requested difficulty rating.
    pub async fn fetch_problems_by_rating(
        &self,
        difficulty: u32,
    ) -> Result<Vec<Problem>, Box<dyn Error + Send + Sync>> {
        // 1. Check cache (30 min TTL)
        {
            let cache = self.rating_cache.lock().await;
            if let Some((timestamp, problems)) = cache.get(&difficulty) {
                if timestamp.elapsed() < Duration::from_secs(1800) {
                    return Ok(problems.clone());
                }
            }
        }

        // 2. Fetch entire problemset from CF API
        let url = "https://codeforces.com/api/problemset.problems";
        let resp = self
            .client
            .get(url)
            .send()
            .await?
            .json::<ProblemsetResponse>()
            .await?;

        if resp.status != "OK" {
            return Err("Failed to fetch problemset".into());
        }

        // 3. Group by rating and cache all ratings at once
        let mut by_rating: HashMap<u32, Vec<Problem>> = HashMap::new();
        for p in resp.result.problems {
            if let Some(rating) = p.rating {
                let entry = by_rating.entry(rating as u32).or_default();
                if entry.len() < 1000 {
                    // Cap at 1000 per rating to limit memory
                    entry.push(p);
                }
            }
        }

        let now = Instant::now();
        let result = by_rating.get(&difficulty).cloned().unwrap_or_default();

        {
            let mut cache = self.rating_cache.lock().await;
            for (rating, problems) in by_rating {
                cache.insert(rating, (now, problems));
            }
        }

        if result.is_empty() {
            return Err(format!("No problems found at rating {}", difficulty).into());
        }

        Ok(result)
    }

    /// Fetch a player's solved problem set from Codeforces.
    /// Cached for 10 minutes per handle.
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

        // 2. Fetch user submissions (up to 5000 most recent)
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

    /// Server-side problem selection: pick a random unsolved problem at the given difficulty.
    /// This is the authoritative source — the client never picks problems.
    pub async fn pick_problem(
        &self,
        difficulty: u32,
        handle: &str,
    ) -> Result<Problem, Box<dyn Error + Send + Sync>> {
        let problems = self.fetch_problems_by_rating(difficulty).await?;

        // Try to filter out already-solved problems
        let solved = self.fetch_player_solved(handle).await.unwrap_or_default();

        let unsolved: Vec<&Problem> = problems
            .iter()
            .filter(|p| {
                if let Some(cid) = p.contest_id {
                    !solved.contains(&format!("{}-{}", cid, p.index))
                } else {
                    true
                }
            })
            .collect();

        let pool = if unsolved.is_empty() {
            tracing::warn!(
                "Player {} has solved all {} problems at rating {}",
                handle,
                problems.len(),
                difficulty
            );
            problems.iter().collect::<Vec<_>>()
        } else {
            unsolved
        };

        let mut rng = rand::thread_rng();
        pool.choose(&mut rng)
            .map(|p| (*p).clone())
            .ok_or_else(|| "No problems available".into())
    }
}
