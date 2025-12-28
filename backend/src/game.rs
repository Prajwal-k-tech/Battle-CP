use crate::state::{CellState, Game, GameConfig, GameStatus, Grid, Player, PlayerStats, Ship};
use uuid::Uuid; //a custom type for unique ids

#[allow(unused)]
impl Game {
    pub fn new(player1_id: Uuid, player1_handle: String, config: GameConfig) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(100);
        Self {
            id: Uuid::new_v4(),
            player1: Player::new(player1_id, player1_handle),
            player2: None,
            status: GameStatus::Waiting,
            config,
            created_at: std::time::Instant::now(),
            game_started_at: None,
            finished_at: None,
            tx,
        }
    }

    pub fn join(&mut self, player2_id: Uuid, player2_handle: String) -> Result<(), &'static str> {
        if player2_id == self.player1.id {
            return Err("Cannot play against yourself");
        }
        if self.player2.is_some() {
            return Err("Game is full");
        }
        self.player2 = Some(Player::new(player2_id, player2_handle));
        // NOTE: Do NOT set status to Playing here!
        // Game only starts when BOTH players have placed their ships.
        // Status stays as Waiting until then.
        Ok(())
    }
    pub fn determine_winner(&self) -> crate::state::TiebreakResult {
        let p1 = &self.player1;

        // Handle case where P2 might not exist (shouldn't happen at end of game but for safety)
        let p2 = match &self.player2 {
            Some(p) => p,
            None => return crate::state::TiebreakResult::Player1Wins, // P2 forfeit/missing
        };

        // Count surviving ships for each player (ships that are NOT sunk)
        let p1_ships_remaining = p1.ships.iter().filter(|s| !s.sunk).count();
        let p2_ships_remaining = p2.ships.iter().filter(|s| !s.sunk).count();

        // 1. Ships Remaining (most important - your surviving fleet)
        if p1_ships_remaining > p2_ships_remaining {
            return crate::state::TiebreakResult::Player1Wins;
        } else if p2_ships_remaining > p1_ships_remaining {
            return crate::state::TiebreakResult::Player2Wins;
        }

        // 2. Cells Hit (total hits you scored on enemy)
        if p1.stats.cells_hit > p2.stats.cells_hit {
            return crate::state::TiebreakResult::Player1Wins;
        } else if p2.stats.cells_hit > p1.stats.cells_hit {
            return crate::state::TiebreakResult::Player2Wins;
        }

        // 3. Problems Solved
        if p1.stats.problems_solved > p2.stats.problems_solved {
            return crate::state::TiebreakResult::Player1Wins;
        } else if p2.stats.problems_solved > p1.stats.problems_solved {
            return crate::state::TiebreakResult::Player2Wins;
        }

        // 4. Tie -> Sudden Death mode
        crate::state::TiebreakResult::SuddenDeath
    }
}

impl Player {
    pub fn new(id: Uuid, cf_handle: String) -> Self {
        Self {
            id,
            cf_handle,
            grid: Grid::new(),
            ships: vec![],
            heat: 0,
            is_locked: false,
            vetoes_used: 0,
            stats: PlayerStats::default(),
            ships_placed: false,
            veto_started_at: None,
            last_verification_attempt: None,
        }
    }

    pub fn fire(
        &mut self,
        opponent: &mut Player,
        x: usize,
        y: usize,
        heat_threshold: u32,
        veto_penalties: &[u64; 3],
    ) -> Result<(String, bool), &'static str> {
        if self.is_locked {
            // Check veto timer
            if let Some(start) = self.veto_started_at {
                let required_secs = veto_penalties
                    .get(self.vetoes_used.saturating_sub(1) as usize)
                    .copied()
                    .unwrap_or(900);
                if start.elapsed() >= std::time::Duration::from_secs(required_secs) {
                    // Timer expired, unlock!
                    self.is_locked = false;
                    self.heat = 0;
                    // NOTE: vetoes_used already incremented in ws.rs when Veto was activated
                    self.veto_started_at = None;
                } else {
                    let _remaining = required_secs.saturating_sub(start.elapsed().as_secs());
                    // This error string format is tricky without allocation, but we'll return static for now
                    // Ideally return structured error
                    return Err("Weapons Locked! Wait for veto timer.");
                }
            } else {
                return Err("Weapons Locked! Solve CP problem or use Veto.");
            }
        }

        // Process shot on grid
        let mut sunk_this_shot = false;
        let result = opponent.grid.receive_shot(x, y);

