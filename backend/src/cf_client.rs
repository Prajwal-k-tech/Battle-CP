use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Type alias for the problem cache to reduce type complexity
type ProblemCache = Arc<Mutex<HashMap<i32, (Instant, Vec<Problem>)>>>;
/// Type alias for the user existence cache
type UserCache = Arc<Mutex<HashMap<String, (Instant, bool)>>>;

#[derive(Clone)]
pub struct CFClient {
    client: Client,
    cache: ProblemCache,
    user_cache: UserCache,
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
        }
    }

    pub async fn fetch_contest_problems(
        &self,
        contest_id: i32,
    ) -> Result<Vec<Problem>, Box<dyn Error + Send + Sync>> {
        // 1. Check Cache
        {
            let cache = self.cache.lock().unwrap();
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
            let mut cache = self.cache.lock().unwrap();
            cache.insert(contest_id, (Instant::now(), problems.clone()));
        }

        Ok(problems)
    }

    pub async fn verify_submission(
        &self,
        handle: &str,
        contest_id: i32,
        index: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://codeforces.com/api/user.status?handle={}&from=1&count=10",
            handle
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
            let cache = self.user_cache.lock().unwrap();
            if let Some((timestamp, exists)) = cache.get(handle) {
                if timestamp.elapsed() < Duration::from_secs(600) {
                    // 10 min cache
                    return Ok(*exists);
                }
            }
        }

        let url = format!("https://codeforces.com/api/user.info?handles={}", handle);

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
            let mut cache = self.user_cache.lock().unwrap();
            cache.insert(handle.to_string(), (Instant::now(), exists));
        }

        Ok(exists)
    }
}
