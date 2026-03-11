"use client";

import React from "react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { motion, AnimatePresence } from "framer-motion";
import { Trophy, Skull, Home, RotateCcw, Target, Brain, Ship, Gauge } from "lucide-react";
import Link from "next/link";
import { RevealedShip } from "@/types/game";
import { useSound } from "@/context/SoundContext";
import Squares from "@/components/ui/Squares";

interface VictoryModalProps {
    isOpen: boolean;
    isWinner: boolean;
    reason: string;
    myStats: {
        cellsHit: number;
        shipsSunk: number;
        problemsSolved: number;
    };
    opponentStats: {
        cellsHit: number;
        shipsSunk: number;
        problemsSolved: number;
    };
    // Board reveal data
    myGrid: string[][] | null;
    myShips: RevealedShip[] | null;
    opponentGrid: string[][] | null;
    opponentShips: RevealedShip[] | null;
    myScore?: number | null;
}

const reasonLabels: Record<string, string> = {
    AllShipsSunk: "All Enemy Ships Destroyed",
    "Timeout - More ships remaining": "Time Limit Reached — More Ships Remaining",
    "SuddenDeath - First hit wins!": "Sudden Death — First Hit Wins!",
    SuddenDeathTimeout: "Sudden Death Timeout — Draw",
    Disconnect: "Opponent Disconnected",
    LobbyTimeout: "No Opponent Joined (5 min)",
    PlacementTimeout: "Ships Not Deployed In Time (10 min)",
};

const GRID_SIZE = 10;
const LABELS = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];

// Mini board for post-game reveal
function RevealBoard({
    title,
    grid,
    ships,
    titleColor,
}: {
    title: string;
    grid: string[][];
    ships: RevealedShip[];
    titleColor: string;
}) {
    // Build a set of ship cells and sunk cells for rendering
    const shipCells = new Set<string>();
    const sunkCells = new Set<string>();

    for (const ship of ships) {
        for (let i = 0; i < ship.size; i++) {
            const cx = ship.vertical ? ship.x : ship.x + i;
            const cy = ship.vertical ? ship.y + i : ship.y;
            const key = `${cx},${cy}`;
            shipCells.add(key);
            if (ship.sunk) sunkCells.add(key);
        }
    }

    return (
        <div className="flex flex-col items-center gap-1">
            <span className={cn("text-[10px] font-mono uppercase tracking-widest", titleColor)}>
                {title}
            </span>
            <div className="flex">
                {/* Row numbers */}
                <div className="flex flex-col">
                    <div className="w-3 h-3" /> {/* Corner spacer */}
                    {Array.from({ length: GRID_SIZE }).map((_, i) => (
                        <div key={i} className="w-3 h-3 flex items-center justify-center text-[6px] text-zinc-600 font-mono">
                            {i + 1}
                        </div>
                    ))}
                </div>

                <div>
                    {/* Column labels */}
                    <div className="flex">
                        {LABELS.map((label) => (
                            <div key={label} className="w-3 h-3 flex items-center justify-center text-[6px] text-zinc-600 font-mono">
                                {label}
                            </div>
                        ))}
                    </div>

                    {/* Grid cells */}
                    <div className="border border-white/10">
                        {Array.from({ length: GRID_SIZE }).map((_, y) => (
                            <div key={y} className="flex">
                                {Array.from({ length: GRID_SIZE }).map((_, x) => {
                                    const cellVal = grid[y]?.[x] ?? "empty";
                                    const key = `${x},${y}`;
                                    const isShipCell = shipCells.has(key);
                                    const isSunkCell = sunkCells.has(key);
                                    const isHit = cellVal === "hit";
                                    const isMiss = cellVal === "miss";

                                    return (
                                        <div
                                            key={`${x}-${y}`}
                                            className={cn(
                                                "w-3 h-3 border border-white/5",
                                                // Sunk ship cell (hit + sunk) = red
                                                isHit && isSunkCell && "bg-red-600/60",
                                                // Hit but not sunk yet = orange
                                                isHit && !isSunkCell && "bg-orange-500/50",
                                                // Miss = dark gray
                                                isMiss && "bg-zinc-700/40",
                                                // Ship but not hit = blue tint
                                                isShipCell && !isHit && !isMiss && "bg-blue-500/30",
                                                // Empty = near invisible
                                                !isShipCell && !isHit && !isMiss && "bg-white/[0.02]"
                                            )}
                                        />
                                    );
                                })}
                            </div>
                        ))}
                    </div>
                </div>
            </div>

            {/* Ship legend */}
            <div className="flex gap-1 mt-1 flex-wrap justify-center">
                {ships.map((ship, i) => (
                    <div
                        key={i}
                        className="flex gap-px"
                        title={`${ship.size}-cell ship${ship.sunk ? " (SUNK)" : ""}`}
                    >
                        {Array.from({ length: ship.size }).map((_, j) => (
                            <div
                                key={j}
                                className={cn(
                                    "w-2 h-2 rounded-[1px]",
                                    ship.sunk ? "bg-red-500/70" : "bg-blue-500/50"
                                )}
                            />
                        ))}
                    </div>
                ))}
            </div>
        </div>
    );
}

