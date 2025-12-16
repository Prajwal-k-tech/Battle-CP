"use client";

import { useEffect, useState, use, useRef } from "react";
import { useGameSocket } from "@/hooks/useGameSocket";
import { PlacementBoard } from "@/components/game/PlacementBoard";
import { CombatGrid } from "@/components/game/CombatGrid";
import { HUD } from "@/components/game/HUD";
import { VictoryModal } from "@/components/game/VictoryModal";
import { ProblemPanel } from "@/components/game/ProblemPanel";
import Squares from "@/components/ui/Squares";
import { ShipPlacement } from "@/types/game";
import { Loader2, Wifi, WifiOff } from "lucide-react";

export default function GamePage({ params }: { params: Promise<{ gameId: string }> }) {
    const { gameId } = use(params);

    // Use hasMounted pattern to avoid hydration mismatch
    const [hasMounted, setHasMounted] = useState(false);
    const [playerId, setPlayerId] = useState<string | null>(null);
    const [cfHandle, setCfHandle] = useState<string | null>(null);
    const [myShips, setMyShips] = useState<ShipPlacement[]>([]);
    const initialized = useRef(false);

    // Initialize credentials ONLY after client-side mount
    useEffect(() => {
        // Prevent double-initialization in StrictMode
        if (initialized.current) return;
        initialized.current = true;

        let storedPlayerId = localStorage.getItem("battlecp_player_id");
        let storedCfHandle = localStorage.getItem("battlecp_cf_handle");

        // Generate if not found
        if (!storedPlayerId || !storedCfHandle) {
            storedPlayerId = crypto.randomUUID();
            storedCfHandle = "anonymous";
            localStorage.setItem("battlecp_player_id", storedPlayerId);
            localStorage.setItem("battlecp_cf_handle", storedCfHandle);
        }

        setPlayerId(storedPlayerId);
        setCfHandle(storedCfHandle);
        setHasMounted(true);
    }, []);

    // Don't render game content until client-side mount is complete
    if (!hasMounted || !playerId || !cfHandle) {
        return (
            <div className="min-h-screen bg-black flex items-center justify-center">
                <Loader2 className="w-8 h-8 animate-spin text-primary" />
            </div>
        );
    }

    return (
        <GameContent
            gameId={gameId}
            playerId={playerId}
            cfHandle={cfHandle}
            myShips={myShips}
            setMyShips={setMyShips}
        />
    );
}

// DevMenu removed - use normal lobby flow for game creation/joining

