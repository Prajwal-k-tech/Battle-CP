use backend::state::AppState;
use backend::state::Game;
use uuid::Uuid;

/// Test heat logic: +1 per shot, lock at 7
#[tokio::test]
async fn test_heat_accumulation_and_lock() {
    // 1. Initialize State
    let state = AppState::new();
    let player1_id = Uuid::new_v4();
    let _player2_id = Uuid::new_v4();
    let new_game = Game::new(
        player1_id,
        "player1".to_string(),
        backend::state::GameConfig::default(),
    );
    let game_id = new_game.id;

    // 2. Insert Game
    state.games.write().await.insert(game_id, new_game);

    // 3. Verify Initial Heat is 0
    {
        let games = state.games.read().await;
        let game = games.get(&game_id).unwrap();
        assert_eq!(game.player1.heat, 0, "Initial heat should be 0");
        assert!(!game.player1.is_locked, "Should not be locked initially");
    }

    // 4. Fire 6 shots (should not lock yet - threshold is 7)
    // Need dummy opponent
    let mut opponent = backend::state::Player::new(Uuid::new_v4(), "opponent".to_string());

    for i in 0..6 {
        let mut games = state.games.write().await;
        let game = games.get_mut(&game_id).unwrap();
        // Fire at dummy opponent
        let result = game.player1.fire(
            &mut opponent,
            i % 10,
            0,
            game.config.heat_threshold,
            &game.config.veto_penalties,
        );
        assert!(result.is_ok(), "Shot {} should succeed", i);
    }

    {
        let games = state.games.read().await;
        let game = games.get(&game_id).unwrap();
        assert_eq!(game.player1.heat, 6, "Heat should be 6 after 6 shots");
        assert!(!game.player1.is_locked, "Should NOT be locked at heat 6");
    }

    // 5. Fire 7th shot - should lock
    {
        let mut games = state.games.write().await;
        let game = games.get_mut(&game_id).unwrap();
        let result = game.player1.fire(
            &mut opponent,
            6,
            0,
            game.config.heat_threshold,
            &game.config.veto_penalties,
        );
        assert!(result.is_ok(), "7th shot should succeed");
    }

    {
        let games = state.games.read().await;
        let game = games.get(&game_id).unwrap();
        assert_eq!(game.player1.heat, 7, "Heat should be 7 after 7 shots");
        assert!(game.player1.is_locked, "Should BE locked at heat 7");
    }

    // 5. Attempt to fire while locked - should fail
    {
        let mut games = state.games.write().await;
        let game = games.get_mut(&game_id).unwrap();
        let result = game.player1.fire(
            &mut opponent,
            7,
            0,
            game.config.heat_threshold,
            &game.config.veto_penalties,
        );
        assert!(result.is_err(), "Shot should fail when locked");
    }

    // 6. Unlock weapons and verify can fire again
    {
        let mut games = state.games.write().await;
        let game = games.get_mut(&game_id).unwrap();
        game.player1.unlock_weapons();
        assert_eq!(game.player1.heat, 0, "Heat should reset to 0 after unlock");
        assert!(
            !game.player1.is_locked,
            "Should be unlocked after unlock_weapons"
        );
    }

    {
        let mut games = state.games.write().await;
        let game = games.get_mut(&game_id).unwrap();
        let result = game.player1.fire(
            &mut opponent,
            8,
            0,
            game.config.heat_threshold,
            &game.config.veto_penalties,
        );
        assert!(result.is_ok(), "Should be able to fire after unlock");
    }
}
