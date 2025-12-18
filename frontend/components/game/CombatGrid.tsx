"use client";

import React from "react";
import { cn } from "@/lib/utils";
import { motion } from "framer-motion";
import { CellState } from "@/types/game";
import { Crosshair } from "lucide-react";

interface CombatGridProps {
    myGrid: CellState[][];
    enemyGrid: CellState[][];
    myShips: { x: number; y: number; size: number; vertical: boolean }[];
    onFire: (x: number, y: number) => void;
    canFire: boolean;
}

const GRID_SIZE = 10;
const LABELS = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];

const Cell = React.memo(function Cell({
    state,
    isEnemy,
    onClick,
    canClick,
}: {
    state: CellState;
    isEnemy: boolean;
    onClick?: () => void;
    canClick: boolean;
}) {
    const isHit = state === "hit";
    const isMiss = state === "miss";
    const isShip = state === "ship";
    const isEmpty = state === "empty";

    return (
        <motion.button
            whileHover={canClick ? { scale: 1.1 } : {}}
            whileTap={canClick ? { scale: 0.95 } : {}}
            onClick={onClick}
            disabled={!canClick || isHit || isMiss}
            className={cn(
                "w-8 h-8 sm:w-10 sm:h-10 border border-white/5 relative flex items-center justify-center transition-all duration-200",
                canClick && isEmpty && "hover:bg-primary/20 hover:border-primary/50 cursor-crosshair",
                !canClick && "cursor-default",
                isShip && !isEnemy && "bg-primary/30 border-primary/40",
                isHit && "bg-red-500/40 border-red-500/60",
                isMiss && "bg-zinc-700/30"
            )}
        >
            {/* Empty state dot */}
            {isEmpty && !isEnemy && <div className="w-1 h-1 bg-white/10 rounded-full" />}

            {/* Hit marker */}
            {isHit && (
                <motion.div
                    initial={{ scale: 0 }}
                    animate={{ scale: 1 }}
                    className="absolute inset-0 flex items-center justify-center"
                >
                    <div className="w-4 h-4 bg-red-500 rounded-full animate-pulse" />
                    <div className="absolute w-6 h-0.5 bg-red-500 rotate-45" />
                    <div className="absolute w-6 h-0.5 bg-red-500 -rotate-45" />
                </motion.div>
            )}

            {/* Miss marker */}
            {isMiss && (
                <motion.div
                    initial={{ scale: 0 }}
                    animate={{ scale: 1 }}
                    className="w-3 h-3 bg-zinc-500 rounded-full opacity-50"
                />
            )}

            {/* Crosshair on hover for enemy grid */}
            {canClick && isEmpty && isEnemy && (
                <Crosshair className="absolute w-4 h-4 text-primary opacity-0 group-hover:opacity-100 transition-opacity" />
            )}
        </motion.button>
    );
});

function GridWithLabels({
    grid,
    title,
    isEnemy,
    ships,
    onCellClick,
    canFire,
}: {
    grid: CellState[][];
    title: string;
    isEnemy: boolean;
    ships?: { x: number; y: number; size: number; vertical: boolean }[];
    onCellClick?: (x: number, y: number) => void;
    canFire: boolean;
}) {
    // For "my grid", overlay ship positions
    const getCellState = (x: number, y: number): CellState => {
        // If it's my grid and there's a ship at this position
        if (!isEnemy && ships) {
            for (const ship of ships) {
                for (let i = 0; i < ship.size; i++) {
                    const sx = ship.vertical ? ship.x : ship.x + i;
                    const sy = ship.vertical ? ship.y + i : ship.y;
                    if (sx === x && sy === y) {
                        // Check if this cell was hit
                        if (grid[y][x] === "hit") return "hit";
                        return "ship";
                    }
                }
            }
        }
        return grid[y][x];
    };

    return (
        <div className="flex flex-col gap-2">
            <h3 className={cn(
                "text-center text-sm font-mono uppercase tracking-widest mb-2",
                isEnemy ? "text-red-500" : "text-blue-500"
            )}>
                {title}
            </h3>

            <div className="flex">
                {/* Row labels */}
                <div className="flex flex-col">
                    <div className="w-6 h-8 sm:h-10" /> {/* Corner spacer */}
                    {Array.from({ length: GRID_SIZE }).map((_, i) => (
                        <div
                            key={i}
                            className="w-6 h-8 sm:h-10 flex items-center justify-center text-xs text-zinc-500 font-mono"
                        >
                            {i + 1}
                        </div>
                    ))}
                </div>

                <div>
                    {/* Column labels */}
                    <div className="flex">
                        {LABELS.map((label) => (
                            <div
                                key={label}
                                className="w-8 h-6 sm:w-10 flex items-center justify-center text-xs text-zinc-500 font-mono"
                            >
                                {label}
                            </div>
                        ))}
                    </div>

                    {/* Grid */}
                    <div className="border border-white/10 bg-black/50 backdrop-blur-sm">
                        {Array.from({ length: GRID_SIZE }).map((_, y) => (
                            <div key={y} className="flex">
                                {Array.from({ length: GRID_SIZE }).map((_, x) => (
                                    <Cell
                                        key={`${x}-${y}`}
                                        state={getCellState(x, y)}
                                        isEnemy={isEnemy}
                                        onClick={isEnemy && onCellClick ? () => onCellClick(x, y) : undefined}
                                        canClick={isEnemy && canFire && getCellState(x, y) === "empty"}
                                    />
                                ))}
                            </div>
                        ))}
                    </div>
                </div>
            </div>
        </div>
    );
}

import { useSound } from "@/context/SoundContext";

export function CombatGrid({ myGrid, enemyGrid, myShips, onFire, canFire }: CombatGridProps) {
    const { playFire } = useSound();

    const handleFire = (x: number, y: number) => {
        if (canFire) {
            playFire(); // Immediate feedback
            onFire(x, y);
        }
    };

    return (
        <div className="flex flex-col lg:flex-row gap-8 lg:gap-16 items-center justify-center p-4">
            {/* Your Fleet */}
            <GridWithLabels
                grid={myGrid}
                title="Your Fleet"
                isEnemy={false}
                ships={myShips}
                canFire={false}
            />

            {/* Divider */}
            <div className="hidden lg:flex flex-col items-center gap-2">
                <div className="w-px h-20 bg-linear-to-b from-transparent via-white/20 to-transparent" />
                <span className="text-xs text-zinc-600 font-mono">VS</span>
                <div className="w-px h-20 bg-linear-to-b from-transparent via-white/20 to-transparent" />
            </div>

            {/* Enemy Waters */}
            <GridWithLabels
                grid={enemyGrid}
                title="Enemy Waters"
                isEnemy={true}
                onCellClick={handleFire}
                canFire={canFire}
            />
        </div>
    );
}
