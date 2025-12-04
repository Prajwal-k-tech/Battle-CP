use crate::state::{AppState, Game};
use axum::{extract::State, http::StatusCode, response::Json};
use serde_json::{Value, json};
use uuid::Uuid;

pub async fn create_game(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let player_id = Uuid::new_v4();
    let new_game = Game::new(player_id);
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
