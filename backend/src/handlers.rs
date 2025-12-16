use crate::state::{AppState, Game, GameConfig};
use axum::{extract::State, http::StatusCode, response::Json};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateGameRequest {
    pub cf_handle: String,
    pub difficulty: Option<u32>,
    pub heat_threshold: Option<u32>,
    pub game_duration_mins: Option<u32>,
    pub veto_strictness: Option<String>, // "low", "medium", "high"
}

pub async fn create_game(
    State(state): State<AppState>,
    Json(payload): Json<CreateGameRequest>,
) -> (StatusCode, Json<Value>) {
    let handle = payload.cf_handle.trim();

    // Validate CF handle exists
    match state.cf_client.verify_user_exists(handle).await {
        Ok(true) => {} // User exists, continue
        Ok(false) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Codeforces handle not found" })),
            );
        }
        Err(e) => {
            // Log error but don't block game creation (CF API might be down)
            tracing::warn!("CF validation failed: {}", e);
        }
    }

    let player_id = Uuid::new_v4();

    // Parse veto strictness to penalties
    let veto_penalties = match payload.veto_strictness.as_deref() {
        Some("low") => [300, 420, 600],   // 5, 7, 10 min
        Some("high") => [600, 900, 1200], // 10, 15, 20 min
        _ => [420, 600, 900],             // 7, 10, 15 min (default/medium)
    };

    let config = GameConfig {
        difficulty: payload.difficulty.unwrap_or(800),
        heat_threshold: payload.heat_threshold.unwrap_or(7),
        game_duration_secs: payload
            .game_duration_mins
            .map(|m| (m * 60) as u64)
            .unwrap_or(45 * 60),
        veto_penalties,
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
