// Game State Types for BattleCP

export type CellState = "empty" | "ship" | "hit" | "miss";

export interface ShipPlacement {
    x: number;
    y: number;
    size: number;
    vertical: boolean;
}

export type GamePhase = "connecting" | "lobby" | "placement" | "combat" | "finished";

export interface GameState {
    phase: GamePhase;
    gameId: string | null;
    playerId: string | null;
    opponentId: string | null;
    opponentConnected: boolean;

    // Placement / Reconnection
    myShipsPlaced: boolean;
    opponentShipsPlaced: boolean;
    myShips: ShipPlacement[]; // Bug 8: Ships data for CombatGrid rendering after reconnect

    // Combat
    myGrid: CellState[][];      // 10x10 - your ships + opponent hits
    enemyGrid: CellState[][];   // 10x10 - your hits/misses on enemy

    // HUD State
    heat: number;
    maxHeat: number;
    maxVetoes: number;
    isLocked: boolean;
    vetoesRemaining: number;
    vetoTimeRemaining: number | null;
    gameTimeRemaining: number;
    difficulty: number;
    status: string;

    // Stats
    problemsSolved: number;
    enemyShipsSunk: number;

    // End State
    winnerId: string | null;
    gameOverReason: string | null;

    // Error
    lastError: string | null;

    // Server-committed problem for the current lock session.
    // When set, ProblemPanel MUST use this instead of picking a random problem.
    activeProblemContestId: number | null;
    activeProblemIndex: string | null;
}

export const initialGameState: GameState = {
    phase: "connecting",
    gameId: null,
    playerId: null,
    opponentId: null,
    opponentConnected: false,

    myShipsPlaced: false,
    opponentShipsPlaced: false,
    myShips: [],

    myGrid: Array(10).fill(null).map(() => Array(10).fill("empty")),
    enemyGrid: Array(10).fill(null).map(() => Array(10).fill("empty")),

    heat: 0,
    maxHeat: 7, // Default, will be updated from server
    maxVetoes: 3, // Default, will be updated from server
    isLocked: false,
    vetoesRemaining: 3,
    vetoTimeRemaining: null,
    gameTimeRemaining: 45 * 60, // 45 minutes
    status: "Connecting...",

    problemsSolved: 0,
    enemyShipsSunk: 0,

    winnerId: null,
    gameOverReason: null,
    lastError: null,
    difficulty: 800,
    activeProblemContestId: null,
    activeProblemIndex: null,
};

// Client -> Server Messages
export type ClientMessage =
    | { type: "JoinGame"; player_id: string; cf_handle: string }
    | { type: "PlaceShips"; ships: ShipPlacement[] }
    | { type: "Fire"; x: number; y: number }
    | { type: "SolveCP"; contest_id: number; problem_index: string }
    | { type: "CommitProblem"; contest_id: number; problem_index: string }
    | { type: "Veto" };

// Server -> Client Messages
export type ServerMessage =
    // Lobby
    | { type: "GameJoined"; game_id: string; player_id: string; difficulty: number; max_heat: number; max_vetoes: number }
    | { type: "PlayerJoined"; player_id: string }

    // Placement
    | { type: "ShipsConfirmed"; player_id: string }
    | { type: "GameStart" }

    // Reconnection
    | { type: "YourShips"; ships: ShipPlacement[] }
    | { type: "GridSync"; my_grid: CellState[][]; enemy_grid: CellState[][] }

    // Combat
    | { type: "GameUpdate"; status: string; is_active: boolean; heat: number; is_locked: boolean; time_remaining_secs: number; vetoes_remaining: number; veto_time_remaining_secs?: number; active_problem_contest_id?: number; active_problem_index?: string }
    | { type: "ShotResult"; x: number; y: number; hit: boolean; sunk: boolean; shooter_id: string }
    | { type: "WeaponsLocked"; player_id: string }
    | { type: "WeaponsUnlocked"; player_id: string; reason: string } // "solved" or "veto_expired"

    // End
    | {
        type: "GameOver";
        winner_id: string | null;
        reason: string;
        // Authoritative stats from server — use these to display correct numbers
        p1_id: string;
        p1_ships_sunk: number;
        p1_cells_hit: number;
        p1_problems_solved: number;
        p2_ships_sunk: number;
        p2_cells_hit: number;
        p2_problems_solved: number;
    }

    // Error
    | { type: "Error"; message: string };
