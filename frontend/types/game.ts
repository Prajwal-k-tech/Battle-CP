// Game State Types for BattleCP

export type CellState = "empty" | "ship" | "hit" | "miss";

export interface ShipPlacement {
    x: number;
    y: number;
    size: number;
    vertical: boolean;
}

export interface RevealedShip {
    x: number;
    y: number;
    size: number;
    vertical: boolean;
    sunk: boolean;
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

    // Sunk ship tracking — accumulated "x,y" keys for cells belonging to fully sunk ships
    enemySunkCells: string[];   // sunk cells on enemy grid (my shots that sunk ships)
    mySunkCells: string[];      // sunk cells on my grid (opponent shots that sunk my ships)

    // HUD State
    heat: number;
    maxHeat: number;
    maxVetoes: number;
    isLocked: boolean;
    vetoesRemaining: number;
    vetoTimeRemaining: number | null;
    gameTimeRemaining: number;
    difficulty: number;
    difficulty_mode: "cf" | "band";
    status: string;

    // Stats
    problemsSolved: number;
    enemyShipsSunk: number;

    // End State
    winnerId: string | null;
    gameOverReason: string | null;

    // Board reveal (populated on GameOver — already resolved to my/opponent perspective)
    revealMyGrid: string[][] | null;
    revealMyShips: RevealedShip[] | null;
    revealOpponentGrid: string[][] | null;
    revealOpponentShips: RevealedShip[] | null;
    // Opponent stats from GameOver
    opponentShipsSunk: number;
    opponentProblemsSolved: number;
    opponentCellsHit: number;
    myCellsHit: number;

    // Swiss tiebreaker (from GameOver, server-computed)
    gameTimeSecs: number | null;  // seconds the combat phase lasted
    myScore: number | null;       // winner: (L-t)/L ∈ [0,1]  |  loser: (t-L)/L ∈ [-1,0]

    // Error
    lastError: string | null;

    // Server-assigned problem for the current lock session.
    // The server is the single source of truth — no client-side problem selection.
    activeProblemContestId: number | null;
    activeProblemIndex: string | null;
    activeProblemName: string | null;
    activeProblemRating: number | null;
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

    enemySunkCells: [],
    mySunkCells: [],

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

    revealMyGrid: null,
    revealMyShips: null,
    revealOpponentGrid: null,
    revealOpponentShips: null,
    opponentShipsSunk: 0,
    opponentProblemsSolved: 0,
    opponentCellsHit: 0,
    myCellsHit: 0,

    gameTimeSecs: null,
    myScore: null,

    lastError: null,
    difficulty: 800,
    difficulty_mode: "cf",
    activeProblemContestId: null,
    activeProblemIndex: null,
    activeProblemName: null,
    activeProblemRating: null,
};

// Client -> Server Messages
export type ClientMessage =
    | { type: "JoinGame"; player_id: string; cf_handle: string }
    | { type: "PlaceShips"; ships: ShipPlacement[] }
    | { type: "Fire"; x: number; y: number }
    | { type: "SolveCP"; contest_id: number; problem_index: string }
    | { type: "Veto" };

// Server -> Client Messages
export type ServerMessage =
    // Lobby
    | { type: "GameJoined"; game_id: string; player_id: string; difficulty: number; difficulty_mode: "cf" | "band"; max_heat: number; max_vetoes: number }
    | { type: "PlayerJoined"; player_id: string }

    // Placement
    | { type: "ShipsConfirmed"; player_id: string }
    | { type: "GameStart" }

    // Reconnection
    | { type: "YourShips"; ships: ShipPlacement[] }
    | { type: "GridSync"; my_grid: CellState[][]; enemy_grid: CellState[][] }

    // Combat
    | { type: "GameUpdate"; status: string; is_active: boolean; heat: number; is_locked: boolean; time_remaining_secs: number; vetoes_remaining: number; veto_time_remaining_secs?: number; active_problem_contest_id?: number; active_problem_index?: string; active_problem_name?: string }
    | { type: "ShotResult"; x: number; y: number; hit: boolean; sunk: boolean; shooter_id: string; sunk_cells?: [number, number][] }
    | { type: "WeaponsLocked"; player_id: string }
    | { type: "WeaponsUnlocked"; player_id: string; reason: string } // "solved" or "veto_expired"

    // Server-assigned problem
    | { type: "ProblemAssigned"; player_id: string; contest_id: number; problem_index: string; problem_name: string; rating: number }

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
        // Board reveal
        p1_grid: string[][];
        p1_ships: RevealedShip[];
        p2_grid: string[][];
        p2_ships: RevealedShip[];
        // Swiss tiebreaker (server-computed, authoritative)
        time_taken_secs: number;
        winner_score: number;
        loser_score: number;
    }

    // Error
    | { type: "Error"; message: string };
