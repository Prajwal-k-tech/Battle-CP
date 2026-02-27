use crate::state::{AppState, DifficultyMode, Game, GameConfig};
use axum::{extract::State, http::StatusCode, response::Json};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateGameRequest {
    pub cf_handle: String,
    /// In Cf mode  : CF rating (800 – 3500)
    /// In Band mode: band id  (0 = Super Easy … 4 = Very Hard)
    pub difficulty: Option<u32>,
    /// "cf" (default) or "band"
    pub difficulty_mode: Option<DifficultyMode>,
    pub heat_threshold: Option<u32>,
    pub game_duration_mins: Option<u32>,
    pub veto_strictness: Option<String>, // "low", "medium", "high"
}

pub async fn create_game(
    State(state): State<AppState>,
    Json(payload): Json<CreateGameRequest>,
) -> (StatusCode, Json<Value>) {
    let handle = payload.cf_handle.trim();

    // Trust the user's CF handle — verification removed for performance.
    // Entering a wrong handle is self-punishing: the player won't be able
    // to verify CP solutions, so they can never unlock weapons.

    // RATE LIMIT: max 3 game creations per CF handle per 5 minutes
    {
        let mut limiter = state.rate_limiter.lock().await;
        let now = std::time::Instant::now();
        let window = std::time::Duration::from_secs(300); // 5 minutes

        let entry = limiter
            .entry(handle.to_lowercase())
            .or_insert((now, 0));

        if now.duration_since(entry.0) > window {
            // Window expired — reset
            *entry = (now, 1);
        } else {
            entry.1 += 1;
            if entry.1 > 3 {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({ "error": "Too many games created. Please wait a few minutes." })),
                );
            }
        }
    }

    let player_id = Uuid::new_v4();

    // Parse veto strictness to penalties
    let veto_penalties = match payload.veto_strictness.as_deref() {
        Some("low") => [300, 420, 600],   // 5, 7, 10 min
        Some("high") => [600, 900, 1200], // 10, 15, 20 min
        _ => [420, 600, 900],             // 7, 10, 15 min (default/medium)
    };

    let mode = payload.difficulty_mode.unwrap_or(DifficultyMode::Cf);

    // Validate difficulty range depends on the mode
    let difficulty = match mode {
        DifficultyMode::Cf => payload.difficulty.unwrap_or(800).clamp(800, 3500),
        DifficultyMode::Band => payload.difficulty.unwrap_or(0).clamp(0, 4),
    };

    let config = GameConfig {
        difficulty,
        difficulty_mode: mode,
        heat_threshold: payload.heat_threshold.unwrap_or(7).clamp(3, 20),
        // Prevent overflow: clamp minutes first, then convert
        game_duration_secs: payload
            .game_duration_mins
            .map(|m| m.clamp(1, 120)) // Clamp to 1-120 minutes first
            .map(|m| (m as u64).saturating_mul(60)) // Safe conversion to u64
            .unwrap_or(45 * 60)
            .clamp(60, 7200), // Final clamp to 1-120 minutes in seconds
        veto_penalties,
        // Default max_vetoes to 3
        ..GameConfig::default()
    };

    let new_game = Game::new(player_id, handle.to_string(), config);
    let game_id = new_game.id;

    state.games.write().await.insert(game_id, new_game);

    (
        StatusCode::CREATED,
        Json(json!({
            "game_id": game_id,
            "player_id": player_id
        })),
    )
}

/// Fetch problems for a specific Codeforces contest
pub async fn get_contest_problems(
    State(state): State<AppState>,
    axum::extract::Path(contest_id): axum::extract::Path<i32>,
) -> (StatusCode, Json<Value>) {
    match state.cf_client.fetch_contest_problems(contest_id).await {
        Ok(problems) => (StatusCode::OK, Json(json!({ "problems": problems }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
