use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::state::DifficultyMode;

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
        // Client sends this to verify their submission.
        // contest_id and problem_index MUST match the server-assigned problem.
        contest_id: i32,
        problem_index: String,
    },
    Veto,
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
        difficulty_mode: DifficultyMode,
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
        status: String,
        is_active: bool,
        heat: u32,
        is_locked: bool,
        time_remaining_secs: u64,
        vetoes_remaining: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        veto_time_remaining_secs: Option<u64>,
        // Server-assigned problem for the current lock session.
        // NOTE: No skip_serializing_if — None serializes as `null` so the frontend
        // can distinguish "no problem" (null) from "field not present" (undefined).
        // This prevents stale problem data from persisting on the frontend.
        active_problem_contest_id: Option<i32>,
        active_problem_index: Option<String>,
        active_problem_name: Option<String>,
    },
    ShotResult {
        x: usize,
        y: usize,
        hit: bool,
        sunk: bool,
        shooter_id: Uuid,
        /// When sunk=true, the (x,y) coordinates of every cell of the sunk ship.
        /// Frontend uses this to color sunk-ship cells differently from normal hits.
        #[serde(skip_serializing_if = "Option::is_none")]
        sunk_cells: Option<Vec<[usize; 2]>>,
    },
    WeaponsLocked {
        player_id: Uuid, //whatevers players weapons get lcoked
    },
    WeaponsUnlocked {
        player_id: Uuid,
        reason: String,
    },

    /// Server-assigned problem when weapons overheat.
    /// Sent once when the problem is picked; also included in every GameUpdate tick.
    ProblemAssigned {
        player_id: Uuid,
        contest_id: i32,
        problem_index: String,
        problem_name: String,
        rating: u32,
    },

    /// Sent immediately when a SolveCP request enters the CF API queue.
    /// Frontend shows a spinner until VerifyResult or WeaponsUnlocked arrives.
    VerifyPending {
        player_id: Uuid,
    },

    /// Result of a SolveCP verification attempt (sent via broadcast).
    /// Only sent on failure — success is signalled by WeaponsUnlocked instead.
    VerifyResult {
        player_id: Uuid,
        accepted: bool,
        message: String,
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
        // Post-game board reveal: both players' full grids + ship placements.
        // Each cell is "empty", "ship", "hit", or "miss".
        // Ships are serialized as {x, y, size, vertical, sunk}.
        p1_grid: Vec<Vec<String>>,
        p1_ships: Vec<RevealedShip>,
        p2_grid: Vec<Vec<String>>,
        p2_ships: Vec<RevealedShip>,
        // Swiss tiebreaker scores (server-authoritative)
        time_taken_secs: u64,
        winner_score: f64,
        loser_score: f64,
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

/// Ship data sent in the post-game board reveal.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RevealedShip {
    pub x: usize,
    pub y: usize,
    pub size: u8,
    pub vertical: bool,
    pub sunk: bool,
}
//this file describes all the json messages between client and server
