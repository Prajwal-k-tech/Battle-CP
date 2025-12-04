use crate::state::{CellState, Game, GameStatus, Grid, Player};
use uuid::Uuid;

impl Game {
    pub fn new(player1_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            player1: Player::new(player1_id),
            player2: None,
            status: GameStatus::Waiting,
        }
    }

    pub fn join(&mut self, player2_id: Uuid) -> Result<(), &'static str> {
        if self.player2.is_some() {
            return Err("Game is full");
        }
        self.player2 = Some(Player::new(player2_id));
        self.status = GameStatus::Playing; // In real app, wait for ship placement
        Ok(())
    }
}

impl Player {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            grid: Grid::new(),
            ships: vec![], // Ships would be placed here
            ammo: 5,
            heat: 0,
            is_locked: false,
        }
    }

    pub fn fire(
        &mut self,
        opponent_grid: &mut Grid,
        x: usize,
        y: usize,
    ) -> Result<String, &'static str> {
        if self.is_locked {
            return Err("Weapons Locked! Solve CP problem.");
        }
        if self.ammo == 0 {
            return Err("No Ammo!");
        }

        // Deduct ammo
        self.ammo -= 1;

        // Add heat (Every shot counts)
        self.heat += 1;
        if self.heat >= 7 {
            self.is_locked = true;
        }

        // Process shot
        let result = opponent_grid.receive_shot(x, y);
        Ok(result)
    }

    pub fn regen_ammo(&mut self) {
        if self.ammo < 5 {
            self.ammo += 1;
        }
    }

    pub fn unlock_weapons(&mut self) {
        self.is_locked = false;
        self.heat = 0;
    }
}

impl Grid {
    pub fn new() -> Self {
        Self {
            cells: [[CellState::Empty; 10]; 10],
        }
    }

    pub fn receive_shot(&mut self, x: usize, y: usize) -> String {
        if x >= 10 || y >= 10 {
            return "Out of bounds".to_string();
        }

        match self.cells[y][x] {
            CellState::Empty => {
                self.cells[y][x] = CellState::Miss;
                "Miss".to_string()
            }
            CellState::Ship => {
                self.cells[y][x] = CellState::Hit;
                "Hit".to_string()
            }
            _ => "Already fired here".to_string(),
        }
    }
}
