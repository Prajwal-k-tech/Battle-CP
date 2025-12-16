use crate::protocol::ServerMessage;
use crate::state::{AppState, GameEvent, GameStatus, TiebreakResult};
use tokio::time::{sleep, Duration};

pub async fn start_global_ticker(state: AppState) {
    loop {
        sleep(Duration::from_secs(1)).await;

        let mut games = state.games.write().await;
        for game in games.values_mut() {
            // Periodic State Sync (Every 1 second for perfect timer sync)
            let _ = game.tx.send(GameEvent::Tick);

            if game.status == GameStatus::Playing {
                // Check veto timer expiry for both players
                let veto_durations = [420u64, 600, 900]; // 7, 10, 15 minutes

                // Check player 1 veto expiry
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

                // Check player 2 veto expiry
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

                // Check game timeout
                if let Some(start) = game.game_started_at {
                    if start.elapsed().as_secs() >= game.config.game_duration_secs {
                        // Time Over! Determine winner or enter sudden death
                        let winner_result = game.determine_winner();

                        // Determine what happens based on TiebreakResult
                        match winner_result {
                            TiebreakResult::Player1Wins => {
                                game.status = GameStatus::Finished;
                                let _ = game.tx.send(GameEvent::Message(ServerMessage::GameOver {
                                    winner_id: Some(game.player1.id),
                                    reason: "Timeout - More ships remaining".to_string(),
                                }));
                            }
                            TiebreakResult::Player2Wins => {
                                game.status = GameStatus::Finished;
                                let _ = game.tx.send(GameEvent::Message(ServerMessage::GameOver {
                                    winner_id: game.player2.as_ref().map(|p| p.id),
                                    reason: "Timeout - More ships remaining".to_string(),
                                }));
                            }
                            TiebreakResult::SuddenDeath => {
                                // Enter sudden death mode - first hit wins!
                                game.status = GameStatus::SuddenDeath;
                                // Unlock both players' weapons for sudden death
                                game.player1.unlock_weapons();
                                if let Some(ref mut p2) = game.player2 {
                                    p2.unlock_weapons();
                                }
                                let _ =
                                    game.tx.send(GameEvent::Message(ServerMessage::GameUpdate {
                                        status: "SUDDEN DEATH! First hit wins!".to_string(),
                                        your_turn: true,
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
    }
}
