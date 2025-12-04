use crate::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use uuid::Uuid;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(game_id): Path<Uuid>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, game_id, state))
}

async fn handle_socket(mut socket: WebSocket, game_id: Uuid, _state: AppState) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    println!("Received message for game {}: {}", game_id, text);
                    // TODO: Parse message and call game logic
                    if socket
                        .send(Message::Text(format!("Echo: {}", text)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                _ => {}
            }
        } else {
            break;
        }
    }
}
