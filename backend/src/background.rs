use crate::protocol::ServerMessage;
use crate::state::{AppState, GameEvent, GameStatus, TiebreakResult};
use tokio::time::{sleep, Duration};
//main game loop / server handling multiple game states at a time
pub async fn start_global_ticker(state: AppState) {
    //our async global ticker, keep passing the app state
    loop {
        //infinite loop in rust
        sleep(Duration::from_secs(1)).await; //1 tick  is 1 second
        let mut games = state.games.write().await;
        for game in games.values_mut() {
            // Periodic State Sync (Every 1 second for perfect timer sync)
            let _ = game.tx.send(GameEvent::Tick);
            if game.status == GameStatus::Playing || game.status == GameStatus::SuddenDeath {
                // Check veto timer expiry for both players
                let veto_durations = game.config.veto_penalties;

                //Check player 1 veto expiry
                if game.player1.is_locked {
                    if let Some(veto_start) = game.player1.veto_started_at {
                        let duration = veto_durations
                            .get(game.player1.vetoes_used.saturating_sub(1) as usize)
                            .copied()
                            .unwrap_or(900);
                        if veto_start.elapsed().as_secs() >= duration {
                            game.player1.unlock_weapons();
                            game.player1.veto_started_at = None;
                            let _ =
                                game.tx
                                    .send(GameEvent::Message(ServerMessage::WeaponsUnlocked {
                                        reason: "veto_expired".to_string(),
                                    }));
                        }
                    }
                }

                //Check player 2 veto expiry
                if let Some(ref mut p2) = game.player2 {
                    if p2.is_locked {
                        if let Some(veto_start) = p2.veto_started_at {
                            let duration = veto_durations
                                .get(p2.vetoes_used.saturating_sub(1) as usize)
                                .copied()
                                .unwrap_or(900);
                            if veto_start.elapsed().as_secs() >= duration {
                                p2.unlock_weapons();
                                p2.veto_started_at = None;
                                let _ = game.tx.send(GameEvent::Message(
                                    ServerMessage::WeaponsUnlocked {
                                        reason: "veto_expired".to_string(),
                                    },
                                ));
                            }
                        }
                    }
                }

                //Check game timeout
                if let Some(start) = game.game_started_at {
                    // Only check for initial timeout if we are clearly in Playing state
                    // If we are already in SuddenDeath, we ignore the standard game duration
                    if game.status == GameStatus::Playing
                        && start.elapsed().as_secs() >= game.config.game_duration_secs
                    {
                        //Time Over! Determine winner or enter sudden death
                        let winner_result = game.determine_winner();

                        //Determine what happens based on TiebreakResult
                        match winner_result {
                            TiebreakResult::Player1Wins => {
                                game.status = GameStatus::Finished;
                                game.finished_at = Some(std::time::Instant::now());
                                let _ = game.tx.send(GameEvent::Message(ServerMessage::GameOver {
                                    winner_id: Some(game.player1.id),
                                    reason: "Timeout - More ships remaining".to_string(),
                                    // Stats for player1 (winner)
                                    your_shots_hit: game.player1.stats.cells_hit,
                                    your_shots_missed: game.player1.stats.cells_missed,
                                    your_ships_sunk: game.player1.stats.ships_sunk,
                                    your_problems_solved: game.player1.stats.problems_solved,
                                }));
                            }
                            TiebreakResult::Player2Wins => {
                                game.status = GameStatus::Finished;
                                game.finished_at = Some(std::time::Instant::now());
                                let p2_stats = game
                                    .player2
                                    .as_ref()
                                    .map(|p| p.stats.clone())
                                    .unwrap_or_default();
                                let _ = game.tx.send(GameEvent::Message(ServerMessage::GameOver {
                                    winner_id: game.player2.as_ref().map(|p| p.id),
                                    reason: "Timeout - More ships remaining".to_string(),
                                    your_shots_hit: p2_stats.cells_hit,
                                    your_shots_missed: p2_stats.cells_missed,
                                    your_ships_sunk: p2_stats.ships_sunk,
                                    your_problems_solved: p2_stats.problems_solved,
                                }));
                            }
                            TiebreakResult::SuddenDeath => {
                                //sudden death to break ties
                                game.status = GameStatus::SuddenDeath;
                                //Unlock both players' weapons for sudden death
                                game.player1.unlock_weapons();
                                if let Some(ref mut p2) = game.player2 {
                                    p2.unlock_weapons();
                                }
                                let _ =
                                    game.tx.send(GameEvent::Message(ServerMessage::GameUpdate {
                                        status: "SUDDEN DEATH! First hit wins!".to_string(),
                                        is_active: true,
                                        heat: 0,
                                        is_locked: false,
                                        time_remaining_secs: 0,
                                        vetoes_remaining: 0,
                                        veto_time_remaining_secs: None,
                                    }));
                            }
                        }
                    }
                }
            }
        }

        // CLEANUP: Remove games that finished more than 5 minutes ago OR are waiting > 30 mins
        let finished_cleanup_threshold = std::time::Duration::from_secs(300); // 5 minutes after finish
        let waiting_cleanup_threshold = std::time::Duration::from_secs(1800); // 30 minutes if waiting

        games.retain(|_id, game| {
            if let Some(finished) = game.finished_at {
                // If finished, keep only if within threshold
                finished.elapsed() < finished_cleanup_threshold
            } else if game.status == GameStatus::Waiting {
                // If waiting, keep only if within threshold
                game.created_at.elapsed() < waiting_cleanup_threshold
            } else {
                // Keep active/playing games
                true
            }
        });
    }
}
