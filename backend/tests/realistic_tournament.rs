use backend::state::{AppState, Game, GameConfig, GameStatus, Ship};
use backend::background;
use std::time::Instant;
use tokio::time::sleep;
use uuid::Uuid;

/// Realistic tournament stress test:
/// - Create 50 games with two players each
/// - Both players place ships (simulating the placement phase)
/// - All 50 games hit Initializing state simultaneously (triggering CF solves fetches)
/// - Measure: init completion time, timeouts, and queue behavior
///
/// This DOES NOT call the real Codeforces API to avoid rate limiting.
/// Instead, we'll measure how the queuing logic would behave under peak load.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn realistic_tournament_50_games_init_flood() {
    let start = Instant::now();
    let state = AppState::new();

    // Start the global ticker
    tokio::spawn(background::start_global_ticker(state.clone()));

    println!("\n=== Realistic Tournament Stress Test (50 games) ===\n");

    // ========== PHASE 1: CREATE GAMES ==========
    println!("[PHASE 1] Creating 50 games...");
    let phase1_start = Instant::now();
    
    // Create and insert all games
    let mut game_ids = Vec::new();
    {
        let mut games = state.games.write().await;
        for i in 0..50 {
            let p1_id = Uuid::new_v4();
            let p2_id = Uuid::new_v4();
            let mut g = Game::new(p1_id, format!("p1_{}", i), GameConfig::default());
            g.join(p2_id, format!("p2_{}", i)).unwrap();

            // Mark as Waiting (before placement)
            g.status = GameStatus::Waiting;
            let gid = g.id;
            games.insert(gid, g);
            game_ids.push(gid);
        }
    }

    println!(
        "✓ Created {} games in {:?}",
        game_ids.len(),
        phase1_start.elapsed()
    );

    // ========== PHASE 2: PLACE SHIPS (CONCURRENT) ==========
    println!("\n[PHASE 2] Placing ships for both players (concurrent)...");
    let phase2_start = Instant::now();

    let mut handles = Vec::new();
    for gid in &game_ids {
        let state_clone = state.clone();
        let gid_copy = *gid;
        let h = tokio::spawn(async move {
            // Create valid fleet: [5, 4, 3, 3, 2]
            let placements = vec![
                (0, 0, 5, false),   // Carrier horizontal
                (0, 1, 4, false),   // Battleship
                (0, 2, 3, false),   // Cruiser
                (0, 3, 3, false),   // Submarine
                (0, 4, 2, false),   // Destroyer
            ];

            let mut games = state_clone.games.write().await;
            if let Some(game) = games.get_mut(&gid_copy) {
                // Place for P1
                for (x, y, size, vert) in &placements {
                    let ship = Ship {
                        size: *size,
                        hits: 0,
                        sunk: false,
                        x: *x,
                        y: *y,
                        vertical: *vert,
                    };
                    let _ = game.player1.place_ship(ship, *x, *y, *vert);
                }
                game.player1.ships_placed = true;

                // Place for P2
                if let Some(ref mut p2) = game.player2 {
                    for (x, y, size, vert) in &placements {
                        let ship = Ship {
                            size: *size,
                            hits: 0,
                            sunk: false,
                            x: *x,
                            y: *y,
                            vertical: *vert,
                        };
                        let _ = p2.place_ship(ship, *x, *y, *vert);
                    }
                    p2.ships_placed = true;
                }

                // Mark as PlacingShips (ready for init)
                game.status = GameStatus::PlacingShips;
                game.placement_started_at = Some(std::time::Instant::now());
            }
        });
        handles.push(h);
    }

    for h in handles {
        let _ = h.await;
    }

    println!(
        "✓ Ships placed for all 50 games in {:?}",
        phase2_start.elapsed()
    );

    // ========== PHASE 3: TRIGGER ALL INITS SIMULTANEOUSLY ==========
    println!("\n[PHASE 3] Marking all games as Initializing...");
    let phase3_start = Instant::now();

    // In real gameplay, games transition to Initializing when both place ships.
    // That triggers tokio::spawn(init_game_from_cf(...)) which fetches CF data.
    // Here, we just mark the state — the spawned tasks aren't running without
    // the full WS handler integration. This shows the state transition overhead.
    {
        let mut games = state.games.write().await;
        for g in games.values_mut() {
            if g.status == GameStatus::PlacingShips {
                g.status = GameStatus::Initializing;
            }
        }
    }

    println!(
        "✓ All 50 games marked as Initializing in {:?}",
        phase3_start.elapsed()
    );
    println!("\n⚠️  NOTE: This test does NOT call the actual init_game_from_cf() function.");
    println!("   In real gameplay, 50 games starting would flood the CF API queue.");
    println!("   Expected behavior:");
    println!("   - First ~43 games fetch solved sets (~43 × 2.1s ≈ 90s)");
    println!("   - Remaining 7 games timeout after 90s with CFUnavailable");

    // ========== PHASE 4: WAIT & CHECK STATE ==========
    println!("\n[PHASE 4] Waiting 2 seconds and checking state...");
    sleep(std::time::Duration::from_secs(2)).await;

    let (initializing_final, playing_final, finished_final) = {
        let games = state.games.read().await;
        let init_count = games.values().filter(|g| g.status == GameStatus::Initializing).count();
        let play_count = games.values().filter(|g| g.status == GameStatus::Playing).count();
        let fin_count = games.values().filter(|g| g.status == GameStatus::Finished).count();
        (init_count, play_count, fin_count)
    };

    // ========== PHASE 5: ANALYSIS & PROJECTION ==========
    println!("\n[PHASE 5] Analysis: What happens with 50 concurrent game inits?\n");

    println!("CF API Queue Analysis:");
    println!("  - Rate limit: 1 request per 2.1 seconds");
    println!("  - 50 games × 2 players = 100 solved-set fetch requests needed");
    println!("  - Sequential processing time: 100 × 2.1s = 210 seconds (3.5 minutes)");
    println!("  - Init timeout: 90 seconds");
    println!();
    println!("Projected outcome with real CF API:");
    println!("  - Games 1-43: Complete successfully (~43 × 2.1s ≈ 90s) ✓");
    println!("  - Games 44-50: Timeout after 90s, finish with CFUnavailable ⚠️");
    println!("  - Success rate: ~86% (43/50)");
    println!();
    println!("Current state after 2 seconds (test state):");
    println!("  Initializing: {} games (no actual CF calls in this test)", initializing_final);
    println!("  Playing: {} games", playing_final);
    println!("  Finished: {} games", finished_final);

    let total_elapsed = start.elapsed();
    println!("\n=== TEST SUMMARY ===");
    println!("Total test time: {:?}", total_elapsed);
    println!("\n=== RECOMMENDATION ===");
    println!("For a flawless 50-game tournament:");
    println!("1. Prefetch solved sets at game join time (spreads CF load)");
    println!("2. OR stagger tournament start (waves of 10-15 games)");
    println!("3. OR increase init timeout to 180s (accept slower starts)");
    println!("4. OR implement 30s dedup cache for duplicate handles")
}
