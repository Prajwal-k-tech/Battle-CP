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
                console.log(`[WS] GameJoined: game_id=${msg.game_id}`);
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
                console.log("[WS] GameStart received - moving to combat");
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

                    console.log(`[WS] GameUpdate: status="${msg.status}", currentPhase="${prev.phase}"`);

                    // Only transit to combat if we are actually playing
                    if (msg.status === "Playing" && (prev.phase === "connecting" || prev.phase === "lobby" || prev.phase === "placement")) {
                        newPhase = "combat";
                        console.log("[WS] GameUpdate: Transitioning to combat");
                    }
                    // Handle initial connection state - but NEVER reset from placement
                    else if (msg.status.includes("Waiting") && prev.phase === "connecting") {
                        newPhase = "lobby";
                        console.log("[WS] GameUpdate: Transitioning from connecting to lobby");
                    }
                    // IMPORTANT: Don't change phase otherwise - preserve placement/combat phases

                    console.log(`[WS] GameUpdate: newPhase will be "${newPhase}"`);

                    return {
                        ...prev,
                        heat: msg.heat,
                        isLocked: msg.is_locked,
                        gameTimeRemaining: msg.time_remaining_secs,
                        vetoesRemaining: msg.vetoes_remaining,
                        vetoTimeRemaining: msg.veto_time_remaining_secs ?? null,
                        status: msg.status,
                        phase: newPhase
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

                    if (isMyShot) {
                        // Update enemy grid with my shot result
                        const newEnemyGrid = prev.enemyGrid.map(row => [...row]);
                        newEnemyGrid[msg.y][msg.x] = msg.hit ? "hit" : "miss";
                        return {
                            ...prev,
                            enemyGrid: newEnemyGrid,
                            // Track ships sunk by me
                            enemyShipsSunk: msg.sunk ? prev.enemyShipsSunk + 1 : prev.enemyShipsSunk,
                        };
                    } else {
                        // Update my grid with opponent's shot
                        const newMyGrid = prev.myGrid.map(row => [...row]);
                        newMyGrid[msg.y][msg.x] = msg.hit ? "hit" : "miss";
                        return { ...prev, myGrid: newMyGrid };
                    }
                });
                break;

            case "YourShips":
                setGameState(prev => ({
                    ...prev,
                    ships: msg.ships,
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
                    };
                });
                break;

            case "GameOver":
                setGameState(prev => ({
                    ...prev,
                    phase: "finished",
                    winnerId: msg.winner_id,
                    gameOverReason: msg.reason,
                    status: msg.winner_id === prev.playerId ? "VICTORY" : "DEFEAT",
                    // Use server-provided stats
                    problemsSolved: msg.your_problems_solved ?? prev.problemsSolved,
                    enemyShipsSunk: msg.your_ships_sunk ?? prev.enemyShipsSunk,
                }));
                break;

            case "Error":
                console.error("[WS] Server error:", msg.message);
                setGameState(prev => ({
                    ...prev,
                    lastError: msg.message,
                }));
                // If game not found or ended, set flag to prevent reconnection
                if (msg.message.includes("Game not found") || msg.message.includes("not found") || msg.message.includes("already ended")) {
                    setGameNotFound(true);
                    shouldStopReconnect.current = true; // Prevent reconnection attempts
                    toast.error("Game not found or has ended. Please create a new game.");
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
            console.log(`[WS] Already connecting or connected (state: ${wsState}), skipping`);
            return;
        }

        const connect = () => {
            // Check guard again before connecting
            if (isConnecting.current) {
                console.log("[WS] connect() called but isConnecting is true, bailing");
                return;
            }
            isConnecting.current = true;

            console.log(`[WS] Connecting to ${WS_BASE_URL}/ws/${gameId}`);
            const ws = new WebSocket(`${WS_BASE_URL}/ws/${gameId}?player_id=${playerId}`);
            wsRef.current = ws;

            ws.onopen = () => {
                console.log("[WS] Connected successfully");
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
                console.log("[WS] Disconnected:", event.code, event.reason);
                isConnecting.current = false;
                setIsConnected(false);
                wsRef.current = null;

                // Attempt reconnect if not intentional close, game exists, and not told to stop
                if (event.code !== 1000 && reconnectAttempts.current < maxReconnectAttempts && !shouldStopReconnect.current) {
                    reconnectAttempts.current++;
                    const delay = Math.min(1000 * Math.pow(2, reconnectAttempts.current), 10000);
                    console.log(`[WS] Reconnecting in ${delay}ms... (attempt ${reconnectAttempts.current})`);
                    setTimeout(connect, delay);
                } else if (shouldStopReconnect.current) {
                    console.log("[WS] Not reconnecting - game not found or ended");
                }
            };

            ws.onerror = (error: Event) => {
                // WebSocket errors don't contain useful info in the Event object
                // Real error details come through onmessage (Error type) or onclose
                // Using warn instead of error to avoid triggering Next.js dev error overlay
                console.warn("[WS] WebSocket error event (often harmless in dev mode). Target:", (error.target as WebSocket)?.url);
                isConnecting.current = false;
                // Don't show toast here - let onclose or Error message handle it
            };
        };

        connect();

        return () => {
            console.log("[WS] Cleanup: closing connection");
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
