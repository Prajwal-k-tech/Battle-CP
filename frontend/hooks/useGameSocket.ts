"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import {
    GameState,
    initialGameState,
    ServerMessage,
    ShipPlacement,
} from "@/types/game";
import { toast } from "sonner";

const WS_BASE_URL = process.env.NEXT_PUBLIC_WS_URL || "ws://localhost:3000";

export function useGameSocket(gameId: string, playerId: string, cfHandle: string) {
    const [gameState, setGameState] = useState<GameState>(initialGameState);
    const [isConnected, setIsConnected] = useState(false);
    const [gameNotFound, setGameNotFound] = useState(false); // Track if game doesn't exist
    const wsRef = useRef<WebSocket | null>(null);
    const isConnecting = useRef(false); // Guard against double connections
    const reconnectAttempts = useRef(0);
    const shouldStopReconnect = useRef(false); // Prevent reconnection when game not found
    const maxReconnectAttempts = 5;

    // Handle incoming server messages
    const handleServerMessage = useCallback((msg: ServerMessage) => {
        // console.log("[WS] Received:", msg); // Keep cleanLogs

        switch (msg.type) {
            case "GameJoined": {
                let wasConnecting = false;
                setGameState(prev => {
                    // ONLY transition to lobby if we're in connecting state
                    // Don't reset from placement/combat on reconnection
                    wasConnecting = prev.phase === "connecting";

                    return {
                        ...prev,
                        phase: wasConnecting ? "lobby" : prev.phase,
                        gameId: msg.game_id,
                        playerId: msg.player_id,
                        difficulty: msg.difficulty,
                        difficulty_mode: msg.difficulty_mode,
                        maxHeat: msg.max_heat,
                        maxVetoes: msg.max_vetoes,
                        vetoesRemaining: msg.max_vetoes, // Initialize from server config
                        status: wasConnecting ? "Waiting for opponent..." : prev.status,
                    };
                });
                // Note: wasConnecting is updated during setState, so this will show toast on initial connect
                if (wasConnecting) {
                    toast.success("Connected to game lobby", { id: "game-connected" });
                }
                break;
            }

            case "PlayerJoined":
                setGameState(prev => {
                    // Only show toast if opponent wasn't already connected (prevent duplicate toasts)
                    const isNewOpponent = !prev.opponentConnected;
                    const shouldAdvance = prev.phase === "lobby" || prev.phase === "connecting";

                    // Only toast on initial connection, not reconnects
                    if (isNewOpponent) {
                        toast.info("Opponent connected!", { id: "opponent-connected" });
                    }

                    return {
                        ...prev,
                        opponentId: msg.player_id,
                        opponentConnected: true,
                        phase: shouldAdvance ? "placement" : prev.phase,
                        status: "Deploy your fleet",
                    };
                });
                break;

            case "ShipsConfirmed":
                setGameState(prev => {
                    const isMe = msg.player_id === prev.playerId;
                    return {
                        ...prev,
                        myShipsPlaced: isMe ? true : prev.myShipsPlaced,
                        opponentShipsPlaced: !isMe ? true : prev.opponentShipsPlaced,
                        status: isMe ? "Waiting for opponent to deploy..." : prev.status,
                    };
                });
                break;

            case "GameStart":
                setGameState(prev => ({
                    ...prev,
                    phase: "combat",
                    status: "COMBAT ACTIVE",
                }));
                toast.success("Battle commencing!", { id: "battle-start" });
                break;

            case "GameUpdate":
                setGameState(prev => {
                    let newPhase = prev.phase;

                    // SECURITY: Only process GameUpdates if we have a confirmed playerId
                    // This prevents third players (who got rejected) from transitioning phases
                    if (!prev.playerId) return prev;

                    // SAFETY: Detect game ended from status if GameOver was missed (broadcast lag)
                    if (msg.status === "Finished" && prev.phase !== "finished") {
                        // Game ended but we missed the GameOver message.
                        // Transition to finished to prevent stuck state.
                        localStorage.removeItem("battlecp_active_game");
                        return {
                            ...prev,
                            phase: "finished" as const,
                            status: "GAME OVER",
                            winnerId: null,
                            gameOverReason: "Game ended",
                        };
                    }

                    // Only transit to combat if we are actually playing
                    // Trust server status — the server only reports "Playing"/"SuddenDeath"
                    // after both players have placed ships, so we don't need to guard on
                    // myShipsPlaced/opponentShipsPlaced (which may not be set yet on reconnect).
                    if ((msg.status === "Playing" || msg.status.includes("SUDDEN DEATH")) &&
                        (prev.phase === "connecting" || prev.phase === "lobby" || prev.phase === "placement")) {
                        newPhase = "combat";
                    }
                    // IMPORTANT: Don't change phase otherwise - preserve placement/combat phases

                    return {
                        ...prev,
                        heat: msg.heat,
                        isLocked: msg.is_locked,
                        gameTimeRemaining: msg.time_remaining_secs,
                        vetoesRemaining: msg.vetoes_remaining,
                        vetoTimeRemaining: msg.veto_time_remaining_secs ?? null,
                        status: msg.status,
                        phase: newPhase,
                        // Server-assigned problem — authoritative source of truth.
                        // The server always includes these fields (null when no problem).
                        // Use explicit undefined check: if field is present (even null), use it;
                        // only fall back to prev if the field is truly absent (non-GameUpdate source).
                        activeProblemContestId: msg.active_problem_contest_id !== undefined
                            ? msg.active_problem_contest_id ?? null
                            : prev.activeProblemContestId,
                        activeProblemIndex: msg.active_problem_index !== undefined
                            ? msg.active_problem_index ?? null
                            : prev.activeProblemIndex,
                        activeProblemName: msg.active_problem_name !== undefined
                            ? msg.active_problem_name ?? null
                            : prev.activeProblemName,
                    };
                });
                break;

            case "ShotResult":
                setGameState(prev => {
                    const isMyShot = msg.shooter_id === prev.playerId;

                    // Side effect for toast - purely visual, can use the derived value
                    if (msg.sunk) {
                        toast.info(isMyShot ? "Enemy ship destroyed!" : "Your ship was sunk!");
                    }

                    // Accumulate sunk cells when a ship is sunk
                    const newSunkCells = msg.sunk && msg.sunk_cells
                        ? msg.sunk_cells.map(([cx, cy]) => `${cx},${cy}`)
                        : [];

                    if (isMyShot) {
                        // Update enemy grid with my shot result
                        const newEnemyGrid = prev.enemyGrid.map(row => [...row]);
                        newEnemyGrid[msg.y][msg.x] = msg.hit ? "hit" : "miss";
                        return {
                            ...prev,
                            enemyGrid: newEnemyGrid,
                            // Track ships sunk by me
                            enemyShipsSunk: msg.sunk ? prev.enemyShipsSunk + 1 : prev.enemyShipsSunk,
                            // Add sunk cells to enemy sunk tracking
                            enemySunkCells: newSunkCells.length > 0
                                ? [...prev.enemySunkCells, ...newSunkCells]
                                : prev.enemySunkCells,
                        };
                    } else {
                        // Update my grid with opponent's shot
                        const newMyGrid = prev.myGrid.map(row => [...row]);
                        newMyGrid[msg.y][msg.x] = msg.hit ? "hit" : "miss";
                        return {
                            ...prev,
                            myGrid: newMyGrid,
                            // Add sunk cells to my sunk tracking
                            mySunkCells: newSunkCells.length > 0
                                ? [...prev.mySunkCells, ...newSunkCells]
                                : prev.mySunkCells,
                        };
                    }
                });
                break;

            case "YourShips":
                // Bug 8 fix: Store ships in myShips for CombatGrid rendering after reconnect
                setGameState(prev => ({
                    ...prev,
                    myShips: msg.ships,
                }));
                break;

            case "GridSync":
                setGameState(prev => ({
                    ...prev,
                    myGrid: msg.my_grid,
                    enemyGrid: msg.enemy_grid,
                }));
                break;

            case "WeaponsLocked":
                // Only apply to the player this message is for
                // Use functional update to access latest playerId (avoid stale closure)
                setGameState(prev => {
                    if (msg.player_id !== prev.playerId) return prev;
                    toast.warning("Weapons overheated! Solve a problem to unlock.", { id: "weapons-locked" });
                    return {
                        ...prev,
                        isLocked: true,
                        status: "WEAPONS LOCKED - Solve to unlock",
                    };
                });
                break;

            case "ProblemAssigned":
                // Server assigned a problem — update state so ProblemPanel displays it
                setGameState(prev => {
                    if (msg.player_id !== prev.playerId) return prev;
                    return {
                        ...prev,
                        activeProblemContestId: msg.contest_id,
                        activeProblemIndex: msg.problem_index,
                        activeProblemName: msg.problem_name,
                        activeProblemRating: msg.rating,
                    };
                });
                break;

            case "WeaponsUnlocked":
                // Only apply to the player this message is for
                // Use functional update to access latest playerId (avoid stale closure)
                setGameState(prev => {
                    if (msg.player_id !== prev.playerId) return prev;
                    toast.success(msg.reason === "solved" ? "Problem solved! Weapons unlocked!" : "Veto expired! Weapons unlocked!", { id: "weapons-unlocked" });
                    return {
                        ...prev,
                        isLocked: false,
                        heat: 0,
                        status: "Weapons unlocked",
                        // Only track problems solved when actually solved (not veto expiry)
                        problemsSolved: msg.reason === "solved" ? prev.problemsSolved + 1 : prev.problemsSolved,
                        // Clear server-assigned problem on unlock
                        activeProblemContestId: null,
                        activeProblemIndex: null,
                        activeProblemName: null,
                        activeProblemRating: null,
                    };
                });
                break;

            case "GameOver":
                // CLEANUP: Clear active game session when game ends
                localStorage.removeItem("battlecp_active_game");

                // Handle timeout reasons with specific messages
                if (msg.reason === "LobbyTimeout") {
                    toast.error("Lobby expired — no opponent joined within 5 minutes.", { id: "lobby-timeout", duration: 10000 });
                } else if (msg.reason === "PlacementTimeout") {
                    toast.error("Game start failed — ships were not deployed in time.", { id: "placement-timeout", duration: 10000 });
                } else if (msg.reason === "SuddenDeathTimeout") {
                    toast.error("Sudden Death timed out — no player landed a hit in 10 minutes.", { id: "sd-timeout", duration: 10000 });
                }

                setGameState(prev => {
                    // Determine which stats belong to this player using the authoritative p1_id
                    const isP1 = prev.playerId === msg.p1_id;
                    const myShipsSunk = isP1 ? msg.p1_ships_sunk : msg.p2_ships_sunk;
                    const myProblemsSolved = isP1 ? msg.p1_problems_solved : msg.p2_problems_solved;
                    const myCellsHit = isP1 ? msg.p1_cells_hit : msg.p2_cells_hit;
                    const oppShipsSunk = isP1 ? msg.p2_ships_sunk : msg.p1_ships_sunk;
                    const oppProblemsSolved = isP1 ? msg.p2_problems_solved : msg.p1_problems_solved;
                    const oppCellsHit = isP1 ? msg.p2_cells_hit : msg.p1_cells_hit;

                    // Resolve board reveal to my/opponent perspective
                    const revealMyGrid = isP1 ? msg.p1_grid : msg.p2_grid;
                    const revealMyShips = isP1 ? msg.p1_ships : msg.p2_ships;
                    const revealOpponentGrid = isP1 ? msg.p2_grid : msg.p1_grid;
                    const revealOpponentShips = isP1 ? msg.p2_ships : msg.p1_ships;

                    return {
                        ...prev,
                        phase: "finished",
                        winnerId: msg.winner_id,
                        gameOverReason: msg.reason,
                        status: msg.reason === "LobbyTimeout" || msg.reason === "PlacementTimeout" || msg.reason === "SuddenDeathTimeout"
                            ? "GAME EXPIRED"
                            : msg.winner_id === prev.playerId ? "VICTORY" : "DEFEAT",
                        // Override with authoritative server stats — these are always correct
                        enemyShipsSunk: myShipsSunk,
                        problemsSolved: myProblemsSolved,
                        myCellsHit: myCellsHit,
                        opponentShipsSunk: oppShipsSunk,
                        opponentProblemsSolved: oppProblemsSolved,
                        opponentCellsHit: oppCellsHit,
                        // Board reveal data (resolved to my/opponent perspective)
                        revealMyGrid,
                        revealMyShips,
                        revealOpponentGrid,
                        revealOpponentShips,
                        // Swiss score (server-authoritative)
                        myScore: msg.winner_id === null
                            ? 0
                            : msg.winner_id === prev.playerId
                                ? msg.winner_score
                                : msg.loser_score,
                        opponentScore: msg.winner_id === null
                            ? 0
                            : msg.winner_id === prev.playerId
                                ? msg.loser_score
                                : msg.winner_score,
                    };
                });
                shouldStopReconnect.current = true; // Don't reconnect after game over
                break;

            case "Error":
                console.error("[WS] Server error:", msg.message);
                setGameState(prev => ({
                    ...prev,
                    lastError: msg.message,
                }));
                // If game not found, ended, or full - set flag to prevent reconnection
                const isFatalError = msg.message.includes("not found")
                    || msg.message.includes("already ended")
                    || msg.message.includes("full")
                    || msg.message.includes("2 players already")
                    || msg.message.includes("You cannot play against yourself");

                if (isFatalError) {
                    setGameNotFound(true);
                    shouldStopReconnect.current = true; // Prevent reconnection attempts
                    localStorage.removeItem("battlecp_active_game");

                    if (msg.message.includes("2 players already") || msg.message.includes("You cannot play against yourself")) {
                        toast.error(msg.message, { id: "easter-egg-full", duration: 8000 }); // Show the easter egg!
                    } else if (msg.message.includes("full")) {
                        toast.error("This game is full. Both player slots are occupied.", { id: "game-full" });
                    } else {
                        toast.error("Game not found or has ended. Please create a new game.");
                    }
                } else if (msg.message.includes("Submission not accepted") || msg.message.includes("No accepted submission")) {
                    toast.error("No accepted submission found. Solve the problem on Codeforces first!");
                } else {
                    toast.error(msg.message);
                }
                break;
        }
    }, []);

    // WebSocket connection
    useEffect(() => {
        if (!gameId || !playerId) return;

        // Guard against double connections (React Strict Mode, fast re-renders)
        // Check for both CONNECTING and OPEN states
        const wsState = wsRef.current?.readyState;
        if (isConnecting.current || wsState === WebSocket.OPEN || wsState === WebSocket.CONNECTING) {
            return;
        }

        const connect = () => {
            // Check guard again before connecting
            if (isConnecting.current) {
                return;
            }
            isConnecting.current = true;
            const ws = new WebSocket(`${WS_BASE_URL}/ws/${gameId}?player_id=${playerId}`);
            wsRef.current = ws;

            ws.onopen = () => {
                isConnecting.current = false;
                setIsConnected(true);
                reconnectAttempts.current = 0;

                // Send JoinGame message
                ws.send(JSON.stringify({
                    type: "JoinGame",
                    player_id: playerId,
                    cf_handle: cfHandle,
                }));
            };

            ws.onmessage = (event) => {
                try {
                    const msg: ServerMessage = JSON.parse(event.data);
                    handleServerMessage(msg);
                } catch (e) {
                    console.error("[WS] Failed to parse message:", e);
                }
            };

            ws.onclose = (event) => {
                isConnecting.current = false;
                setIsConnected(false);
                wsRef.current = null;

                if (event.code !== 1000 && reconnectAttempts.current < maxReconnectAttempts && !shouldStopReconnect.current) {
                    reconnectAttempts.current++;
                    const delay = Math.min(1000 * Math.pow(2, reconnectAttempts.current), 10000);
                    setTimeout(connect, delay);
                }
            };

            ws.onerror = () => {
                isConnecting.current = false;
            };
        };

        connect();

        return () => {
            isConnecting.current = false;
            if (wsRef.current) {
                wsRef.current.close(1000, "Component unmounting");
                wsRef.current = null;
            }
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [gameId, playerId, cfHandle]); // Removed handleServerMessage to prevent unnecessary reconnects

    // Action: Fire at coordinates
    const fire = useCallback((x: number, y: number) => {
        if (wsRef.current?.readyState === WebSocket.OPEN && !gameState.isLocked) {
            wsRef.current.send(JSON.stringify({ type: "Fire", x, y }));
        }
    }, [gameState.isLocked]);

    // Action: Place ships
    const placeShips = useCallback((ships: ShipPlacement[]) => {
        if (wsRef.current?.readyState === WebSocket.OPEN) {
            wsRef.current.send(JSON.stringify({ type: "PlaceShips", ships }));
        }
    }, []);

    // Action: Solve CP problem
    const solveCP = useCallback((contestId: number, problemIndex: string) => {
        if (wsRef.current?.readyState === WebSocket.OPEN) {
            wsRef.current.send(JSON.stringify({
                type: "SolveCP",
                contest_id: contestId,
                problem_index: problemIndex
            }));
        }
    }, []);

    // Action: Use veto
    const veto = useCallback(() => {
        if (wsRef.current?.readyState === WebSocket.OPEN && gameState.vetoesRemaining > 0) {
            wsRef.current.send(JSON.stringify({ type: "Veto" }));
        }
    }, [gameState.vetoesRemaining]);

    return {
        gameState,
        isConnected,
        gameNotFound,
        fire,
        placeShips,
        solveCP,
        veto,
    };
}
