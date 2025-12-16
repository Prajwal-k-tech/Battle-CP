use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    // Lobby Phase
    GameJoined {
        game_id: Uuid,
        player_id: Uuid,
        difficulty: u32,
    },
    PlayerJoined {
        player_id: Uuid,
    },

    // Placement Phase
    ShipsConfirmed {
        player_id: Uuid,
    },
    GameStart,

    // Combat Phase
    GameUpdate {
        status: String,
        your_turn: bool,
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
    WeaponsLocked,
    WeaponsUnlocked {
        reason: String, // "solved" or "veto_expired"
    },

    // Game End
    GameOver {
        winner_id: Option<Uuid>,
        reason: String,
    },

    // Errors
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShipPlacement {
    pub x: usize,
    pub y: usize,
    pub size: u8,
    pub vertical: bool,
}
