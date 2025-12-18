use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub games: Arc<RwLock<HashMap<Uuid, Game>>>,
    pub cf_client: crate::cf_client::CFClient,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            games: Arc::new(RwLock::new(HashMap::new())),
            cf_client: crate::cf_client::CFClient::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum GameEvent {
    Tick,
    Message(crate::protocol::ServerMessage),
}

/// Game configuration with sensible defaults
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameConfig {
    pub difficulty: u32,          // CP problem rating: 700-1200
    pub heat_threshold: u32,      // 5, 7, 10, 15
    pub veto_penalties: [u64; 3], // seconds: 7, 10, 15 min
    pub max_vetoes: u32,
    pub game_duration_secs: u64, // default: 45 * 60 = 2700
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            difficulty: 800,
            heat_threshold: 7,
            veto_penalties: [420, 600, 900], // 7, 10, 15 minutes
            max_vetoes: 3,
            game_duration_secs: 45 * 60, // 45 minutes
        }
    }
}

/// Player statistics for tie-breaking
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerStats {
    pub ships_sunk: u32,
    pub cells_hit: u32,
    pub problems_solved: u32,
}

/// Tie-break result
#[derive(Clone, Debug, PartialEq)]
pub enum TiebreakResult {
    Player1Wins,
    Player2Wins,
    SuddenDeath,
}

#[derive(Debug, Serialize)]
pub struct Game {
    pub id: Uuid,
    pub player1: Player,
    pub player2: Option<Player>,
    pub status: GameStatus,
    pub config: GameConfig,
    #[serde(skip)]
    pub created_at: std::time::Instant, // When lobby was created (for cleanup)
    #[serde(skip)]
    pub game_started_at: Option<std::time::Instant>,
    #[serde(skip)]
    pub finished_at: Option<std::time::Instant>, // For auto-cleanup
    #[serde(skip)]
    pub tx: broadcast::Sender<GameEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GameStatus {
    Waiting,      // Waiting for P2 to join
    PlacingShips, // Both players joined, placing ships
    Playing,      // Both placed ships, combat phase
    SuddenDeath,  // Tiebreaker: first hit wins
    Finished,     // Game over
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub id: Uuid,
    pub cf_handle: String,
    pub grid: Grid,
    pub ships: Vec<Ship>,
    pub heat: u32,
    pub is_locked: bool,
    pub vetoes_used: u32,
    pub stats: PlayerStats,
    pub ships_placed: bool,
    #[serde(skip)]
    pub veto_started_at: Option<std::time::Instant>,
    #[serde(skip)]
    pub last_verification_attempt: Option<std::time::Instant>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grid {
    pub cells: [[CellState; 10]; 10],
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum CellState {
    Empty,
    Ship,
    Hit,
    Miss,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ship {
    pub size: u8,
    pub hits: u8,
    pub sunk: bool,
    pub x: usize,
    pub y: usize,
    pub vertical: bool,
}
