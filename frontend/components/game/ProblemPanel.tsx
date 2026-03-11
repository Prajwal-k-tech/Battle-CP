"use client";

import React, { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { motion, AnimatePresence } from "framer-motion";
import { ExternalLink, Check, Clock, Loader2 } from "lucide-react";
import { toast } from "sonner";

interface ProblemPanelProps {
    cfHandle: string;
    isLocked: boolean;
    difficulty: number;
    vetoesRemaining: number;
    maxVetoes: number;
    vetoTimeRemaining: number | null;
    // Server-assigned problem — the server is the single source of truth
    activeProblemContestId: number | null;
    activeProblemIndex: string | null;
    activeProblemName: string | null;
    activeProblemRating: number | null;
    onSolve: (contestId: number, problemIndex: string) => void;
    onVeto: () => void;
}

import { useSound } from "@/context/SoundContext";

export function ProblemPanel({
    isLocked,
    vetoesRemaining,
    maxVetoes,
    vetoTimeRemaining,
    activeProblemContestId,
    activeProblemIndex,
    activeProblemName,
    activeProblemRating,
    onSolve,
    onVeto,
}: ProblemPanelProps) {
    const [verifying, setVerifying] = useState(false);
    const [verifyCooldown, setVerifyCooldown] = useState(0);
    const [localVetoTime, setLocalVetoTime] = useState<number | null>(null);
    const { playAlarm, playInvalid: playVeto } = useSound();

    // Play alarm when locked
    useEffect(() => {
        if (isLocked) {
            playAlarm();
        }
    }, [isLocked, playAlarm]);

    // Sync veto timer from server
    useEffect(() => {
        if (vetoTimeRemaining !== null && vetoTimeRemaining > 0) {
            setLocalVetoTime(vetoTimeRemaining);
        }
    }, [vetoTimeRemaining]);

    // Local veto countdown
    useEffect(() => {
        if (!isLocked || localVetoTime === null) return;

        const timer = setInterval(() => {
            setLocalVetoTime(prev => {
                if (prev === null || prev <= 1) {
                    clearInterval(timer);
                    return null;
                }
                return prev - 1;
            });
        }, 1000);

        return () => clearInterval(timer);
    }, [isLocked]); // Only re-run when lock state changes

    // Reset state when unlocked
    useEffect(() => {
        if (!isLocked) {
            setLocalVetoTime(null);
            setVerifyCooldown(0);
            setVerifying(false);
        }
    }, [isLocked]);

    const handleVerify = async () => {
        if (!activeProblemContestId || !activeProblemIndex || verifyCooldown > 0) return;
        setVerifying(true);
        setVerifyCooldown(10); // Match backend's 10-second cooldown
        try {
            onSolve(activeProblemContestId, activeProblemIndex);
            toast.info("Verifying submission...");
        } finally {
            setTimeout(() => setVerifying(false), 3000);
        }
    };

    // Countdown timer for verify cooldown
    useEffect(() => {
        if (verifyCooldown <= 0) return;
        const timer = setTimeout(() => setVerifyCooldown(c => c - 1), 1000);
        return () => clearTimeout(timer);
    }, [verifyCooldown]);

    const handleVeto = () => {
        if (vetoesRemaining <= 0) {
            toast.error("No vetoes remaining!");
            return;
        }
        if (localVetoTime !== null && localVetoTime > 0) {
            toast.error("Already on veto timer!");
            return;
        }
        playVeto();
        onVeto();
    };

    if (!isLocked) return null;

    const formatTime = (secs: number) => {
        const mins = Math.floor(secs / 60);
        const s = secs % 60;
        return `${mins}:${String(s).padStart(2, "0")}`;
    };

    const hasProblem = activeProblemContestId !== null && activeProblemIndex !== null;

    return (
        <AnimatePresence>
            {isLocked && (
                <motion.div
                    initial={{ x: 400, opacity: 0 }}
                    animate={{ x: 0, opacity: 1 }}
                    exit={{ x: 400, opacity: 0 }}
                    transition={{ type: "spring", damping: 25, stiffness: 200 }}
                    className="fixed right-0 top-20 bottom-4 w-80 z-40 flex flex-col"
                >
                    <div className="flex-1 bg-zinc-950/95 backdrop-blur-lg rounded-l-2xl border-l border-y border-red-500/40 shadow-2xl shadow-red-500/10 overflow-hidden flex flex-col">
                        {/* Header */}
                        <div className="px-4 py-3 bg-red-900/30 border-b border-red-500/30">
                            <div className="flex items-center gap-2">
                                <div className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
                                <span className="text-sm font-bold text-red-400 uppercase tracking-wider">
                                    ⚠ Overheated
                                </span>
                            </div>
                        </div>

                        {/* Content */}
                        <div className="flex-1 p-4 overflow-y-auto">
                            {/* During veto penalty — no problem, just waiting */}
                            {localVetoTime !== null && localVetoTime > 0 ? (
                                <div className="flex flex-col items-center justify-center h-full gap-4 text-center">
                                    <Clock className="w-10 h-10 text-purple-400 animate-pulse" />
                                    <div>
                                        <p className="text-sm font-bold text-purple-400 mb-1">VETO PENALTY</p>
                                        <p className="text-2xl font-mono font-bold text-white">
                                            {formatTime(localVetoTime)}
                                        </p>
                                    </div>
                                    <p className="text-xs text-zinc-500">
                                        New problem assigned when timer expires.
                                        You must solve it to unlock weapons.
                                    </p>
                                </div>
                            ) : !hasProblem ? (
                                <div className="flex flex-col items-center justify-center h-full gap-3">
                                    <Loader2 className="w-8 h-8 animate-spin text-red-400" />
                                    <span className="text-xs text-zinc-500">
                                        Server is assigning a problem...
                                    </span>
                                </div>
                            ) : (
                                <div className="space-y-4">
                                    {/* Problem Info */}
                                    <div>
                                        <div className="flex items-center justify-between mb-1">
                                            <span className="text-xs font-mono text-zinc-400">
                                                {activeProblemContestId}{activeProblemIndex}
                                            </span>
                                            {activeProblemRating && (
                                                <span className="text-xs font-mono text-blue-400 bg-blue-500/10 px-2 py-0.5 rounded">
                                                    {activeProblemRating}
                                                </span>
                                            )}
                                        </div>
                                        <h3 className="text-base font-bold text-white leading-tight">
                                            {activeProblemName || `Problem ${activeProblemIndex}`}
                                        </h3>
                                    </div>

                                    {/* Open Button */}
                                    <a
                                        href={`https://codeforces.com/problemset/problem/${activeProblemContestId}/${activeProblemIndex}`}
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        className="flex items-center justify-center gap-2 w-full py-3 bg-blue-600 hover:bg-blue-500 rounded-lg text-white font-bold text-sm transition"
                                    >
                                        <ExternalLink className="w-4 h-4" />
                                        Open on Codeforces
                                    </a>

                                    {/* Instructions */}
                                    <p className="text-xs text-zinc-500 leading-relaxed">
                                        Solve on Codeforces using your handle.
                                        Get <span className="text-green-400">Accepted</span>, then verify.
                                    </p>

                                    {/* Actions */}
                                    <div className="space-y-2">
                                        <Button
                                            onClick={handleVerify}
                                            disabled={verifying || verifyCooldown > 0 || (localVetoTime !== null && localVetoTime > 0)}
                                            className="w-full h-11 bg-green-600 hover:bg-green-500 font-bold"
                                        >
                                            {verifying ? (
                                                <>
                                                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                                    Verifying...
                                                </>
                                            ) : verifyCooldown > 0 ? (
                                                <>
                                                    <Clock className="w-4 h-4 mr-2" />
                                                    Wait {verifyCooldown}s
                                                </>
                                            ) : (
                                                <>
                                                    <Check className="w-4 h-4 mr-2" />
                                                    Verify Solution
                                                </>
                                            )}
                                        </Button>

                                        <Button
                                            onClick={handleVeto}
                                            disabled={vetoesRemaining <= 0 || (localVetoTime !== null && localVetoTime > 0)}
                                            variant="outline"
                                            className="w-full h-11 border-purple-500/50 text-purple-400 hover:bg-purple-500/10"
                                        >
                                            {localVetoTime !== null && localVetoTime > 0 ? (
                                                <div className="flex items-center gap-2">
                                                    <Clock className="w-4 h-4 animate-pulse" />
                                                    <span>Wait {formatTime(localVetoTime)}</span>
                                                </div>
                                            ) : (
                                                <div className="flex items-center gap-2">
                                                    <span>Veto ({vetoesRemaining}/{maxVetoes})</span>
                                                </div>
                                            )}
                                        </Button>
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                </motion.div>
            )}
        </AnimatePresence>
    );
}
