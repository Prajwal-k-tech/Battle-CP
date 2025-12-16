"use client";

import React from "react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { motion, AnimatePresence } from "framer-motion";
import { Trophy, Skull, Home, RotateCcw, Swords, Target, Brain } from "lucide-react";
import Link from "next/link";

interface VictoryModalProps {
    isOpen: boolean;
    isWinner: boolean;
    reason: string;
    stats?: {
        shotsHit: number;
        shotsMissed: number;
        problemsSolved: number;
        enemyShipsSunk: number;
    };
}

const reasonLabels: Record<string, string> = {
    AllShipsSunk: "All Enemy Ships Destroyed",
    Timeout: "Time Limit Reached",
    SuddenDeath: "Sudden Death Victory",
    Disconnect: "Opponent Disconnected",
};

export function VictoryModal({ isOpen, isWinner, reason, stats }: VictoryModalProps) {
    if (!isOpen) return null;

    return (
        <AnimatePresence>
            <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="fixed inset-0 z-100 flex items-center justify-center bg-black/80 backdrop-blur-md"
            >
                <motion.div
                    initial={{ scale: 0.8, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    transition={{ type: "spring", duration: 0.5 }}
                >
                    <Card className={cn(
                        "w-[400px] border-2 bg-black/90 backdrop-blur-xl",
                        isWinner ? "border-emerald-500/50" : "border-red-500/50"
                    )}>
                        <CardHeader className="text-center pb-2">
                            {/* Icon */}
                            <motion.div
                                initial={{ scale: 0 }}
                                animate={{ scale: 1, rotate: [0, 10, -10, 0] }}
                                transition={{ delay: 0.2, duration: 0.5 }}
                                className="mx-auto mb-4"
                            >
                                {isWinner ? (
                                    <div className="w-20 h-20 rounded-full bg-emerald-500/20 flex items-center justify-center">
                                        <Trophy className="w-10 h-10 text-emerald-500" />
                                    </div>
                                ) : (
                                    <div className="w-20 h-20 rounded-full bg-red-500/20 flex items-center justify-center">
                                        <Skull className="w-10 h-10 text-red-500" />
                                    </div>
                                )}
                            </motion.div>

                            {/* Title */}
                            <CardTitle className={cn(
                                "text-4xl font-heading tracking-widest",
                                isWinner ? "text-emerald-500" : "text-red-500"
                            )}>
                                {isWinner ? "VICTORY" : "DEFEAT"}
                            </CardTitle>

                            {/* Reason */}
                            <p className="text-sm text-zinc-400 mt-2">
                                {reasonLabels[reason] || reason}
                            </p>
                        </CardHeader>

                        <CardContent className="space-y-6">
                            {/* Stats */}
                            {stats && (
                                <div className="grid grid-cols-2 gap-4">
                                    <StatCard icon={<Target className="w-4 h-4" />} label="Hits" value={stats.shotsHit} />
                                    <StatCard icon={<Swords className="w-4 h-4" />} label="Ships Sunk" value={stats.enemyShipsSunk} />
                                    <StatCard icon={<Brain className="w-4 h-4" />} label="Problems" value={stats.problemsSolved} />
                                    <StatCard
                                        icon={<Target className="w-4 h-4 opacity-50" />}
                                        label="Accuracy"
                                        value={`${stats.shotsHit + stats.shotsMissed > 0
                                            ? Math.round((stats.shotsHit / (stats.shotsHit + stats.shotsMissed)) * 100)
                                            : 0}%`}
                                    />
                                </div>
                            )}

                            {/* Actions */}
                            <div className="flex gap-3 pt-4">
                                <Link href="/" className="flex-1">
                                    <Button variant="outline" className="w-full border-white/10">
                                        <Home className="w-4 h-4 mr-2" />
                                        Home
                                    </Button>
                                </Link>
                                <Link href="/" className="flex-1">
                                    <Button
                                        className={cn(
                                            "w-full",
                                            isWinner ? "bg-emerald-600 hover:bg-emerald-700" : "bg-red-600 hover:bg-red-700"
                                        )}
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

function StatCard({ icon, label, value }: { icon: React.ReactNode; label: string; value: string | number }) {
    return (
        <div className="bg-white/5 rounded-lg p-3 text-center">
            <div className="flex items-center justify-center gap-1 text-zinc-500 mb-1">
                {icon}
                <span className="text-xs uppercase">{label}</span>
            </div>
            <div className="text-xl font-mono font-bold text-white">{value}</div>
        </div>
    );
}
