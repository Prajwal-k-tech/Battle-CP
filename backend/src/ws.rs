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
    ws.on_upgrade(move |socket| handle_socket(socket, game_id, query.player_id, state))
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

    // Subscribe to game events
    let rx = {
        let games = state.games.read().await;
        games.get(&game_id).map(|g| {
            println!(
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
            println!("[WS] Game {:?} not found!", game_id);
            let _ = sender
                .send(Message::Text(
                    serde_json::to_string(&ServerMessage::Error {
                        message: "Game not found".to_string(),
                    })
                    .unwrap(),
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
                                let responses = handle_client_message(
                                    client_msg,
                                    &mut player_id,
                                    game_id,
                                    &state,
                                ).await;

                                for resp in responses {
                                    let resp_text = serde_json::to_string(&resp).unwrap();
                                    if sender.send(Message::Text(resp_text)).await.is_err() {
                                        println!("[WS] Failed to send response, closing connection");
                                        break 'main_loop;
                                    }
                                }
                            } else {
                                println!("[WS] Failed to parse message: {}", text);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        println!("[WS] Receive error: {:?}", e);
                        break 'main_loop;
                    }
                    None => {
                        println!("[WS] Client disconnected");
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
                                                    let _ = sender.send(Message::Text(msg_text)).await;
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
                                                status: format!("{:?}", game.status),
                                                is_active: true,
                                                heat: p.heat,
                                                is_locked: p.is_locked,
                                                time_remaining_secs: remaining,
                                                vetoes_remaining: game.config.max_vetoes.saturating_sub(p.vetoes_used),
                                                veto_time_remaining_secs: veto_time_remaining,
                                            };
                                            let resp_text = serde_json::to_string(&update).unwrap();
                                            if sender.send(Message::Text(resp_text)).await.is_err() {
                                                println!("[WS] Failed to send tick update, closing connection");
                                                break 'main_loop;
                                            }
                                        }
                                    }
                                }
                            }
                            crate::state::GameEvent::Message(msg) => {
                                 let resp_text = serde_json::to_string(&msg).unwrap();
                                 if sender.send(Message::Text(resp_text)).await.is_err() {
                                     println!("[WS] Failed to send broadcast message, closing connection");
                                     break 'main_loop;
                                 }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        println!("[WS] Broadcast receiver lagged by {} messages", n);
                        // Continue - we can recover from lag
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        println!("[WS] Broadcast channel closed, ending connection");
                        break 'main_loop;
                    }
                }
            }
        }
    }
    println!(
        "[WS] Connection handler exiting for player {:?} in game {:?}",
        player_id, game_id
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
            *player_id = Some(pid);
            let mut games = state.games.write().await;
            if let Some(game) = games.get_mut(&game_id) {
                // Check if game is finished - reject all join attempts
                if game.status == crate::state::GameStatus::Finished {
                    return vec![ServerMessage::Error {
                        message: "Game has already ended".to_string(),
                    }];
                }

                // Check if player is already in the game (Reconnect)
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
                    msgs.push(ServerMessage::GameUpdate {
                        status: format!("{:?}", game.status),
                        is_active: true,
                        heat: player.heat,
                        is_locked: player.is_locked,
                        time_remaining_secs: remaining,
                        vetoes_remaining: game.config.max_vetoes.saturating_sub(player.vetoes_used),
                        veto_time_remaining_secs: None,
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

                    // 4. If game started (both placed), send GameStart and Grids
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

                    return msgs;
                }

                if game.player1.id != pid && game.player2.is_none() {
                    // P2 is joining - get P1's ID before joining
                    let p1_id = game.player1.id;

                    // Verify CF Handle exists immediately (Security Fix)
                    let handle_exists = state
                        .cf_client
                        .verify_user_exists(&cf_handle)
                        .await
                        .unwrap_or(false); // Fail closed if API error

                    if !handle_exists {
                        return vec![ServerMessage::Error {
                            message: format!("Codeforces handle '{}' not found", cf_handle),
                        }];
                    }

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
                    println!(
                        "[WS] Broadcast PlayerJoined for {:?} - result: {:?}, subscribers: {}",
                        pid,
                        result.is_ok(),
                        game.tx.receiver_count()
                    );

                    // Return GameJoined + PlayerJoined for P1 to the joining Guest
                    // This tells Guest that the opponent (Host) already exists
                    return vec![
                        ServerMessage::GameJoined {
                            game_id,
                            player_id: pid,
                            difficulty: game.config.difficulty,
                            max_heat: game.config.heat_threshold,
                            max_vetoes: game.config.max_vetoes,
                        },
                        ServerMessage::PlayerJoined { player_id: p1_id },
                    ];
                }
                vec![ServerMessage::GameJoined {
                    game_id,
                    player_id: pid,
                    difficulty: game.config.difficulty,
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
                // Set game status to Playing BEFORE broadcasting GameStart
                game.status = crate::state::GameStatus::Playing;
                game.game_started_at = Some(std::time::Instant::now());

                // Broadcast GameStart to all players
                let _ = game
                    .tx
                    .send(crate::state::GameEvent::Message(ServerMessage::GameStart));
            }

            // Get player again for response
            let player = if is_player1 {
                &game.player1
            } else if let Some(ref p) = game.player2 {
                p
            } else {
                return vec![ServerMessage::Error {
                    message: "Opponent left".to_string(),
                }];
            };

            // Return status based on whether game started
            let status = if both_ready {
                "Playing".to_string()
            } else {
                "Placement Complete".to_string()
            };

            vec![ServerMessage::GameUpdate {
                status,
                is_active: both_ready,
                heat: player.heat,
                is_locked: player.is_locked,
                time_remaining_secs: game.config.game_duration_secs,
                vetoes_remaining: game.config.max_vetoes.saturating_sub(player.vetoes_used),
                veto_time_remaining_secs: None,
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
                        .fire(p2, x, y, config.heat_threshold, &config.veto_penalties)
                } else {
                    return vec![ServerMessage::Error {
                        message: "Waiting for opponent".to_string(),
                    }];
                }
            } else if game.player2.as_ref().map(|p| p.id) == Some(pid) {
                let p1 = &mut game.player1;
                if let Some(ref mut p2) = game.player2 {
                    p2.fire(p1, x, y, config.heat_threshold, &config.veto_penalties)
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
                Ok((result, sunk_this_shot)) => {
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
                            ServerMessage::WeaponsLocked,
                        ));
                    }

                    // If game over (all sunk), broadcast
                    if all_sunk {
                        // Get stats for the shooter (pid)
                        let shooter_stats = if game.player1.id == pid {
                            game.player1.stats.clone()
                        } else {
                            game.player2
                                .as_ref()
                                .map(|p| p.stats.clone())
                                .unwrap_or_default()
                        };
                        let _ = game.tx.send(crate::state::GameEvent::Message(
                            ServerMessage::GameOver {
                                winner_id: Some(pid),
                                reason: "AllShipsSunk".to_string(),
                                your_shots_hit: shooter_stats.cells_hit,
                                your_shots_missed: shooter_stats.cells_missed,
                                your_ships_sunk: shooter_stats.ships_sunk,
                                your_problems_solved: shooter_stats.problems_solved,
                            },
                        ));
                    }

                    // SUDDEN DEATH: First hit wins!
                    if is_sudden_death && result == "Hit" {
                        game.status = GameStatus::Finished;
                        game.finished_at = Some(std::time::Instant::now());
                        let shooter_stats = if game.player1.id == pid {
                            game.player1.stats.clone()
                        } else {
                            game.player2
                                .as_ref()
                                .map(|p| p.stats.clone())
                                .unwrap_or_default()
                        };
                        let _ = game.tx.send(crate::state::GameEvent::Message(
                            ServerMessage::GameOver {
                                winner_id: Some(pid),
                                reason: "SuddenDeath - First hit wins!".to_string(),
                                your_shots_hit: shooter_stats.cells_hit,
                                your_shots_missed: shooter_stats.cells_missed,
                                your_ships_sunk: shooter_stats.ships_sunk,
                                your_problems_solved: shooter_stats.problems_solved,
                            },
                        ));
                    }

                    vec![shot_result]
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

            // CRITICAL: Block solving during active veto
            if player.veto_started_at.is_some() {
                return vec![ServerMessage::Error {
                    message: "Cannot solve during veto penalty. You must wait for the timer."
                        .to_string(),
                }];
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
            drop(games); // Drop lock strictly here

            let verify_result = state
                .cf_client
                .verify_submission(&handle, contest_id, &problem_index)
                .await;

            match verify_result {
                Ok(true) => {
                    // Re-acquire write lock to update state
                    let mut games = state.games.write().await;
                    if let Some(game) = games.get_mut(&game_id) {
                        let player = if game.player1.id == pid {
                            &mut game.player1
                        } else if let Some(ref mut p) = game.player2 {
                            p
                        } else {
                            return vec![];
                        };
                        player.unlock_weapons();
                        player.stats.problems_solved += 1;

                        // Broadcast WeaponsUnlocked
                        let _ = game.tx.send(crate::state::GameEvent::Message(
                            ServerMessage::WeaponsUnlocked {
                                reason: "solved".to_string(),
                            },
                        ));

                        let elapsed = game
                            .game_started_at
                            .map(|s| s.elapsed().as_secs())
                            .unwrap_or(0);
                        let remaining = game.config.game_duration_secs.saturating_sub(elapsed);

                        vec![ServerMessage::GameUpdate {
                            status: "Weapons Unlocked!".to_string(),
                            is_active: true,
                            heat: player.heat,
                            is_locked: player.is_locked,
                            time_remaining_secs: remaining,
                            vetoes_remaining: game
                                .config
                                .max_vetoes
                                .saturating_sub(player.vetoes_used),
                            veto_time_remaining_secs: None,
                        }]
                    } else {
                        vec![]
                    }
                }
                Ok(false) => vec![ServerMessage::Error {
                    message: "Submission not accepted".to_string(),
                }],
                Err(e) => vec![ServerMessage::Error {
                    message: e.to_string(),
                }],
            }
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
                is_locked: true, // Still locked during veto
                time_remaining_secs: game_remaining,
                vetoes_remaining: game.config.max_vetoes.saturating_sub(player.vetoes_used),
                veto_time_remaining_secs: Some(duration_secs),
            }]
        }
    }
}