        // Update stats
        if result == "Hit" {
            self.stats.cells_hit += 1;

            // Find which ship was hit and update it
            for ship in &mut opponent.ships {
                // Check if (x,y) is part of this ship
                let is_hit = if ship.vertical {
                    x == ship.x && y >= ship.y && y < ship.y + ship.size as usize
                } else {
                    y == ship.y && x >= ship.x && x < ship.x + ship.size as usize
                };

                if is_hit {
                    ship.hits += 1;
                    if ship.hits >= ship.size && !ship.sunk {
                        ship.sunk = true;
                        self.stats.ships_sunk += 1; // Shooter gets credit
                        sunk_this_shot = true;
                    }
                    break;
                }
            }
        } else if result == "Miss" {
            self.stats.cells_missed += 1;
        }

        // Heat Logic: Every valid shot adds +1 heat
        if result == "Hit" || result == "Miss" {
            self.heat += 1;
        }

        // Lock at heat >= threshold
        if self.heat >= heat_threshold {
            self.is_locked = true;
        }

        Ok((result, sunk_this_shot))
    }

    pub fn place_ship(
        &mut self,
        mut ship: Ship,
        x: usize,
        y: usize,
        vertical: bool,
    ) -> Result<(), &'static str> {
        // Validation logic - check BOTH start position and ship end position
        // Grid is 10x10, valid indices are 0-9
        if x >= 10 || y >= 10 {
            return Err("Ship starting position out of bounds");
        }

        // Check ship doesn't extend beyond grid
        let end_x = if vertical { x } else { x + ship.size as usize };
        let end_y = if vertical { y + ship.size as usize } else { y };

        if end_x > 10 || end_y > 10 {
            return Err("Ship extends beyond grid boundary");
        }

        // Check for overlap
        for i in 0..ship.size as usize {
            let (cx, cy) = if vertical { (x, y + i) } else { (x + i, y) };
            if self.grid.cells[cy][cx] != CellState::Empty {
                return Err("Ship overlaps with another ship");
            }
        }

        // Update ship coords
        ship.x = x;
        ship.y = y;
        ship.vertical = vertical;

        // Place ship on grid
        for i in 0..ship.size as usize {
            let (cx, cy) = if vertical { (x, y + i) } else { (x + i, y) };
            self.grid.cells[cy][cx] = CellState::Ship;
        }

        self.ships.push(ship);
        Ok(())
    }

    pub fn unlock_weapons(&mut self) {
        self.is_locked = false;
        self.heat = 0;
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GameConfig, Ship, TiebreakResult};
    use uuid::Uuid;

    fn create_ships() -> Vec<Ship> {
        vec![
            Ship {
                size: 5,
                hits: 0,
                sunk: false,
                x: 0,
                y: 0,
                vertical: false,
            },
            Ship {
                size: 4,
                hits: 0,
                sunk: false,
                x: 0,
                y: 1,
                vertical: false,
            },
            Ship {
                size: 3,
                hits: 0,
                sunk: false,
                x: 0,
                y: 2,
                vertical: false,
            },
        ]
    }

    #[test]
    fn test_determine_winner() {
        let p1_id = Uuid::new_v4();
        let p2_id = Uuid::new_v4();
        let config = GameConfig::default();
        let mut game = Game::new(p1_id, "p1".to_string(), config);
        game.join(p2_id, "p2".to_string()).unwrap();

        // Add ships to both players
        game.player1.ships = create_ships();
        game.player2.as_mut().unwrap().ships = create_ships();

        // Case 1: P1 has more ships remaining (fewer sunk)
        game.player1.ships[0].sunk = false; // 3 remaining
        game.player2.as_mut().unwrap().ships[0].sunk = true; // 2 remaining
        assert_eq!(game.determine_winner(), TiebreakResult::Player1Wins);

        // Case 2: Ships equal, P2 has more hits
        game.player1.ships[0].sunk = true; // Both have 2 remaining
        game.player2.as_mut().unwrap().ships[0].sunk = true;
        game.player1.stats.cells_hit = 5;
        game.player2.as_mut().unwrap().stats.cells_hit = 10;
        assert_eq!(game.determine_winner(), TiebreakResult::Player2Wins);

        // Case 3: Ships and hits equal, P1 solved more problems
        game.player1.stats.cells_hit = 10;
        game.player2.as_mut().unwrap().stats.cells_hit = 10;
        game.player1.stats.problems_solved = 1;
        game.player2.as_mut().unwrap().stats.problems_solved = 0;
        assert_eq!(game.determine_winner(), TiebreakResult::Player1Wins);

        // Case 4: Complete Tie -> Sudden Death
        game.player1.stats.problems_solved = 0;
        game.player2.as_mut().unwrap().stats.problems_solved = 0;
        assert_eq!(game.determine_winner(), TiebreakResult::SuddenDeath);
    }
}
