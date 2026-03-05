use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub games: Arc<RwLock<HashMap<Uuid, Game>>>,
    pub cf_client: crate::cf_client::CFClient,
    /// Rate limiter: maps CF handle → (first_request_time, count) for game creation
    pub rate_limiter: Arc<Mutex<HashMap<String, (std::time::Instant, u32)>>>,
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
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Clone, Debug)]
pub enum GameEvent {
    Tick,
    Message(crate::protocol::ServerMessage),
}

/// Which difficulty system to use when picking problems for a locked player.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DifficultyMode {
    /// Standard CF mode: `difficulty` is an exact CF rating (800, 900, … 3500).
    #[default]
    Cf,
    /// Band mode: `difficulty` is a band id (0 = SuperEasy … 4 = VeryHard).
    /// Bands map to clist.by rating ranges (0–300, 301–600, 601–1000, 1001–1500, 1501+).
    Band,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameConfig {
    /// In Cf mode  : exact CF rating (800 / 900 / … / 3500)
    /// In Band mode: band id (0 = SuperEasy, 1 = Easy, 2 = Medium, 3 = Hard, 4 = VeryHard)
    pub difficulty: u32,
    pub difficulty_mode: DifficultyMode,
    pub heat_threshold: u32,      // 5, 7, 10, 15
    pub veto_penalties: [u64; 3], // seconds: 7, 10, 15 min
    pub max_vetoes: u32,
    pub game_duration_secs: u64, // default: 45 * 60 = 2700
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            difficulty: 800,
            difficulty_mode: DifficultyMode::Cf,
            heat_threshold: 7,
            veto_penalties: [420, 600, 900], // 7, 10, 15 minutes
            max_vetoes: 3,
            game_duration_secs: 2700, // 45 minutes (written in seconds)
        }
    }
}

// Player statistics for tie-breaking
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerStats {
    //stats displayed later + win deterimination
    pub ships_sunk: u32,
    pub cells_hit: u32,
    pub cells_missed: u32,
    pub problems_solved: u32,
}

// Tie-break result
#[derive(Clone, Debug, PartialEq)]
pub enum TiebreakResult {
    Player1Wins,
    Player2Wins,
    SuddenDeath,
}

/// A problem assigned by the server when weapons overheat.
/// The server is the single source of truth for problem selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssignedProblem {
    pub contest_id: i32,
    pub index: String,
    pub name: String,
    pub rating: u32,
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
    pub placement_started_at: Option<std::time::Instant>, // When both players joined and placement started
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
    /// Server-assigned problem for the current lock session.
    /// Set by the backend when weapons overheat; cleared on unlock.
    /// The client can only solve THIS problem — no switching.
    #[serde(skip)]
    pub active_problem: Option<AssignedProblem>,
    /// Wall-clock Unix timestamp (seconds) when weapons were locked.
    #[serde(skip)]
    pub locked_at_unix: Option<u64>,
    /// Pre-fetched set of problem keys the player has already solved on CF.
    /// Populated once when the player joins, cleared when the game ends.
    /// Format: "contestId-index" (e.g., "1234-A").
    #[serde(skip)]
    pub solved_set: HashSet<String>,
    /// Whether `solved_set` was successfully fetched from CF API.
    /// When false, problem assignment is deferred until a background retry succeeds.
    #[serde(skip)]
    pub solved_set_fetched: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grid {
    pub cells: [[CellState; 10]; 10], //10x10 grid
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum CellState {
    //stats a cell can have for front end to figure out
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
