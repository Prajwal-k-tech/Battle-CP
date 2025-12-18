"use client";

import React, { useEffect, useState, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { motion, AnimatePresence } from "framer-motion";
import { ExternalLink, Check, Clock, Loader2 } from "lucide-react";
import { toast } from "sonner";

interface Problem {
    contestId: number;
    index: string;
    name: string;
    rating?: number;
}

// Module-level problem cache to avoid repeated API calls across overheat events
// Persists for the lifetime of the tab, even across component re-renders
const problemCache: Map<number, Problem[]> = new Map();

interface ProblemPanelProps {
    isLocked: boolean;
    difficulty: number;
    vetoesRemaining: number;
    maxVetoes: number;
    vetoTimeRemaining: number | null;
    onSolve: (contestId: number, problemIndex: string) => void;
    onVeto: () => void;
}

import { useSound } from "@/context/SoundContext";

export function ProblemPanel({
    isLocked,
    difficulty,
    vetoesRemaining,
    maxVetoes,
    vetoTimeRemaining,
    onSolve,
    onVeto,
}: ProblemPanelProps) {
    const [problem, setProblem] = useState<Problem | null>(null);
    const [loading, setLoading] = useState(false);
    const [verifying, setVerifying] = useState(false);
    const [verifyCooldown, setVerifyCooldown] = useState(0); // Seconds remaining before can verify again
    const [localVetoTime, setLocalVetoTime] = useState<number | null>(null);
    const { playAlarm, playInvalid: playVeto } = useSound();

    // Play alarm when locked
    useEffect(() => {
        if (isLocked) {
            playAlarm();
        }
    }, [isLocked, playAlarm]);

    // ... rest of timer logic
    useEffect(() => {
        if (vetoTimeRemaining !== null && vetoTimeRemaining > 0) {
            setLocalVetoTime(vetoTimeRemaining);
        }
    }, [vetoTimeRemaining]);

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
    }, [isLocked]); // Only re-run when lock state changes - timer manages itself

    // ... fetchProblem logic unchanged

    const fetchProblem = useCallback(async () => {
        setLoading(true);
        try {
            // Check cache first
            let problems = problemCache.get(difficulty);

            if (!problems) {
                // Cache miss - fetch from API
                const response = await fetch(
                    `https://codeforces.com/api/problemset.problems`
                );

                if (!response.ok) throw new Error("Failed to fetch problems");

                const data = await response.json();
                if (data.status !== "OK") throw new Error("Codeforces API error");

                // Filter and cache ALL difficulty levels at once to minimize future calls
                const allProblems = data.result.problems as Problem[];
                const byRating = new Map<number, Problem[]>();

                for (const p of allProblems) {
                    if (p.rating) {
                        const existing = byRating.get(p.rating) || [];
                        if (existing.length < 100) { // Cap at 100 per rating
                            existing.push(p);
                            byRating.set(p.rating, existing);
                        }
                    }
                }

                // Store all ratings in cache
                byRating.forEach((probs, rating) => {
                    problemCache.set(rating, probs);
                });

                problems = problemCache.get(difficulty) || [];
            }

            if (!problems || problems.length === 0) {
                throw new Error(`No problems found at rating ${difficulty}`);
            }

            // Pick a random one from cache
            const randomProblem = problems[Math.floor(Math.random() * problems.length)];
            setProblem(randomProblem);
        } catch (error) {
            console.error("Failed to fetch problem:", error);
            toast.error("Failed to load problem from Codeforces");

            // Fallback problems by rating - used when CF API is down
            const fallbacksByRating: Record<number, Problem[]> = {
                800: [
                    { contestId: 1950, index: "A", name: "Stair, Peak, or Neither?", rating: 800 },
                    { contestId: 1950, index: "B", name: "Upscaling", rating: 800 },
                ],
                1000: [
                    { contestId: 1941, index: "B", name: "Rudolf and 121", rating: 1000 },
                    { contestId: 1937, index: "B", name: "Binary Path", rating: 1000 },
                ],
                1200: [
                    { contestId: 1941, index: "C", name: "Rudolf and the Ugly String", rating: 1200 },
                    { contestId: 1937, index: "C", name: "Bitwise Operation Wizard", rating: 1200 },
                ],
                1400: [
                    { contestId: 1941, index: "D", name: "Rudolf and the Ball Game", rating: 1400 },
                    { contestId: 1929, index: "C", name: "Sasha and the Casino", rating: 1400 },
                ],
                1600: [
                    { contestId: 1929, index: "D", name: "Sasha and a Walk in the City", rating: 1600 },
                ],
            };
            // Find closest available rating
            const availableRatings = Object.keys(fallbacksByRating).map(Number).sort((a, b) => a - b);
            const closestRating = availableRatings.reduce((prev, curr) =>
                Math.abs(curr - difficulty) < Math.abs(prev - difficulty) ? curr : prev
            );
            const fallbackProblems = fallbacksByRating[closestRating] || fallbacksByRating[800];
            toast.warning(`Using fallback problem (rating ${closestRating})`);
            setProblem(fallbackProblems[Math.floor(Math.random() * fallbackProblems.length)]);
        } finally {
            setLoading(false);
        }
    }, [difficulty]);

    useEffect(() => {
        if (isLocked && !problem) {
            fetchProblem();
        } else if (!isLocked) {
            setProblem(null);
            setLocalVetoTime(null);
            // If we just unlocked, plays sound? Handled by backend message but here we can too
        }
    }, [isLocked, problem, fetchProblem]);

    const handleVerify = async () => {
        if (!problem || verifyCooldown > 0) return;
        setVerifying(true);
        setVerifyCooldown(30);
        try {
            onSolve(problem.contestId, problem.index);
            toast.info("Verifying submission...");
            // Backend will send confirmation which unlocks weapons -> triggers effect cleanup
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
                            {loading ? (
                                <div className="flex flex-col items-center justify-center h-full gap-3">
                                    <Loader2 className="w-8 h-8 animate-spin text-red-400" />
                                    <span className="text-xs text-zinc-500">Loading problem...</span>
                                </div>
                            ) : problem ? (
                                <div className="space-y-4">
                                    {/* Problem Info */}
                                    <div>
                                        <div className="flex items-center justify-between mb-1">
                                            <span className="text-xs font-mono text-zinc-400">
                                                {problem.contestId}{problem.index}
                                            </span>
                                            {problem.rating && (
                                                <span className="text-xs font-mono text-blue-400 bg-blue-500/10 px-2 py-0.5 rounded">
                                                    {problem.rating}
                                                </span>
                                            )}
                                        </div>
                                        <h3 className="text-base font-bold text-white leading-tight">
                                            {problem.name}
                                        </h3>
                                    </div>

                                    {/* Open Button */}
                                    <a
                                        href={`https://codeforces.com/problemset/problem/${problem.contestId}/${problem.index}`}
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
                            ) : (
                                <div className="flex flex-col items-center justify-center h-full gap-3">
                                    <span className="text-xs text-zinc-500">No problem loaded</span>
                                    <Button onClick={fetchProblem} variant="outline" size="sm">
                                        Load Problem
                                    </Button>
                                </div>
                            )}
                        </div>
                    </div>
                </motion.div>
            )
            }
        </AnimatePresence >
    );
}
