use crate::protocol::ServerMessage;
use crate::state::{AppState, GameEvent, GameStatus, TiebreakResult}; //our app state 
use tokio::time::{sleep, Duration};
//main game loop / server handling multiple game states at a timeR
pub async fn start_global_ticker(state: AppState) {
    //our async global ticker, keep passing the app state
    loop {
        sleep(Duration::from_secs(1)).await; //1 tick  is 1 second
        let mut games = state.games.write().await;
        for game in games.values_mut() {
            // Periodic State Sync (Every 1 second for perfect timer sync)
            let _ = game.tx.send(GameEvent::Tick);

            if game.status == GameStatus::Waiting {
                if game.created_at.elapsed() >= std::time::Duration::from_secs(300) { //if you waited for more than 5 minutes
                    game.status = GameStatus::Finished;
                    game.finished_at = Some(std::time::Instant::now());
                    let go_msg = crate::game::build_game_over(game, None, "LobbyTimeout".to_string());
                    game.game_over_msg = Some(go_msg.clone());
                    let _ = game.tx.send(GameEvent::Message(go_msg));
                    tracing::info!("Game {:?} lobby timed out (5 min)", game.id);
                }
            }

            // Placement timeout: 10 minutes from when placement actually started (P2 joined).
            // Using placement_started_at (not created_at) guarantees a full 10 minutes
            // regardless of how long the lobby waited for P2.
            if game.status == GameStatus::PlacingShips {
                if let Some(placement_start) = game.placement_started_at {
                    if placement_start.elapsed() >= std::time::Duration::from_secs(600) {
                        game.status = GameStatus::Finished;
                        game.finished_at = Some(std::time::Instant::now());
                        let go_msg = crate::game::build_game_over(game, None, "PlacementTimeout".to_string());
                        game.game_over_msg = Some(go_msg.clone());
                        let _ = game.tx.send(GameEvent::Message(go_msg));
                        tracing::info!("Game {:?} placement timed out (10 min)", game.id);
                    }
                }
            }
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
                            game.player1.unlock_weapons(); // also clears veto_started_at
                            let _ =
                                game.tx
                                    .send(GameEvent::Message(ServerMessage::WeaponsUnlocked {
                                        player_id: game.player1.id,
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
                                let p2_id = p2.id;
                                p2.unlock_weapons(); // also clears veto_started_at
                                let _ = game.tx.send(GameEvent::Message(
                                    ServerMessage::WeaponsUnlocked {
                                        player_id: p2_id,
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
                                let winner = Some(game.player1.id);
                                let go_msg = crate::game::build_game_over(game, winner, "Timeout - More ships remaining".to_string());
                                game.game_over_msg = Some(go_msg.clone());
                                let _ = game.tx.send(GameEvent::Message(go_msg));
                            }
                            TiebreakResult::Player2Wins => {
                                game.status = GameStatus::Finished;
                                game.finished_at = Some(std::time::Instant::now());
                                let winner = game.player2.as_ref().map(|p| p.id);
                                let go_msg = crate::game::build_game_over(game, winner, "Timeout - More ships remaining".to_string());
                                game.game_over_msg = Some(go_msg.clone());
                                let _ = game.tx.send(GameEvent::Message(go_msg));
                            }
                            TiebreakResult::SuddenDeath => {
                                // Sudden Death: first player to land a HIT wins.
                                // No player state changes on entry — heat locks, veto timers,
                                // and unlock requirements ALL carry over unchanged.
                                // The Tick handler propagates per-player state every second,
                                // advertising "SuddenDeath" status to both clients.
                                game.status = GameStatus::SuddenDeath;
                            }
                        }
                    }

                    // SUDDEN DEATH TIMEOUT: 10 minutes max to prevent infinite games
                    // (e.g., both players locked with no vetoes remaining)
                    const SUDDEN_DEATH_TIMEOUT_SECS: u64 = 600; // 10 minutes
                    if game.status == GameStatus::SuddenDeath
                        && start.elapsed().as_secs()
                            >= game.config.game_duration_secs + SUDDEN_DEATH_TIMEOUT_SECS
                    {
                        game.status = GameStatus::Finished;
                        game.finished_at = Some(std::time::Instant::now());
                        let go_msg = crate::game::build_game_over(game, None, "SuddenDeathTimeout".to_string());
                        game.game_over_msg = Some(go_msg.clone());
                        let _ = game.tx.send(GameEvent::Message(go_msg));
                        tracing::info!("Game {:?} sudden death timed out (10 min)", game.id);
                    }
                }
            }
        }

        // CLEANUP: Remove games that:
        // - Finished more than 5 minutes ago
        // - Are waiting > 30 mins
        // - Are placing ships > 30 mins (player joined but never placed)
        let finished_cleanup_threshold = std::time::Duration::from_secs(300); // 5 minutes after finish
        let waiting_cleanup_threshold = std::time::Duration::from_secs(1800); // 30 minutes if waiting
        let placing_cleanup_threshold = std::time::Duration::from_secs(1800); // 30 minutes if placing ships

        let removed = games.len();
        games.retain(|_id, game| {
            if let Some(finished) = game.finished_at {
                // If finished, keep only if within threshold
                finished.elapsed() < finished_cleanup_threshold
            } else if game.status == GameStatus::Waiting {
                // If waiting for P2, keep only if within threshold
                game.created_at.elapsed() < waiting_cleanup_threshold
            } else if game.status == GameStatus::PlacingShips {
                // If stuck in placement phase, clean up after threshold from when placement started
                game.placement_started_at
                    .map(|ps| ps.elapsed() < placing_cleanup_threshold)
                    .unwrap_or_else(|| game.created_at.elapsed() < placing_cleanup_threshold)
            } else {
                // Keep active/playing games (Playing, SuddenDeath)
                true
            }
        });
        let removed = removed - games.len();
        if removed > 0 {
            tracing::info!("Cleaned up {} finished/abandoned games ({} remaining)", removed, games.len());
        }
        // Drop write lock before rate limiter cleanup
        drop(games);

        // RATE LIMITER CLEANUP: Purge expired entries every 60 seconds
        // to prevent unbounded memory growth during tournament
        static LAST_LIMITER_CLEANUP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last = LAST_LIMITER_CLEANUP.load(std::sync::atomic::Ordering::Relaxed);
        if now_secs - last >= 60 {
            LAST_LIMITER_CLEANUP.store(now_secs, std::sync::atomic::Ordering::Relaxed);
            let mut limiter = state.rate_limiter.lock().await;
            let before = limiter.len();
            let window = std::time::Duration::from_secs(300);
            limiter.retain(|_, (created, _)| created.elapsed() < window);
            let purged = before - limiter.len();
            if purged > 0 {
                tracing::debug!("Purged {} expired rate limiter entries", purged);
            }
        }
    }
}
