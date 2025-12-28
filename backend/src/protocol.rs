use serde::{Deserialize, Serialize}; //for json
use uuid::Uuid; //creation of uuids
                //contains our server messages
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    JoinGame {
        player_id: Uuid,
        cf_handle: String,
    },
    PlaceShips {
        ships: Vec<ShipPlacement>,
    },
    Fire {
        x: usize,
        y: usize,
    },
    SolveCP {
        contest_id: i32,
        problem_index: String,
    },
    Veto,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ServerMessage {
    GameJoined {
        //joined the game
        game_id: Uuid,
        player_id: Uuid,
        difficulty: u32, // problem difficulty for the game
        max_heat: u32,   // heat threshold before weapons lock
        max_vetoes: u32, // total vetoes allowed
    },
    PlayerJoined {
        //player joined the game
        player_id: Uuid,
    },

    //Placement Phase
    ShipsConfirmed {
        //player confirmed their ships
        player_id: Uuid,
    },
    GameStart, //Game started

    //Combat Phase
    GameUpdate {
        status: String,
        is_active: bool,
        heat: u32,
        is_locked: bool,
        time_remaining_secs: u64,
        vetoes_remaining: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        veto_time_remaining_secs: Option<u64>,
    },
    ShotResult {
        x: usize,
        y: usize,
        hit: bool,
        sunk: bool,
        shooter_id: Uuid,
    },
    WeaponsLocked {
        player_id: Uuid, // So frontend can filter by player
    },
    WeaponsUnlocked {
        player_id: Uuid, // So frontend can filter by player
        reason: String,  // "solved" or "veto_expired"
    },

    // Game End
    GameOver {
        winner_id: Option<Uuid>,
        reason: String,
        // Stats for the receiving player
        your_shots_hit: u32,
        your_shots_missed: u32,
        your_ships_sunk: u32,
        your_problems_solved: u32,
    },

    // Errors
    Error {
        message: String,
    },

    // Reconnection State
    YourShips {
        ships: Vec<ShipPlacement>,
    },
    GridSync {
        my_grid: Vec<Vec<String>>,    // "empty", "ship", "hit", "miss"
        enemy_grid: Vec<Vec<String>>, // "empty", "hit", "miss" (ships hidden)
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShipPlacement {
    pub x: usize,
    pub y: usize,
    pub size: u8,
    pub vertical: bool,
}
