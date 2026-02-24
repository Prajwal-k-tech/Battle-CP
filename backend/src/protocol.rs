use serde::{Deserialize, Serialize};
use uuid::Uuid;
//read
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    JoinGame {
        //join a game + players cf handle
        player_id: Uuid,
        cf_handle: String,
    },
    PlaceShips {
        //placement of ships
        ships: Vec<ShipPlacement>,
    },
    Fire {
        //where in the grid you fire
        x: usize,
        y: usize,
    },
    SolveCP {
        //contest id + problem index shows which q to solve
        contest_id: i32,
        problem_index: String,
    },
    Veto, //veto request, type only
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ServerMessage {
    //handles server messages
    GameJoined {
        //player successfully joined the game + lobby settings
        game_id: Uuid,
        player_id: Uuid,
        difficulty: u32,
        max_heat: u32,
        max_vetoes: u32,
    },
    PlayerJoined {
        player_id: Uuid,
    },
    //Placement Phase
    ShipsConfirmed {
        player_id: Uuid,
    },
    GameStart,

    //Combat Phase
    GameUpdate {
        //every tick you get an update on these game stats
        status: String,
        is_active: bool,
        heat: u32,
        is_locked: bool,
        time_remaining_secs: u64,
        vetoes_remaining: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        veto_time_remaining_secs: Option<u64>,
        // Committed problem for this lock session — sent so client can restore after reconnect.
        // None when weapons are not locked or no problem committed yet.
        #[serde(skip_serializing_if = "Option::is_none")]
        active_problem_contest_id: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        active_problem_index: Option<String>,
    },
    ShotResult {
        x: usize,
        y: usize,
        hit: bool,
        sunk: bool,
        shooter_id: Uuid,
    },
    WeaponsLocked {
        player_id: Uuid, //whatevers players weapons get lcoked
    },
    WeaponsUnlocked {
        player_id: Uuid,
        reason: String, //either "solved" or "veto"
    },

    GameOver {
        winner_id: Option<Uuid>,
        reason: String,
        // Full stats for both players — each client reads their own by player_id
        p1_id: Uuid,
        p1_ships_sunk: u32,
        p1_cells_hit: u32,
        p1_problems_solved: u32,
        p2_ships_sunk: u32,
        p2_cells_hit: u32,
        p2_problems_solved: u32,
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
//this file describes all the json messages between client and server