export function VictoryModal({
    isOpen,
    isWinner,
    reason,
    myStats,
    opponentStats,
    myGrid,
    myShips,
    opponentGrid,
    opponentShips,
    myScore,
}: VictoryModalProps) {
    const { playShipPlace, playJoin } = useSound();

    if (!isOpen) return null;

    const hasBoards = myGrid && myShips && opponentGrid && opponentShips;
    const isTimeout = reason === "LobbyTimeout" || reason === "PlacementTimeout" || reason === "SuddenDeathTimeout";

    return (
        <AnimatePresence>
            <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="fixed inset-0 z-50 flex items-center justify-center"
            >
                {/* Background with Squares animation */}
                <div className="absolute inset-0 z-0">
                    <Squares
                        speed={0.3}
                        squareSize={50}
                        direction="diagonal"
                        borderColor={isWinner ? "#0a2540" : "#3d1a0a"}
                        hoverFillColor={isWinner ? "#1e5080" : "#8b3a1a"}
                    />
                    <div className="absolute inset-0 bg-black/70" />
                </div>
                <motion.div
                    initial={{ scale: 0.8, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    transition={{ type: "spring", duration: 0.5 }}
                    className="relative z-10 max-h-[90vh] overflow-y-auto"
                >
                    <Card className={cn(
                        "w-[520px] max-w-[95vw] border-2 bg-black/90 backdrop-blur-xl",
                        isWinner ? "border-emerald-500/50" : "border-red-500/50"
                    )}>
                        <CardHeader className="text-center pb-2">
                            {/* Icon */}
                            <motion.div
                                initial={{ scale: 0 }}
                                animate={{ scale: 1, rotate: [0, 10, -10, 0] }}
                                transition={{ delay: 0.2, duration: 0.5 }}
                                className="mx-auto mb-3"
                            >
                                {isWinner ? (
                                    <div className="w-16 h-16 rounded-full bg-emerald-500/20 flex items-center justify-center">
                                        <Trophy className="w-8 h-8 text-emerald-500" />
                                    </div>
                                ) : (
                                    <div className="w-16 h-16 rounded-full bg-red-500/20 flex items-center justify-center">
                                        <Skull className="w-8 h-8 text-red-500" />
                                    </div>
                                )}
                            </motion.div>

                            {/* Title */}
                            <CardTitle className={cn(
                                "text-3xl font-heading tracking-widest",
                                isWinner ? "text-emerald-500" : "text-red-500"
                            )}>
                                {isWinner ? "VICTORY" : "DEFEAT"}
                            </CardTitle>

                            {/* Reason */}
                            <p className="text-sm text-zinc-400 mt-1">
                                {reasonLabels[reason] || reason}
                            </p>
                        </CardHeader>

                        <CardContent className="space-y-5">
                            {/* Side-by-side stats comparison */}
                            {!isTimeout && (
                                <div className="space-y-2">
                                    <div className="grid grid-cols-3 gap-2 text-center text-xs text-zinc-500 font-mono">
                                        <span className="text-blue-400">YOU</span>
                                        <span></span>
                                        <span className="text-red-400">OPPONENT</span>
                                    </div>
                                    {myScore != null && (
                                        <ComparisonRow
                                            icon={<Gauge className="w-3.5 h-3.5" />}
                                            label="Score"
                                            myVal={myScore >= 0 ? `+${myScore.toFixed(3)}` : myScore.toFixed(3)}
                                            oppVal={(-myScore) >= 0 ? `+${(-myScore).toFixed(3)}` : (-myScore).toFixed(3)}
                                            myBetter={myScore > 0}
                                            oppBetter={myScore < 0}
                                        />
                                    )}
                                    <ComparisonRow
                                        icon={<Target className="w-3.5 h-3.5" />}
                                        label="Hits"
                                        myVal={myStats.cellsHit}
                                        oppVal={opponentStats.cellsHit}
                                    />
                                    <ComparisonRow
                                        icon={<Ship className="w-3.5 h-3.5" />}
                                        label="Ships Sunk"
                                        myVal={myStats.shipsSunk}
                                        oppVal={opponentStats.shipsSunk}
                                    />
                                    <ComparisonRow
                                        icon={<Brain className="w-3.5 h-3.5" />}
                                        label="Problems"
                                        myVal={myStats.problemsSolved}
                                        oppVal={opponentStats.problemsSolved}
                                    />
                                </div>
                            )}

                            {/* Board Reveal */}
                            {hasBoards && (
                                <div className="space-y-2">
                                    <div className="text-center text-[10px] text-zinc-600 font-mono uppercase tracking-widest">
                                        Board Reveal
                                    </div>
                                    <div className="flex justify-center gap-6">
                                        <RevealBoard
                                            title="Your Fleet"
                                            grid={myGrid}
                                            ships={myShips}
                                            titleColor="text-blue-400"
                                        />
                                        <RevealBoard
                                            title="Enemy Fleet"
                                            grid={opponentGrid}
                                            ships={opponentShips}
                                            titleColor="text-red-400"
                                        />
                                    </div>
                                    <div className="flex justify-center gap-4 text-[8px] text-zinc-600 font-mono">
                                        <span className="flex items-center gap-1">
                                            <div className="w-2 h-2 bg-blue-500/30 border border-white/10" /> Ship
                                        </span>
                                        <span className="flex items-center gap-1">
                                            <div className="w-2 h-2 bg-orange-500/50 border border-white/10" /> Hit
                                        </span>
                                        <span className="flex items-center gap-1">
                                            <div className="w-2 h-2 bg-red-600/60 border border-white/10" /> Sunk
                                        </span>
                                        <span className="flex items-center gap-1">
                                            <div className="w-2 h-2 bg-zinc-700/40 border border-white/10" /> Miss
                                        </span>
                                    </div>
                                </div>
                            )}

                            {/* Actions */}
                            <div className="flex gap-3 pt-2">
                                <Link href="/" className="flex-1">
                                    <Button variant="outline" className="w-full border-white/10" onClick={() => playShipPlace()}>
                                        <Home className="w-4 h-4 mr-2" />
                                        Home
                                    </Button>
                                </Link>
                                <Link href="/lobby/create" className="flex-1">
                                    <Button
                                        className={cn(
                                            "w-full",
                                            isWinner ? "bg-emerald-600 hover:bg-emerald-700" : "bg-red-600 hover:bg-red-700"
                                        )}
                                        onClick={() => playJoin()}
                                    >
                                        <RotateCcw className="w-4 h-4 mr-2" />
                                        Play Again
                                    </Button>
                                </Link>
                            </div>
                        </CardContent>
                    </Card>
                </motion.div>
            </motion.div>
        </AnimatePresence>
    );
}

function ComparisonRow({
    icon,
    label,
    myVal,
    oppVal,
    myBetter,
    oppBetter,
}: {
    icon: React.ReactNode;
    label: string;
    myVal: number | string;
    oppVal: number | string;
    myBetter?: boolean;
    oppBetter?: boolean;
}) {
    const _myBetter = myBetter !== undefined ? myBetter : (typeof myVal === 'number' && typeof oppVal === 'number' && myVal > oppVal);
    const _oppBetter = oppBetter !== undefined ? oppBetter : (typeof myVal === 'number' && typeof oppVal === 'number' && oppVal > myVal);

    return (
        <div className="grid grid-cols-3 gap-2 items-center">
            <div className={cn(
                "bg-white/5 rounded-lg px-3 py-2 text-center font-mono text-lg",
                _myBetter && "text-emerald-400",
                !_myBetter && "text-white"
            )}>
                {myVal}
            </div>
            <div className="flex items-center justify-center gap-1.5 text-zinc-500">
                {icon}
                <span className="text-[10px] font-mono uppercase">{label}</span>
            </div>
            <div className={cn(
                "bg-white/5 rounded-lg px-3 py-2 text-center font-mono text-lg",
                _oppBetter && "text-emerald-400",
                !_oppBetter && "text-white"
            )}>
                {oppVal}
            </div>
        </div>
    );
}
