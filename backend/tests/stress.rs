use backend::state::{AppState, Game, GameConfig, GameStatus};
use backend::background;
use tokio::time::sleep;
use uuid::Uuid;

// Stress test: spawn 50 games, mark them Playing, then perform many concurrent
// fire() calls across all games to exercise per-game write locks, ticker, and
// general concurrency. This test does NOT call the Codeforces API.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn stress_50_games_fire_load() {
    let state = AppState::new();

    // Start the global ticker which the server normally runs.
    tokio::spawn(background::start_global_ticker(state.clone()));

    // Create 50 games with two players each and place them into Playing state.
    let mut game_ids = Vec::new();
    for i in 0..50 {
        let p1_id = Uuid::new_v4();
        let p2_id = Uuid::new_v4();
        let mut g = Game::new(p1_id, format!("p1_{}", i), GameConfig::default());
        g.join(p2_id, format!("p2_{}", i)).unwrap();

        // Mark as placed and start playing immediately.
        g.player1.ships_placed = true;
        if let Some(ref mut p2) = g.player2 { p2.ships_placed = true; }
        g.status = GameStatus::Playing;
        g.game_started_at = Some(std::time::Instant::now());

        let id = g.id;
        state.games.write().await.insert(id, g);
        game_ids.push(id);
    }

    // For each game spawn a task that performs 200 shots (alternating shooters)
    // using internal API `fire()` to avoid WebSocket overhead.
    let mut handles = Vec::new();
    for gid in game_ids.iter().cloned() {
        let st = state.clone();
        let h = tokio::spawn(async move {
            for s in 0..200usize {
                // choose coordinates to avoid repeated 'Already fired here' errors
                let x = (s % 10) as usize;
                let y = ((s / 10) % 10) as usize;

                // Acquire write lock briefly and perform one shot
                {
                    let mut games = st.games.write().await;
                    if let Some(game) = games.get_mut(&gid) {
                        let _ = if s % 2 == 0 {
                            if let Some(ref mut p2) = game.player2 {
                                game.player1.fire(p2, x, y, game.config.heat_threshold)
                            } else {
                                Err("missing opponent")
                            }
                        } else {
                            // p2 shoots at p1
                            if let Some(ref mut p2) = game.player2 {
                                p2.fire(&mut game.player1, x, y, game.config.heat_threshold)
                            } else {
                                Err("missing opponent")
                            }
                        };
                    }
                }

                // tiny sleep to spread load, but still intense
                sleep(std::time::Duration::from_millis(1)).await;
            }
        });
        handles.push(h);
    }

    // Await all tasks with a timeout to ensure we don't hang forever in CI
    for h in handles {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(30), h).await;
    }

    // Sanity check: all games still present
    let games = state.games.read().await;
    assert_eq!(games.len(), 50);
}
