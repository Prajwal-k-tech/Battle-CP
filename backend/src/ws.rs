//! WebSocket handler for real-time game communication.
//!
//! Handles:
//! - Player joining/reconnecting
//! - Ship placement
//! - Firing shots
//! - CP problem solving verification
//! - Veto timer mechanism

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use uuid::Uuid;

use crate::protocol::{ClientMessage, ServerMessage};
use crate::state::{AppState, CellState, GameStatus, Ship};

#[derive(Deserialize)]
pub struct WsQuery {
    pub player_id: Option<Uuid>,
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(game_id): Path<Uuid>,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.max_frame_size(8192) // 8 KB max frame — prevents memory bombs
        .max_message_size(16384) // 16 KB max message
        .on_upgrade(move |socket| handle_socket(socket, game_id, query.player_id, state))
}

/// Main WebSocket connection handler
async fn handle_socket(
    socket: WebSocket,
    game_id: Uuid,
    initial_player_id: Option<Uuid>,
    state: AppState,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut player_id: Option<Uuid> = initial_player_id;

    // Per-connection, per-message-type rate limiters.
    // Silently drops messages that arrive faster than the minimum interval.
    let mut last_fire_at:  Option<std::time::Instant> = None;
    let mut last_place_at: Option<std::time::Instant> = None;
    let mut last_solve_at: Option<std::time::Instant> = None;
    let mut last_veto_at:  Option<std::time::Instant> = None;
    let mut last_join_at:  Option<std::time::Instant> = None;

    // Subscribe to game events
    let rx = {
        let games = state.games.read().await;
        games.get(&game_id).map(|g| {
            tracing::debug!(
                "[WS] Player connecting to game {:?}, subscribing to broadcast (current subs: {})",
                game_id,
                g.tx.receiver_count()
            );
            g.tx.subscribe()
        })
    };

    let mut rx = match rx {
        Some(rx) => rx,
        None => {
            tracing::warn!("[WS] Game {:?} not found!", game_id);
            let _ = sender
                .send(Message::Text(
                    serde_json::to_string(&ServerMessage::Error {
                        message: "Game not found".to_string(),
                    })
                    .unwrap()
                    .into(),
                ))
                .await;
            return;
        }
    };

    'main_loop: loop {
        tokio::select! {
            // Handle incoming messages from client
            msg_opt = receiver.next() => {
                match msg_opt {
                    Some(Ok(msg)) => {
                        if let Message::Text(text) = msg {
                            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                // Per-message-type rate limiting.
                                // Fire: 200ms (fast action), others: 2s (prevents lock contention spam).
                                macro_rules! rate_check {
                                    ($tracker:expr, $min_ms:expr) => {
                                        if let Some(last) = $tracker {
                                            if last.elapsed() < std::time::Duration::from_millis($min_ms) {
                                                continue; // Silently drop rapid spam
                                            }
                                        }
                                        $tracker = Some(std::time::Instant::now());
                                    };
                                }
                                match &client_msg {
                                    ClientMessage::Fire { .. }       => { rate_check!(last_fire_at,  200);  }
                                    ClientMessage::PlaceShips { .. } => { rate_check!(last_place_at, 2000); }
                                    ClientMessage::SolveCP { .. }    => { rate_check!(last_solve_at, 2000); }
                                    ClientMessage::Veto              => { rate_check!(last_veto_at,  2000); }
                                    ClientMessage::JoinGame { .. }   => { rate_check!(last_join_at,  2000); }
                                }

                                let responses = handle_client_message(
                                    client_msg,
                                    &mut player_id,
                                    game_id,
                                    &state,
                                ).await;

                                for resp in responses {
                                    let resp_text = match serde_json::to_string(&resp) {
                                        Ok(t) => t,
                                        Err(e) => {
                                            tracing::error!("[WS] Failed to serialize response: {}", e);
                                            continue;
                                        }
                                    };
                                    if sender.send(Message::Text(resp_text.into())).await.is_err() {
                                        tracing::warn!("[WS] Failed to send response, closing connection");
                                        break 'main_loop;
                                    }
                                }
                            } else {
                                tracing::debug!("[WS] Failed to parse message: {}", text);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        tracing::warn!("[WS] Receive error: {:?}", e);
                        break 'main_loop;
                    }
                    None => {
                        tracing::debug!("[WS] Client disconnected");
                        break 'main_loop;
                    }
                }
            }

            // Handle broadcast messages (game ticks, etc.)
            event_res = rx.recv() => {
                match event_res {
                    Ok(event) => {
                        match event {
                            crate::state::GameEvent::Tick => {
                                // Send periodic game state update
                                if let Some(pid) = player_id {
                                    let games = state.games.read().await;
                                    if let Some(game) = games.get(&game_id) {
                                        let is_p1 = game.player1.id == pid;
                                        let player = if is_p1 {
                                            Some(&game.player1)
                                        } else if game.player2.as_ref().map(|p| p.id) == Some(pid) {
                                            game.player2.as_ref()
                                        } else {
                                            None
                                        };

                                        if let Some(p) = player {
                                            // FALLBACK: If this is P1 (Host) and P2 exists but game is still Waiting,
                                            // send PlayerJoined to ensure Host knows about Guest
                                            if is_p1 && game.player2.is_some() && game.status == crate::state::GameStatus::Waiting {
                                                let p2_id = game.player2.as_ref().unwrap().id;
                                                let joined_msg = ServerMessage::PlayerJoined { player_id: p2_id };
                                                if let Ok(msg_text) = serde_json::to_string(&joined_msg) {
                                                    let _ = sender.send(Message::Text(msg_text.into())).await;
                                                }
                                            }

                                            let elapsed = game.game_started_at.map(|s| s.elapsed().as_secs()).unwrap_or(0);
                                            let remaining = game.config.game_duration_secs.saturating_sub(elapsed);

                                            // Calculate veto time remaining if player is on veto timer
                                            let veto_durations = game.config.veto_penalties;
                                            let veto_time_remaining = if let Some(veto_start) = p.veto_started_at {
                                                let duration = veto_durations
                                                    .get(p.vetoes_used.saturating_sub(1) as usize)
                                                    .copied()
                                                    .unwrap_or(900);
                                                let elapsed_veto = veto_start.elapsed().as_secs();
                                                if elapsed_veto < duration {
                                                    Some(duration - elapsed_veto)
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                            let update = ServerMessage::GameUpdate {
                                                status: match game.status {
                                                    crate::state::GameStatus::SuddenDeath =>
                                                        "SUDDEN DEATH! First hit wins!".to_string(),
                                                    _ => format!("{:?}", game.status),
                                                },
                                                is_active: true,
                                                heat: p.heat,
                                                is_locked: p.is_locked,
                                                time_remaining_secs: remaining,
                                                vetoes_remaining: game.config.max_vetoes.saturating_sub(p.vetoes_used),
                                                veto_time_remaining_secs: veto_time_remaining,
                                                active_problem_contest_id: p.active_problem.as_ref().map(|ap| ap.contest_id),
                                                active_problem_index: p.active_problem.as_ref().map(|ap| ap.index.clone()),
                                                active_problem_name: p.active_problem.as_ref().map(|ap| ap.name.clone()),
                                            };
                                            if let Ok(resp_text) = serde_json::to_string(&update) {
                                                if sender.send(Message::Text(resp_text.into())).await.is_err() {
                                                    tracing::warn!("[WS] Failed to send tick update, closing connection");
                                                    break 'main_loop;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            crate::state::GameEvent::Message(msg) => {
                                 if let Ok(resp_text) = serde_json::to_string(&msg) {
                                     if sender.send(Message::Text(resp_text.into())).await.is_err() {
                                         tracing::warn!("[WS] Failed to send broadcast message, closing connection");
                                         break 'main_loop;
                                     }
                                 }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("[WS] Broadcast receiver lagged by {} messages", n);
                        // Continue - we can recover from lag
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::debug!("[WS] Broadcast channel closed, ending connection");
                        break 'main_loop;
                    }
                }
            }
        }
    }
    tracing::debug!(
        "[WS] Connection handler exiting for player {:?} in game {:?}",
        player_id,
        game_id
    );
}

/// Process individual client messages
async fn handle_client_message(
    msg: ClientMessage,
    player_id: &mut Option<Uuid>,
    game_id: Uuid,
    state: &AppState,
) -> Vec<ServerMessage> {
    match msg {
        ClientMessage::JoinGame {
            player_id: pid,
            cf_handle,
        } => {
            // SECURITY: Lock player_id to the first JoinGame message.
            // Subsequent JoinGame messages with a different player_id are rejected
            // to prevent impersonation attacks.
            if let Some(existing_pid) = *player_id {
                if existing_pid != pid {
                    return vec![ServerMessage::Error {
                        message: "Player identity already established".to_string(),
                    }];
                }
            }
            *player_id = Some(pid);
            let mut games = state.games.write().await;
            if let Some(game) = games.get_mut(&game_id) {
                // Check if game is finished - allow original participants to rejoin and see results
                if game.status == crate::state::GameStatus::Finished {
                    let is_p1 = game.player1.id == pid;
                    let is_p2 = game.player2.as_ref().map(|p| p.id) == Some(pid);
                    if is_p1 || is_p2 {
                        if let Some(go_msg) = &game.game_over_msg {
                            // Send GameJoined first so frontend sets playerId before processing GameOver
                            return vec![
                                ServerMessage::GameJoined {
                                    game_id,
                                    player_id: pid,
                                    difficulty: game.config.difficulty,
                                    difficulty_mode: game.config.difficulty_mode.clone(),
                                    max_heat: game.config.heat_threshold,
                                    max_vetoes: game.config.max_vetoes,
                                },
                                go_msg.clone(),
                            ];
                        }
                    }
                    return vec![ServerMessage::Error {
                        message: "Game has already ended".to_string(),
                    }];
                }

                // Check if player is already in the game (Reconnect)
                // SECURITY: Only match by player_id — CF handles are public and not auth tokens
                let is_p1 = game.player1.id == pid;
                let is_p2 = game.player2.as_ref().map(|p| p.id) == Some(pid);

                if is_p1 || is_p2 {
                    // RECONNECTION LOGIC
                    let mut msgs = vec![];

                    // 1. Confirm Join
                    msgs.push(ServerMessage::GameJoined {
                        game_id,
                        player_id: pid,
                        difficulty: game.config.difficulty,
                        difficulty_mode: game.config.difficulty_mode.clone(),
                        max_heat: game.config.heat_threshold,
                        max_vetoes: game.config.max_vetoes,
                    });

                    // 2. Send Current State
                    let player = if is_p1 {
                        &game.player1
                    } else {
                        game.player2.as_ref().unwrap()
                    };
                    let elapsed = game
                        .game_started_at
                        .map(|s| s.elapsed().as_secs())
                        .unwrap_or(0);
                    let remaining = game.config.game_duration_secs.saturating_sub(elapsed);
                    // Calculate remaining veto time so reconnected player sees the correct countdown
                    let veto_durations = game.config.veto_penalties;
                    let veto_time_remaining = player.veto_started_at.and_then(|veto_start| {
                        let duration = veto_durations
                            .get(player.vetoes_used.saturating_sub(1) as usize)
                            .copied()
                            .unwrap_or(900);
                        let elapsed_veto = veto_start.elapsed().as_secs();
                        if elapsed_veto < duration {
                            Some(duration - elapsed_veto)
                        } else {
                            None
                        }
                    });
                    msgs.push(ServerMessage::GameUpdate {
                        status: match game.status {
                            crate::state::GameStatus::SuddenDeath => {
                                "SUDDEN DEATH! First hit wins!".to_string()
                            }
                            _ => format!("{:?}", game.status),
                        },
                        is_active: true,
                        heat: player.heat,
                        is_locked: player.is_locked,
                        time_remaining_secs: remaining,
                        vetoes_remaining: game.config.max_vetoes.saturating_sub(player.vetoes_used),
                        veto_time_remaining_secs: veto_time_remaining,
                        active_problem_contest_id: player
                            .active_problem
                            .as_ref()
                            .map(|ap| ap.contest_id),
                        active_problem_index: player
                            .active_problem
                            .as_ref()
                            .map(|ap| ap.index.clone()),
                        active_problem_name: player
                            .active_problem
                            .as_ref()
                            .map(|ap| ap.name.clone()),
                    });

                    // 3. If ships placed, confirm and RESEND ships
                    if player.ships_placed {
                        msgs.push(ServerMessage::ShipsConfirmed { player_id: pid });

                        if !player.ships.is_empty() {
                            msgs.push(ServerMessage::YourShips {
                                ships: player
                                    .ships
                                    .iter()
                                    .map(|s| crate::protocol::ShipPlacement {
                                        x: s.x,
                                        y: s.y,
                                        size: s.size,
                                        vertical: s.vertical,
                                    })
                                    .collect(),
                            });
                        }
                    }

                    // 4. Tell reconnecting player the opponent is here (Bug 2 fix)
                    if game.status == crate::state::GameStatus::PlacingShips
                        || game.status == crate::state::GameStatus::Initializing
                        || game.status == crate::state::GameStatus::Playing
                        || game.status == crate::state::GameStatus::SuddenDeath
                    {
                        let opponent_id = if is_p1 {
                            game.player2.as_ref().map(|p| p.id)
                        } else {
                            Some(game.player1.id)
                        };
                        if let Some(oid) = opponent_id {
                            msgs.push(ServerMessage::PlayerJoined { player_id: oid });
                        }

                        // Re-send opponent's ShipsConfirmed if they already placed
                        let opponent_placed = if is_p1 {
                            game.player2
                                .as_ref()
                                .map(|p| p.ships_placed)
                                .unwrap_or(false)
                        } else {
                            game.player1.ships_placed
                        };
                        if opponent_placed {
                            let oid = if is_p1 {
                                game.player2.as_ref().unwrap().id
                            } else {
                                game.player1.id
                            };
                            msgs.push(ServerMessage::ShipsConfirmed { player_id: oid });
                        }
                    }

                    // 5. If game started (both placed), send GameStart and Grids
                    if game.status == crate::state::GameStatus::Playing
                        || game.status == crate::state::GameStatus::SuddenDeath
                    {
                        msgs.push(ServerMessage::GameStart);

                        // My Grid
                        let my_grid: Vec<Vec<String>> = player
                            .grid
                            .cells
                            .iter()
                            .map(|row| {
                                row.iter()
                                    .map(|cell| match cell {
                                        crate::state::CellState::Empty => "empty".to_string(),
                                        crate::state::CellState::Ship => "ship".to_string(),
                                        crate::state::CellState::Hit => "hit".to_string(),
                                        crate::state::CellState::Miss => "miss".to_string(),
                                    })
                                    .collect()
                            })
                            .collect();

                        // Enemy Grid
                        let enemy = if is_p1 {
                            game.player2.as_ref()
                        } else {
                            Some(&game.player1)
                        };

                        let enemy_grid: Vec<Vec<String>> = if let Some(enemy_p) = enemy {
                            enemy_p
                                .grid
                                .cells
                                .iter()
                                .map(|row| {
                                    row.iter()
                                        .map(|cell| match cell {
                                            crate::state::CellState::Empty
                                            | crate::state::CellState::Ship => "empty".to_string(), // Hide ships!
                                            crate::state::CellState::Hit => "hit".to_string(),
                                            crate::state::CellState::Miss => "miss".to_string(),
                                        })
                                        .collect()
                                })
                                .collect()
                        } else {
                            // Should not happen if playing
                            vec![vec!["empty".to_string(); 10]; 10]
                        };

                        msgs.push(ServerMessage::GridSync {
                            my_grid,
                            enemy_grid,
                        });
                    }

                    // Prefetch solved set on first connection if not already done.
                    // Spreads CF API load: P1 prefetches while waiting for P2,
                    // P2 prefetches during placement.
                    let should_prefetch = if is_p1 {
                        !game.player1.solved_set_ready
                    } else {
                        game.player2.as_ref().map(|p| !p.solved_set_ready).unwrap_or(false)
                    };
                    if should_prefetch {
                        let state2 = state.clone();
                        let gid = game_id;
                        let handle = if is_p1 {
                            game.player1.cf_handle.clone()
                        } else {
                            game.player2.as_ref().unwrap().cf_handle.clone()
                        };
                        let p_id = pid;
                        tokio::spawn(async move {
                            prefetch_solved_set(state2, gid, p_id, handle).await;
                        });
                    }

                    return msgs;
                }

                // Check if player is trying to join as P2
                if game.player1.id != pid && game.player2.is_none() {
                    if game.player1.cf_handle.eq_ignore_ascii_case(&cf_handle) {
                        return vec![ServerMessage::Error {
                            message: "You cannot play against yourself!".to_string(),
                        }];
                    }

                    // P2 is joining - get P1's ID before joining
                    let p1_id = game.player1.id;

                    // Trust the user's CF handle — verification removed for performance.
                    // Entering a wrong handle is self-punishing: the player can't verify
                    // CP solutions on someone else's account, so weapons stay locked forever.

                    if let Err(e) = game.join(pid, cf_handle) {
                        return vec![ServerMessage::Error {
                            message: e.to_string(),
                        }];
                    }

                    // Set status to PlacingShips now that both players are in
                    game.status = crate::state::GameStatus::PlacingShips;

                    // Broadcast PlayerJoined to Host (P1) so they know P2 joined
                    let result = game.tx.send(crate::state::GameEvent::Message(
                        ServerMessage::PlayerJoined { player_id: pid },
                    ));
                    tracing::debug!(
                        "[WS] Broadcast PlayerJoined for {:?} - result: {:?}, subscribers: {}",
                        pid,
                        result.is_ok(),
                        game.tx.receiver_count()
                    );

                    // Return GameJoined + PlayerJoined for P1 to the joining Guest
                    // This tells Guest that the opponent (Host) already exists

                    // Spawn P2 solved-set prefetch so it runs during the placement phase.
                    // By the time both players finish placing ships, this is likely done.
                    {
                        let state2 = state.clone();
                        let gid = game_id;
                        let p2_handle = game.player2.as_ref().unwrap().cf_handle.clone();
                        tokio::spawn(async move {
                            prefetch_solved_set(state2, gid, pid, p2_handle).await;
                        });
                    }

                    return vec![
                        ServerMessage::GameJoined {
                            game_id,
                            player_id: pid,
                            difficulty: game.config.difficulty,
                            difficulty_mode: game.config.difficulty_mode.clone(),
                            max_heat: game.config.heat_threshold,
                            max_vetoes: game.config.max_vetoes,
                        },
                        ServerMessage::PlayerJoined { player_id: p1_id },
                    ];
                }

                // Third player trying to join a full game — explicitly reject
                if game.player1.id != pid && game.player2.is_some() {
                    return vec![ServerMessage::Error {
                        message: "Game already has 2 players.".to_string(),
                    }];
                }

                // If we reach here, player is P1 (host) connecting for first time
                // This should only happen if P1 connects before calling JoinGame
                vec![ServerMessage::GameJoined {
                    game_id,
                    player_id: pid,
                    difficulty: game.config.difficulty,
                    difficulty_mode: game.config.difficulty_mode.clone(),
                    max_heat: game.config.heat_threshold,
                    max_vetoes: game.config.max_vetoes,
                }]
            } else {
                vec![ServerMessage::Error {
                    message: "Game not found".to_string(),
                }]
            }
        }

        ClientMessage::PlaceShips { ships } => {
            let pid = (*player_id).unwrap_or_default(); // Should handle None better but simplified
            if pid == Uuid::default() {
                return vec![ServerMessage::Error {
                    message: "No player ID".to_string(),
                }];
            }

            let mut games = state.games.write().await;
            let game = match games.get_mut(&game_id) {
                Some(g) => g,
                None => {
                    return vec![ServerMessage::Error {
                        message: "Game not found".to_string(),
                    }]
                }
            };

            // Determine if player is P1 or P2
            let is_player1 = game.player1.id == pid;
            let is_player2 = game.player2.as_ref().map(|p| p.id) == Some(pid);

            if !is_player1 && !is_player2 {
                return vec![ServerMessage::Error {
                    message: "Not in game".to_string(),
                }];
            }

            // SECURITY: Prevent ship placement after game has started
            if game.status == GameStatus::Playing
                || game.status == GameStatus::SuddenDeath
                || game.status == GameStatus::Finished
            {
                return vec![ServerMessage::Error {
                    message: "Cannot place ships after game has started".to_string(),
                }];
            }

            // IDEMPOTENCE CHECK
            // Check if already placed WITHOUT borrowing mutable yet
            let already_placed = if is_player1 {
                game.player1.ships_placed
            } else if let Some(ref p) = game.player2 {
                p.ships_placed
            } else {
                false
            };

            if already_placed {
                let player = if is_player1 {
                    &game.player1
                } else if let Some(ref p) = game.player2 {
                    p
                } else {
                    return vec![ServerMessage::Error {
                        message: "Opponent left".to_string(),
                    }];
                };
                return vec![
                    ServerMessage::ShipsConfirmed { player_id: pid },
                    ServerMessage::GameUpdate {
                        status: "Ships Placed".to_string(),
                        is_active: true,
                        heat: player.heat,
                        is_locked: player.is_locked,
                        time_remaining_secs: game.config.game_duration_secs,
                        vetoes_remaining: game.config.max_vetoes.saturating_sub(player.vetoes_used),
                        veto_time_remaining_secs: None,
                        active_problem_contest_id: None,
                        active_problem_index: None,
                        active_problem_name: None,
                    },
                ];
            }

            // ANTI-CHEAT: Validate fleet composition
            // Standard Battleship fleet: Carrier (5), Battleship (4), Cruiser (3), Submarine (3), Destroyer (2)
            const VALID_FLEET: [u8; 5] = [5, 4, 3, 3, 2];
            if ships.len() != 5 {
                return vec![ServerMessage::Error {
                    message: format!("Invalid fleet: expected 5 ships, got {}", ships.len()),
                }];
            }
            let mut ship_sizes: Vec<u8> = ships.iter().map(|s| s.size).collect();
            ship_sizes.sort_unstable();
            ship_sizes.reverse(); // Sort descending to match VALID_FLEET
            if ship_sizes != VALID_FLEET {
                return vec![ServerMessage::Error {
                    message: "Invalid fleet composition. Ships must be sizes 5, 4, 3, 3, 2"
                        .to_string(),
                }];
            }

            // Place ships
            let mut success = true;
            {
                let player = if is_player1 {
                    &mut game.player1
                } else if let Some(ref mut p) = game.player2 {
                    p
                } else {
                    return vec![ServerMessage::Error {
                        message: "Opponent left".to_string(),
                    }];
                };

                // Clear existing state allow retries
                player.ships.clear();
                player.grid = crate::state::Grid::new();

                for placement in ships {
                    let ship = Ship {
                        size: placement.size,
                        hits: 0,
                        sunk: false,
                        x: placement.x,
                        y: placement.y,
                        vertical: placement.vertical,
                    };
                    if player
                        .place_ship(ship, placement.x, placement.y, placement.vertical)
                        .is_err()
                    {
                        success = false;
                        break;
                    }
                }

                if success {
                    player.ships_placed = true;
                }
            } // mutable borrow of player ends here

            if !success {
                return vec![ServerMessage::Error {
                    message: "Invalid ship placement".to_string(),
                }];
            }

            // Now we can safely check both players
            let both_ready = game.player1.ships_placed
                && game
                    .player2
                    .as_ref()
                    .map(|p| p.ships_placed)
                    .unwrap_or(false);

            // IMPORTANT: Broadcast ShipsConfirmed FIRST so opponent knows we placed
            let _ = game.tx.send(crate::state::GameEvent::Message(
                ServerMessage::ShipsConfirmed { player_id: pid },
            ));

            // THEN check if both ready and start game
            if both_ready {
                // Mark as Initializing while we fetch CF solved sets.
                // NOT Playing — the background ticker ignores Initializing games,
                // so the game timer doesn't start counting down during the CF fetch.
                game.status = crate::state::GameStatus::Initializing;
                // DON'T set game_started_at yet — timer starts after CF fetch completes.

                let p1_handle = game.player1.cf_handle.clone();
                let p2_handle = game.player2.as_ref().map(|p| p.cf_handle.clone()).unwrap_or_default();

                // Spawn a background task for the CF fetch so we don't block
                // this WS connection's message loop (ticks keep flowing).
                let state2 = state.clone();
                let gid = game_id;
                tokio::spawn(async move {
                    init_game_from_cf(state2, gid, p1_handle, p2_handle).await;
                });

                // Return immediately — the frontend shows "Initializing" / "Setting up battle..."
                // until GameStart is broadcast by the spawned task.
                return vec![];
            }

            // Only one player has placed — waiting for the other
            let player = if is_player1 {
                &game.player1
            } else if let Some(ref p) = game.player2 {
                p
            } else {
                return vec![ServerMessage::Error {
                    message: "Opponent left".to_string(),
                }];
            };

            vec![ServerMessage::GameUpdate {
                status: "Placement Complete".to_string(),
                is_active: false,
                heat: player.heat,
                is_locked: player.is_locked,
                time_remaining_secs: game.config.game_duration_secs,
                vetoes_remaining: game.config.max_vetoes.saturating_sub(player.vetoes_used),
                veto_time_remaining_secs: None,
                active_problem_contest_id: None,
                active_problem_index: None,
                active_problem_name: None,
            }]
        }

        ClientMessage::Fire { x, y } => {
            let pid = (*player_id).unwrap_or_default();
            if pid == Uuid::default() {
                return vec![ServerMessage::Error {
                    message: "No player ID".to_string(),
                }];
            }

            let mut games = state.games.write().await;
            let game = match games.get_mut(&game_id) {
                Some(g) => g,
                None => {
                    return vec![ServerMessage::Error {
                        message: "Game not found".to_string(),
                    }]
                }
            };

            // CRITICAL: Check game is in Playing or SuddenDeath status
            let is_sudden_death = game.status == GameStatus::SuddenDeath;
            if game.status != GameStatus::Playing && !is_sudden_death {
                return vec![ServerMessage::Error {
                    message: "Game has not started yet. Wait for both players to place ships."
                        .to_string(),
                }];
            }

            let config = game.config.clone();

            let res = if game.player1.id == pid {
                if let Some(ref mut p2) = game.player2 {
                    game.player1
                        .fire(p2, x, y, config.heat_threshold)
                } else {
                    return vec![ServerMessage::Error {
                        message: "Waiting for opponent".to_string(),
                    }];
                }
            } else if game.player2.as_ref().map(|p| p.id) == Some(pid) {
                let p1 = &mut game.player1;
                if let Some(ref mut p2) = game.player2 {
                    p2.fire(p1, x, y, config.heat_threshold)
                } else {
                    return vec![ServerMessage::Error {
                        message: "Opponent missing".to_string(),
                    }];
                }
            } else {
                return vec![ServerMessage::Error {
                    message: "Not in game".to_string(),
                }];
            };

            match res {
                Ok((result, sunk_this_shot, sunk_cells)) => {
                    // Check for victory logic
                    let all_sunk = if game.player1.id == pid {
                        game.player2.as_ref().is_some_and(|p2| {
                            p2.grid
                                .cells
                                .iter()
                                .flatten()
                                .filter(|&&c| c == CellState::Ship)
                                .count()
                                == 0
                        })
                    } else {
                        game.player1
                            .grid
                            .cells
                            .iter()
                            .flatten()
                            .filter(|&&c| c == CellState::Ship)
                            .count()
                            == 0
                    };

                    if all_sunk {
                        game.status = GameStatus::Finished;
                        game.finished_at = Some(std::time::Instant::now());
                    }

                    // sunk_this_shot now comes from fire() - true only if THIS shot sunk a ship

                    let shot_result = ServerMessage::ShotResult {
                        x,
                        y,
                        hit: result == "Hit",
                        sunk: sunk_this_shot,
                        shooter_id: pid,
                        sunk_cells,
                    };

                    // Broadcast to both players
                    let _ = game
                        .tx
                        .send(crate::state::GameEvent::Message(shot_result.clone()));

                    // Check if shooter is now locked and broadcast
                    let shooter_locked = if game.player1.id == pid {
                        game.player1.is_locked
                    } else {
                        game.player2.as_ref().is_some_and(|p| p.is_locked)
                    };
                    if shooter_locked {
                        let _ = game.tx.send(crate::state::GameEvent::Message(
                            ServerMessage::WeaponsLocked { player_id: pid },
                        ));
                    }

                    // If game over (all sunk), broadcast — but ONLY in standard mode.
                    // In SuddenDeath, the SD path below always takes priority
                    // to prevent sending two GameOver messages.
                    if all_sunk && !is_sudden_death {
                        let go_msg = crate::game::build_game_over(game, Some(pid), "AllShipsSunk".to_string());
                        game.game_over_msg = Some(go_msg.clone());
                        let _ = game.tx.send(crate::state::GameEvent::Message(go_msg));
                        crate::discord::log_game(game, Some(pid), "AllShipsSunk");
                    }

                    // SUDDEN DEATH: First hit wins!
                    if is_sudden_death && result == "Hit" {
                        game.status = GameStatus::Finished;
                        game.finished_at = Some(std::time::Instant::now());
                        let go_msg = crate::game::build_game_over(game, Some(pid), "SuddenDeath - First hit wins!".to_string());
                        game.game_over_msg = Some(go_msg.clone());
                        let _ = game.tx.send(crate::state::GameEvent::Message(go_msg));
                        crate::discord::log_game(game, Some(pid), "SuddenDeath");
                    }

                    // Bug 9 fix: Don't return ShotResult directly — broadcast handles it
                    // Returning it here caused the shooter to receive it twice (double toasts)

                    // ── SERVER-SIDE PROBLEM ASSIGNMENT ──
                    // If the shooter just got locked and the game isn't over,
                    // assign next problem from the shared queue.
                    if shooter_locked && game.status != GameStatus::Finished {
                        // Check if already has a problem (shouldn't happen, but be safe)
                        let already_has = if game.player1.id == pid {
                            game.player1.active_problem.is_some()
                        } else {
                            game.player2.as_ref().is_some_and(|p| p.active_problem.is_some())
                        };

                        if !already_has {
                            let is_p1 = game.player1.id == pid;

                            // Draw from shared problem queue
                            let queue_idx = if is_p1 { &mut game.p1_queue_idx } else { &mut game.p2_queue_idx };
                            let assigned = if *queue_idx < game.problem_queue.len() {
                                let ap = game.problem_queue[*queue_idx].clone();
                                *queue_idx += 1;
                                Some(ap)
                            } else {
                                // Queue exhausted — fallback to pick_problem()
                                tracing::warn!("Problem queue exhausted for player {:?}, falling back to pick_problem", pid);
                                let solved_set = if is_p1 {
                                    &game.player1.solved_set
                                } else {
                                    &game.player2.as_ref().unwrap().solved_set
                                };
                                match state.cf_client.pick_problem(
                                    game.config.difficulty,
                                    game.config.difficulty_mode.clone(),
                                    solved_set,
                                ) {
                                    Ok(p) => Some(crate::state::AssignedProblem {
                                        contest_id: p.contest_id,
                                        index: p.index,
                                        name: p.name,
                                        rating: p.rating,
                                    }),
                                    Err(e) => {
                                        tracing::error!("Queue exhausted + pick_problem failed: {}", e);
                                        None
                                    }
                                }
                            };

                            if let Some(ap) = assigned {
                                let tx = game.tx.clone();
                                if is_p1 {
                                    game.player1.active_problem = Some(ap.clone());
                                } else if let Some(ref mut p2) = game.player2 {
                                    p2.active_problem = Some(ap.clone());
                                }
                                let _ = tx.send(crate::state::GameEvent::Message(
                                    ServerMessage::ProblemAssigned {
                                        player_id: pid,
                                        contest_id: ap.contest_id,
                                        problem_index: ap.index,
                                        problem_name: ap.name,
                                        rating: ap.rating,
                                    },
                                ));
                            }
                        }
                    }

                    vec![]
                }
                Err(e) => vec![ServerMessage::Error {
                    message: e.to_string(),
                }],
            }
        }

        ClientMessage::SolveCP {
            contest_id,
            problem_index,
        } => {
            let pid = if let Some(p) = *player_id {
                p
            } else {
                return vec![ServerMessage::Error {
                    message: "No player ID".to_string(),
                }];
            };
            let mut games = state.games.write().await;
            let game = if let Some(g) = games.get_mut(&game_id) {
                g
            } else {
                return vec![ServerMessage::Error {
                    message: "Game not found".to_string(),
                }];
            };

            if game.status == crate::state::GameStatus::Finished {
                return vec![ServerMessage::Error {
                    message: "Game has already ended".to_string(),
                }];
            }

            let player = if game.player1.id == pid {
                &mut game.player1
            } else if game.player2.as_ref().map(|p| p.id) == Some(pid) {
                if let Some(ref mut p) = game.player2 {
                    p
                } else {
                    return vec![ServerMessage::Error {
                        message: "Waiting for opponent".to_string(),
                    }];
                }
            } else {
                return vec![ServerMessage::Error {
                    message: "Not in game".to_string(),
                }];
            };

            // SECURITY: Block solving during active veto
            if player.veto_started_at.is_some() {
                return vec![ServerMessage::Error {
                    message: "Cannot solve during veto penalty. You must wait for the timer."
                        .to_string(),
                }];
            }

            // SECURITY: Block solving when weapons are not locked
            // Without this, a player could freely call SolveCP to inflate problems_solved
            // and get heat/lock reset for free at any time
            if !player.is_locked {
                return vec![ServerMessage::Error {
                    message: "Cannot verify - weapons are not locked".to_string(),
                }];
            }

            // SECURITY: Server is the single source of truth for problem assignment.
            // The player MUST solve the problem the server assigned when weapons locked.
            // No client-side problem selection — prevents pre-solve exploits.
            match &player.active_problem {
                None => {
                    return vec![ServerMessage::Error {
                        message: "No problem assigned yet. Wait for the server to assign one."
                            .to_string(),
                    }];
                }
                Some(assigned) => {
                    if assigned.contest_id != contest_id || assigned.index != problem_index {
                        return vec![ServerMessage::Error {
                            message:
                                "You must solve the assigned problem. Use veto to get a new one."
                                    .to_string(),
                        }];
                    }
                }
            }

            // RATE LIMIT CHECK: 10 seconds cooldown
            if let Some(last) = player.last_verification_attempt {
                if last.elapsed() < std::time::Duration::from_secs(10) {
                    return vec![ServerMessage::Error {
                        message: "Please wait 10 seconds before verifying again.".to_string(),
                    }];
                }
            }
            // Update timestamp
            player.last_verification_attempt = Some(std::time::Instant::now());

            let handle = player.cf_handle.clone();
            let locked_at = player.locked_at_unix;
            let tx = game.tx.clone();
            drop(games); // Drop lock strictly here

            // Broadcast VerifyPending so the frontend shows a spinner
            let _ = tx.send(crate::state::GameEvent::Message(
                ServerMessage::VerifyPending { player_id: pid },
            ));

            // Spawn a background task for the CF API call so this WS
            // connection keeps processing ticks and broadcasts.
            let state2 = state.clone();
            let pidx = problem_index.clone();
            tokio::spawn(async move {
                verify_and_unlock(state2, game_id, pid, handle, contest_id, pidx, locked_at).await;
            });

            vec![]
        }

        ClientMessage::Veto => {
            let pid = if let Some(p) = *player_id {
                p
            } else {
                return vec![ServerMessage::Error {
                    message: "No player ID".to_string(),
                }];
            };
            let mut games = state.games.write().await;
            let game = if let Some(g) = games.get_mut(&game_id) {
                g
            } else {
                return vec![ServerMessage::Error {
                    message: "Game not found".to_string(),
                }];
            };

            if game.status == crate::state::GameStatus::Finished {
                return vec![ServerMessage::Error {
                    message: "Game has already ended".to_string(),
                }];
            }

            let player = if game.player1.id == pid {
                &mut game.player1
            } else if game.player2.as_ref().map(|p| p.id) == Some(pid) {
                if let Some(ref mut p) = game.player2 {
                    p
                } else {
                    return vec![ServerMessage::Error {
                        message: "Waiting for opponent".to_string(),
                    }];
                }
            } else {
                return vec![ServerMessage::Error {
                    message: "Not in game".to_string(),
                }];
            };

            // Check if player is actually locked - can't use veto if not overheated
            if !player.is_locked {
                return vec![ServerMessage::Error {
                    message: "Cannot use veto - weapons are not locked".to_string(),
                }];
            }

            // Check if already on veto timer - can't double veto
            if player.veto_started_at.is_some() {
                return vec![ServerMessage::Error {
                    message: "Already on veto timer. Wait for it to expire.".to_string(),
                }];
            }

            // Check if player has vetoes remaining (use config, not hardcoded 3)
            if player.vetoes_used >= game.config.max_vetoes {
                return vec![ServerMessage::Error {
                    message: "No vetoes remaining".to_string(),
                }];
            }

            // Get veto duration based on current usage count (BEFORE incrementing)
            let veto_durations = game.config.veto_penalties;
            let duration_secs = match veto_durations.get(player.vetoes_used as usize).copied() {
                Some(d) => d,
                None => {
                    return vec![ServerMessage::Error {
                        message: "Invalid veto configuration".to_string(),
                    }]
                }
            };

            // Start veto timer
            player.veto_started_at = Some(std::time::Instant::now());

            // NOW increment vetoes_used
            player.vetoes_used += 1;

            // Clear the current problem — veto means SKIP solving entirely.
            // The player waits out the penalty, then unlock_weapons() is called
            // by the background ticker. No new problem is assigned during veto.
            // When they overheat again later, a new problem will be picked then.
            player.active_problem = None;

            let elapsed = game
                .game_started_at
                .map(|s| s.elapsed().as_secs())
                .unwrap_or(0);
            let game_remaining = game.config.game_duration_secs.saturating_sub(elapsed);

            // vetoes_remaining is now calculated AFTER incrementing
            vec![ServerMessage::GameUpdate {
                status: format!("Veto activated. Wait {} minutes.", duration_secs / 60),
                is_active: false,
                heat: player.heat,
                is_locked: true,
                time_remaining_secs: game_remaining,
                vetoes_remaining: game.config.max_vetoes.saturating_sub(player.vetoes_used),
                veto_time_remaining_secs: Some(duration_secs),
                // Problem cleared — veto skips solving, no new problem assigned
                active_problem_contest_id: None,
                active_problem_index: None,
                active_problem_name: None,
            }]
        }
    }
}

// ---------------------------------------------------------------------------
// Spawned helpers — run in background so the WS loop stays responsive
// ---------------------------------------------------------------------------

/// Fetch both players' solved sets via the CF queue with retry,
/// build the shared problem queue, then transition to Playing.
///
/// Runs as a `tokio::spawn`-ed task.  Uses pre-fetched solved sets when
/// available (spawned at JoinGame time to spread CF load across lobby +
/// placement phases).  Falls back to direct fetch for any sets that
/// weren't pre-fetched in time.  5-minute timeout — refuses to start the
/// game without properly checking both players' submission histories.
async fn init_game_from_cf(
    state: AppState,
    game_id: uuid::Uuid,
    p1_handle: String,
    p2_handle: String,
) {
    // 5-minute timeout — player agreed this is reasonable.  We refuse to
    // start a game without a properly built problem queue.
    let fetch_result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        async {
            // Check what the prefetch tasks have already fetched
            let (p1_prefetched, p2_prefetched) = {
                let games = state.games.read().await;
                match games.get(&game_id) {
                    Some(game) => {
                        let p1 = if game.player1.solved_set_ready {
                            Some(game.player1.solved_set.clone())
                        } else {
                            None
                        };
                        let p2 = game.player2.as_ref().and_then(|p| {
                            if p.solved_set_ready { Some(p.solved_set.clone()) } else { None }
                        });
                        (p1, p2)
                    }
                    None => return None,
                }
            };

            // Fetch only what's missing — prefetched sets save CF API calls
            let p1_set = match p1_prefetched {
                Some(set) => {
                    tracing::info!("init_game_from_cf {:?}: P1 solved set already prefetched ({} problems)", game_id, set.len());
                    set
                }
                None => {
                    tracing::info!("init_game_from_cf {:?}: P1 solved set not ready, fetching now", game_id);
                    fetch_solved_with_retry(&state, &p1_handle).await
                }
            };
            let p2_set = match p2_prefetched {
                Some(set) => {
                    tracing::info!("init_game_from_cf {:?}: P2 solved set already prefetched ({} problems)", game_id, set.len());
                    set
                }
                None => {
                    tracing::info!("init_game_from_cf {:?}: P2 solved set not ready, fetching now", game_id);
                    fetch_solved_with_retry(&state, &p2_handle).await
                }
            };

            Some((p1_set, p2_set))
        },
    ).await;

    let (p1_set, p2_set) = match fetch_result {
        Ok(Some(sets)) => sets,
        Ok(None) => {
            // Game vanished while reading
            return;
        }
        Err(_) => {
            // 5-minute timeout — CF has been unreachable the entire time.
            tracing::error!("init_game_from_cf: 5-min timeout fetching solved sets for game {:?}", game_id);
            let mut games = state.games.write().await;
            if let Some(game) = games.get_mut(&game_id) {
                if game.status == crate::state::GameStatus::Initializing {
                    game.status = crate::state::GameStatus::Finished;
                    game.finished_at = Some(std::time::Instant::now());
                    let go_msg = crate::game::build_game_over(game, None, "CFUnavailable".to_string());
                    game.game_over_msg = Some(go_msg.clone());
                    let _ = game.tx.send(crate::state::GameEvent::Message(go_msg));
                }
            }
            return;
        }
    };

    // Re-acquire the lock and store results
    let mut games = state.games.write().await;
    let game = match games.get_mut(&game_id) {
        Some(g) => g,
        None => {
            tracing::warn!("init_game_from_cf: game {:?} vanished", game_id);
            return;
        }
    };

    // Guard: game was cleaned up or finished while we were fetching
    if game.status != crate::state::GameStatus::Initializing {
        tracing::info!(
            "init_game_from_cf: game {:?} no longer Initializing (status={:?}), aborting",
            game_id, game.status
        );
        return;
    }

    game.player1.solved_set = p1_set;
    game.player1.solved_set_ready = true;
    if let Some(ref mut p2) = game.player2 {
        p2.solved_set = p2_set;
        p2.solved_set_ready = true;
    }
    tracing::info!(
        "Game {:?}: fetched solved sets (P1: {}, P2: {})",
        game_id,
        game.player1.solved_set.len(),
        game.player2.as_ref().map(|p| p.solved_set.len()).unwrap_or(0),
    );

    // Build shared problem queue from union of both solved sets
    {
        let empty_set = std::collections::HashSet::new();
        let p2_solved = game.player2.as_ref()
            .map(|p| &p.solved_set)
            .unwrap_or(&empty_set);
        let queue = state.cf_client.build_shared_queue(
            game.config.difficulty,
            &game.config.difficulty_mode,
            &game.player1.solved_set,
            p2_solved,
            50,
        );
        game.problem_queue = queue.into_iter().map(|p| crate::state::AssignedProblem {
            contest_id: p.contest_id,
            index: p.index,
            name: p.name,
            rating: p.rating,
        }).collect();
        tracing::info!(
            "Game {:?}: built shared queue with {} problems",
            game_id, game.problem_queue.len(),
        );
    }

    // CF data fetched and queue built — NOW start the game.
    game.status = crate::state::GameStatus::Playing;
    game.game_started_at = Some(std::time::Instant::now());

    // Broadcast GameStart to both players
    let _ = game.tx.send(crate::state::GameEvent::Message(ServerMessage::GameStart));
}

/// Pre-fetch a player's solved set in the background.
/// Spawned at JoinGame time to spread CF API load across the lobby
/// and placement phases instead of concentrating it at game start.
/// 4-minute timeout — leaves headroom for init_game_from_cf's 5-min timeout.
async fn prefetch_solved_set(
    state: AppState,
    game_id: uuid::Uuid,
    player_id: uuid::Uuid,
    handle: String,
) {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(240),
        fetch_solved_with_retry(&state, &handle),
    ).await;

    let set = match result {
        Ok(set) => set,
        Err(_) => {
            tracing::warn!(
                "prefetch_solved_set timed out for '{}' in game {:?}",
                handle, game_id
            );
            return; // init_game_from_cf will handle the missing data
        }
    };

    let mut games = state.games.write().await;
    if let Some(game) = games.get_mut(&game_id) {
        if game.player1.id == player_id {
            game.player1.solved_set = set;
            game.player1.solved_set_ready = true;
            tracing::info!(
                "Prefetched P1 ({}) solved set for game {:?}: {} problems",
                handle, game_id, game.player1.solved_set.len()
            );
        } else if let Some(ref mut p2) = game.player2 {
            if p2.id == player_id {
                p2.solved_set = set;
                p2.solved_set_ready = true;
                tracing::info!(
                    "Prefetched P2 ({}) solved set for game {:?}: {} problems",
                    handle, game_id, p2.solved_set.len()
                );
            }
        }
    }
}

/// Fetch a player's solved set via the CF queue with infinite retry.
/// Retries with exponential backoff (capped at 8s) until success.
/// The placement timeout (10 min) is the ultimate safety net.
async fn fetch_solved_with_retry(state: &AppState, handle: &str) -> std::collections::HashSet<String> {
    let mut attempt: u32 = 0;
    loop {
        match state.cf_queue.fetch_solved_set(handle).await {
            Ok(set) => return set,
            Err(e) => {
                attempt += 1;
                let backoff = std::time::Duration::from_secs(2u64.saturating_pow(attempt).min(8));
                tracing::warn!(
                    "fetch_solved_with_retry('{}') attempt {} failed: {} — retrying in {:?}",
                    handle, attempt, e, backoff
                );
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

/// Verify a submission via the CF queue and update game state.
///
/// Runs as a `tokio::spawn`-ed task so the WS loop stays responsive.
/// Broadcasts VerifyResult on failure, WeaponsUnlocked on success.
async fn verify_and_unlock(
    state: AppState,
    game_id: uuid::Uuid,
    pid: uuid::Uuid,
    handle: String,
    contest_id: i32,
    problem_index: String,
    locked_at: Option<u64>,
) {
    // Route through the global CF queue (high priority) with transparent retry.
    // Up to 3 attempts with 3s backoff between retries — absorbs transient CF
    // blips so the player just sees a spinner instead of an error + 10s wait.
    let mut result = Err("no attempt made".to_string());
    for attempt in 0..3u32 {
        result = state.cf_queue.verify_submission(
            &handle, contest_id, &problem_index, locked_at,
        ).await;
        match &result {
            Ok(_) => break,           // Got a definite answer (true or false)
            Err(e) => {
                if attempt < 2 {
                    tracing::warn!(
                        "verify_and_unlock: attempt {} failed for {:?}: {} — retrying",
                        attempt + 1, pid, e
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
            }
        }
    }

    match result {
        Ok(true) => {
            // Re-acquire write lock to update state
            let mut games = state.games.write().await;
            let game = match games.get_mut(&game_id) {
                Some(g) => g,
                None => return,
            };

            // Guard: game may have ended while the CF API call was in-flight
            if game.status == crate::state::GameStatus::Finished {
                return;
            }

            let player = if game.player1.id == pid {
                &mut game.player1
            } else if let Some(ref mut p) = game.player2 {
                p
            } else {
                return;
            };

            // Guard: player may have been unlocked by veto expiry racing with this
            if !player.is_locked {
                return;
            }

            // Add to solved_set so it's never re-assigned this game
            if let Some(ref ap) = player.active_problem {
                let key = format!("{}-{}", ap.contest_id, ap.index);
                player.solved_set.insert(key);
            }

            player.unlock_weapons();
            player.stats.problems_solved += 1;

            // Broadcast WeaponsUnlocked
            let _ = game.tx.send(crate::state::GameEvent::Message(
                ServerMessage::WeaponsUnlocked {
                    player_id: pid,
                    reason: "solved".to_string(),
                },
            ));
        }
        Ok(false) => {
            // Not accepted — broadcast result so frontend shows feedback
            let games = state.games.read().await;
            if let Some(game) = games.get(&game_id) {
                let _ = game.tx.send(crate::state::GameEvent::Message(
                    ServerMessage::VerifyResult {
                        player_id: pid,
                        accepted: false,
                        message: "Submission not accepted yet. Solve it on Codeforces first!".to_string(),
                    },
                ));
            }
        }
        Err(e) => {
            // CF API error — broadcast so player knows to retry
            let games = state.games.read().await;
            if let Some(game) = games.get(&game_id) {
                let _ = game.tx.send(crate::state::GameEvent::Message(
                    ServerMessage::VerifyResult {
                        player_id: pid,
                        accepted: false,
                        message: format!("Codeforces API error: {}. Please retry.", e),
                    },
                ));
            }
        }
    }
}