function GameContent({
    gameId,
    playerId,
    cfHandle,
    myShips,
    setMyShips,
}: {
    gameId: string;
    playerId: string;
    cfHandle: string;
    myShips: ShipPlacement[];
    setMyShips: (ships: ShipPlacement[]) => void;
}) {
    const { gameState, isConnected, gameNotFound, fire, placeShips, solveCP, veto } = useGameSocket(gameId, playerId, cfHandle);

    const handleShipsConfirmed = (ships: any[]) => {
        // Convert to ShipPlacement format
        const placements: ShipPlacement[] = ships.map(s => ({
            x: s.x,
            y: s.y,
            size: s.size,
            vertical: s.orientation === "vertical",
        }));
        setMyShips(placements);
        placeShips(placements);
    };

    // Dev Tool: Auto Place
    const handleAutoPlace = (ships: ShipPlacement[]) => {
        setMyShips(ships);
        placeShips(ships);
    };

    const handleFire = (x: number, y: number) => {
        if (!gameState.isLocked) {
            fire(x, y);
        }
    };

    const isWinner = gameState.winnerId === playerId;

    // Calculate Stats
    const shotsHit = gameState.enemyGrid.flat().filter(c => c === "hit").length;
    const shotsMissed = gameState.enemyGrid.flat().filter(c => c === "miss").length;

    // Show error UI if game not found
    if (gameNotFound) {
        return (
            <div className="relative w-full min-h-screen bg-black flex flex-col items-center justify-center">
                <div className="text-center space-y-4">
                    <h1 className="text-2xl font-bold text-red-500">Game Not Found</h1>
                    <p className="text-gray-400">This game may have expired or doesn't exist.</p>
                    <a
                        href="/lobby/create"
                        className="inline-block px-6 py-3 bg-primary text-black font-bold rounded hover:bg-primary/80 transition"
                    >
                        Create New Game
                    </a>
                </div>
            </div>
        );
    }

    return (
        <div className="relative w-full min-h-screen bg-black overflow-hidden flex flex-col">

            {/* Background - Animated Squares (dynamic colors based on heat) */}
            <div className="absolute inset-0 z-0" style={{ pointerEvents: 'auto' }}>
                <Squares
                    speed={0.4}
                    squareSize={40}
                    direction="diagonal"
                    borderColor={gameState.isLocked ? "#3d1a0a" : "#0a2540"}
                    hoverFillColor={gameState.isLocked ? "#8b3a1a" : "#1e5080"}
                />
            </div>

            {/* HUD - Always visible during combat */}
            {(gameState.phase === "combat" || gameState.phase === "placement") && (
                <HUD
                    heat={gameState.heat}
                    maxHeat={gameState.maxHeat}
                    isLocked={gameState.isLocked}
                    gameTimeRemaining={gameState.gameTimeRemaining}
                    vetoTimeRemaining={gameState.vetoTimeRemaining}
                    vetoesRemaining={gameState.vetoesRemaining}
                    status={gameState.status}
                    opponentConnected={gameState.opponentConnected}
                />
            )}

            {/* Connection Status Bar */}
            <div className="fixed bottom-4 right-4 z-50 flex items-center gap-2 bg-black/80 px-3 py-2 rounded-full border border-white/10 text-xs">
                {isConnected ? (
                    <>
                        <Wifi className="w-4 h-4 text-green-500" />
                        <span className="text-green-500">Connected</span>
                    </>
                ) : (
                    <>
                        <WifiOff className="w-4 h-4 text-red-500" />
                        <span className="text-red-500">Reconnecting...</span>
                    </>
                )}
            </div>

            {/* Main Content */}
            <main className="relative z-10 flex-1 flex flex-col items-center justify-center pt-20 p-4">
                {/* Connecting State */}
                {gameState.phase === "connecting" && (
                    <div className="flex flex-col items-center gap-4 animate-pulse">
                        <Loader2 className="w-12 h-12 animate-spin text-primary" />
                        <span className="text-zinc-400 font-mono">Establishing uplink... {gameState.lastError && <span className="text-red-500 block text-xs">{gameState.lastError}</span>}</span>
                    </div>
                )}

                {/* Lobby State - Waiting for opponent */}
                {gameState.phase === "lobby" && (
                    <div className="flex flex-col items-center gap-6 text-center">
                        <div className="text-4xl font-heading text-primary animate-pulse">AWAITING OPPONENT</div>
                        <div className="text-zinc-500 max-w-md">
                            Share your game code with a friend to start the battle.
                        </div>
                        <div className="bg-primary/10 border border-primary/30 rounded-lg px-8 py-4">
                            <span className="text-xs text-zinc-500 block mb-1">GAME CODE</span>
                            <span className="text-3xl font-mono font-bold text-white tracking-widest">{gameId}</span>
                        </div>
                    </div>
                )}

                {/* Placement Phase */}
                {gameState.phase === "placement" && (
                    <div className="w-full h-full flex items-center justify-center">
                        {!gameState.myShipsPlaced ? (
                            <div className="animate-in fade-in zoom-in-95 duration-500">
                                <div className="text-center mb-8">
                                    <h2 className="text-2xl font-bold mb-2">DEPLOY YOUR FLEET</h2>
                                    <p className="text-zinc-500">Drag ships to the grid. Click ROTATE to change orientation.</p>
                                </div>
                                <PlacementBoard onConfirm={handleShipsConfirmed} />
                            </div>
                        ) : (
                            <div className="flex flex-col items-center gap-4">
                                <Loader2 className="w-8 h-8 animate-spin text-primary" />
                                <span className="text-zinc-400">
                                    {gameState.opponentShipsPlaced
                                        ? "Starting battle..."
                                        : "Waiting for opponent to deploy..."}
                                </span>
                            </div>
                        )}
                    </div>
                )}

                {/* Combat Phase */}
                {gameState.phase === "combat" && (
                    <div className="w-full animate-in fade-in duration-500">
                        <CombatGrid
                            myGrid={gameState.myGrid}
                            enemyGrid={gameState.enemyGrid}
                            myShips={myShips}
                            onFire={handleFire}
                            canFire={!gameState.isLocked}
                        />
                    </div>
                )}

                {/* Problem Panel - Shows when weapons are locked */}
                {gameState.phase === "combat" && (
                    <ProblemPanel
                        isLocked={gameState.isLocked}
                        difficulty={gameState.difficulty}
                        vetoesRemaining={gameState.vetoesRemaining}
                        vetoTimeRemaining={gameState.vetoTimeRemaining}
                        onSolve={solveCP}
                        onVeto={veto}
                    />
                )}

                {/* Finished Phase */}
                {gameState.phase === "finished" && (
                    <VictoryModal
                        isOpen={true}
                        isWinner={isWinner}
                        reason={gameState.gameOverReason || "Unknown"}
                        stats={{
                            shotsHit,
                            shotsMissed,
                            problemsSolved: gameState.problemsSolved,
                            enemyShipsSunk: gameState.enemyShipsSunk,
                        }}
                    />
                )}
            </main>
        </div>
    );
}
