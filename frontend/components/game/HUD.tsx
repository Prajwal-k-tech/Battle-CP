"use client";

import React from "react";
import { cn } from "@/lib/utils";
import { Flame, Clock, AlertTriangle } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

interface HUDProps {
    heat: number;
    maxHeat: number;
    isLocked: boolean;
    gameTimeRemaining: number;
    vetoTimeRemaining: number | null;
    vetoesRemaining: number;
    maxVetoes: number;
    status: string;
    opponentConnected: boolean;
}

function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
}

function HUDComponent({
    heat,
    maxHeat,
    isLocked,
    gameTimeRemaining,
    vetoTimeRemaining,
    vetoesRemaining,
    maxVetoes,
    status,
    opponentConnected,
}: HUDProps) {
    const heatPercentage = (heat / maxHeat) * 100;
    const isOverheating = heat >= maxHeat - 1;
    const isWarning = heat >= maxHeat - 2;

    return (
        <div className="fixed top-0 left-0 right-0 h-16 bg-black/90 backdrop-blur-md border-b border-white/10 flex items-center justify-between px-6 z-50">
            {/* Left Section: Heat Gauge */}
            <div className="flex items-center gap-4">
                <div className="flex items-center gap-2">
                    <Flame className={cn("w-5 h-5", isOverheating ? "text-red-500 animate-pulse" : "text-orange-500")} />
                    <span className="text-xs text-zinc-400 uppercase tracking-wider font-mono">Heat</span>
                </div>

                <div className="flex items-center gap-3">
                    <div className="w-40 h-3 bg-zinc-800 rounded-full overflow-hidden relative">
                        <motion.div
                            className={cn(
                                "h-full transition-colors",
                                isOverheating ? "bg-red-500" : isWarning ? "bg-orange-500" : "bg-blue-500"
                            )}
                            initial={{ width: 0 }}
                            animate={{ width: `${heatPercentage}%` }}
                            transition={{ type: "spring", stiffness: 100 }}
                        />
                        {isOverheating && (
                            <div className="absolute inset-0 bg-red-500/30 animate-pulse" />
                        )}
                    </div>
                    <span className={cn(
                        "text-sm font-mono font-bold min-w-[40px]",
                        isOverheating ? "text-red-500" : isWarning ? "text-orange-500" : "text-blue-500"
                    )}>
                        {heat}/{maxHeat || 7}
                    </span>
                </div>
            </div>

            {/* Center Section: Timer & Status */}
            <div className="flex flex-col items-center">
                <div className="flex items-center gap-2">
                    <Clock className="w-4 h-4 text-zinc-500" />
                    <span className={cn(
                        "text-3xl font-mono font-bold tracking-wider",
                        gameTimeRemaining < 300 ? "text-red-500 animate-pulse" : "text-white"
                    )}>
                        {formatTime(gameTimeRemaining)}
                    </span>
                </div>
                <span className="text-xs text-zinc-500 uppercase tracking-widest">{status}</span>
            </div>

            {/* Right Section: Veto & Connection */}
            <div className="flex items-center gap-6">
                {/* Veto Status - dots indicating vetoes remaining */}
                <div className="flex items-center gap-2">
                    <span className="text-xs text-zinc-400 uppercase tracking-wider font-mono">Veto</span>
                    {vetoTimeRemaining !== null ? (
                        <span className="text-orange-500 font-mono animate-pulse">
                            {formatTime(vetoTimeRemaining)}
                        </span>
                    ) : (
                        <div className="flex gap-1">
                            {Array.from({ length: maxVetoes || 3 }).map((_, i) => (
                                <div
                                    key={i}
                                    className={cn(
                                        "w-3 h-3 rounded-full border",
                                        i < vetoesRemaining
                                            ? "bg-purple-500 border-purple-400"
                                            : "bg-zinc-800 border-zinc-700"
                                    )}
                                />
                            ))}
                        </div>
                    )}
                </div>

                {/* Connection Status */}
                <div className="flex items-center gap-2">
                    <div className={cn(
                        "w-2 h-2 rounded-full",
                        opponentConnected ? "bg-green-500" : "bg-yellow-500 animate-pulse"
                    )} />
                    <span className="text-xs text-zinc-500 font-mono">
                        {opponentConnected ? "LINKED" : "WAITING"}
                    </span>
                </div>
            </div>

            {/* Compact Locked Indicator - shows below HUD bar */}
            <AnimatePresence>
                {isLocked && (
                    <motion.div
                        initial={{ opacity: 0, y: -10 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: -10 }}
                        className="absolute left-0 right-0 top-full mt-2 flex justify-center"
                    >
                        <div className="bg-red-900/90 px-4 py-2 rounded-full border border-red-500/50 flex items-center gap-2 backdrop-blur-sm">
                            <AlertTriangle className="w-4 h-4 text-red-400 animate-pulse" />
                            <span className="text-sm font-bold text-red-400 uppercase tracking-wider">
                                Weapons Locked
                            </span>
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
}

export const HUD = React.memo(HUDComponent);
