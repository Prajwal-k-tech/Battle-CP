use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub games: Arc<RwLock<HashMap<Uuid, Game>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            games: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub id: Uuid,
    pub player1: Player,
    pub player2: Option<Player>,
    pub status: GameStatus,
    // Add more fields as needed
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GameStatus {
    Waiting,
    Playing,
    Finished,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub id: Uuid,
    pub grid: Grid,
    pub ships: Vec<Ship>,
    pub ammo: u32,
    pub heat: u32,
    pub is_locked: bool,
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
    // Position details would go here
}
