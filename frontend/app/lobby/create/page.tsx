"use client";

import { MemoizedFaultyTerminal } from "@/components/ui/FaultyTerminal";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ArrowRight, Loader2, Copy, Terminal, User, Settings as SettingsIcon } from "lucide-react";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { Slider } from "@/components/ui/slider";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { useState } from "react";

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:3000";

// Stable constant for FaultyTerminal to prevent re-renders
const GRID_MUL: [number, number] = [2, 1];

import { useSound } from "@/context/SoundContext";

export default function CreateGamePage() {
    const router = useRouter();
    const [cfHandle, setCfHandle] = useState("");
    const [isCreating, setIsCreating] = useState(false);
    const { playJoin, playSuccess, playShipPlace } = useSound();
    const [gameId, setGameId] = useState<string | null>(null);

    // Game Settings
    const [difficulty, setDifficulty] = useState(800);
    const [timeLimit, setTimeLimit] = useState(45); // minutes
    const [heatThreshold, setHeatThreshold] = useState(7); // shots before overheat
    const [vetoStrictness, setVetoStrictness] = useState<"low" | "medium" | "high">("medium");

    const handleCreate = async () => {
        if (!cfHandle.trim()) {
            toast.error("Please enter your Codeforces handle");
            return;
        }

        setIsCreating(true);

        try {
            const res = await fetch(`${API_BASE_URL}/api/game`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    cf_handle: cfHandle.trim(),
                    difficulty,
                    heat_threshold: heatThreshold,
                    game_duration_mins: timeLimit,
                    veto_strictness: vetoStrictness,
                }),
            });

            if (!res.ok) {
                const errorData = await res.json().catch(() => ({}));
                throw new Error(errorData.error || "Failed to create game");
            }

            const data = await res.json();
            const newGameId = data.game_id;
            const newPlayerId = data.player_id;

            // Store credentials
            localStorage.setItem("battlecp_player_id", newPlayerId);
            localStorage.setItem("battlecp_cf_handle", cfHandle.trim());

            toast.success("Uplink Established", { description: `Lobby ID: ${newGameId}` });

            // Auto-redirect to game page - P1 enters immediately
            // Don't set gameId state - it causes a flash of the old UI before redirect
            router.push(`/game/${newGameId}`);
        } catch (error) {
            console.error("Create game error:", error);

            if (error instanceof Error && error.message.includes("handle not found")) {
                toast.error("Codeforces handle not found", { description: "Please check your handle and try again" });
                setIsCreating(false);
                return;
            }

            // Fallback for demo/development when backend isn't running
            const fallbackGameId = crypto.randomUUID();
            const fallbackPlayerId = crypto.randomUUID();

            localStorage.setItem("battlecp_player_id", fallbackPlayerId);
            localStorage.setItem("battlecp_cf_handle", cfHandle.trim());

            toast.warning("Using offline mode (backend unavailable)");

            // Auto-redirect even in fallback mode
            // Don't set gameId state - it causes a flash of the old UI before redirect
            router.push(`/game/${fallbackGameId}`);
        } finally {
            setIsCreating(false);
        }
    };

    const copyToClipboard = () => {
        if (gameId) {
            navigator.clipboard.writeText(gameId);
            toast.info("Copied to clipboard");
        }
    };

    const enterLobby = () => {
        if (gameId) {
            router.push(`/game/${gameId}`);
        }
    };

    return (
        <div className="relative min-h-screen w-full flex items-center justify-center bg-black overflow-hidden">
            {/* Background */}
            <div className="absolute inset-0 z-0 opacity-40">
                <MemoizedFaultyTerminal
                    scale={1.5}
                    gridMul={GRID_MUL}
                    digitSize={1.2}
                    timeScale={0.5}
                    scanlineIntensity={0.5}
                    noiseAmp={1}
                    curvature={0.1}
                    tint="#10b981"
                    mouseReact={true}
                    mouseStrength={0.5}
                    brightness={0.6}
                />
            </div>

            <div className="relative z-10 w-full max-w-md p-6">
                <Card className="border-emerald-500/20 bg-black/80 backdrop-blur-xl">
                    <CardHeader>
                        <CardTitle className="text-2xl font-arcade text-emerald-500 tracking-tighter">INITIATE UPLINK</CardTitle>
                        <CardDescription className="font-mono">Establish a secure connection for 1v1 combat.</CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-6">
                        {!gameId ? (
                            <div className="space-y-4">
                                {/* CF Handle Input */}
                                <div className="space-y-2">
                                    <label className="text-sm text-zinc-400 flex items-center gap-2 font-mono">
                                        <User className="w-4 h-4" />
                                        Codeforces Handle
                                    </label>
                                    <div className="relative">
                                        <Terminal className="absolute left-3 top-3 h-5 w-5 text-emerald-500/50" />
                                        <Input
                                            className="pl-10 h-12 bg-black/50 border-white/10 focus-visible:ring-emerald-500 font-mono text-lg"
                                            placeholder="e.g. tourist"
                                            value={cfHandle}
                                            onChange={(e) => setCfHandle(e.target.value)}
                                            autoFocus
                                        />
                                    </div>
                                    <p className="text-xs text-zinc-600 font-mono">
                                        Used to verify your problem submissions
                                    </p>
                                </div>

                                {/* Combat Protocol */}
                                <div className="space-y-2">
                                    <div className="flex justify-between items-center">
                                        <label className="text-sm text-zinc-400 font-mono">Combat Protocol</label>
                                        <Dialog>
                                            <DialogTrigger asChild>
                                                <Button variant="ghost" size="sm" className="h-6 w-6 p-0 hover:bg-emerald-500/20" onClick={() => playShipPlace()}>
                                                    <SettingsIcon className="w-3 h-3 text-emerald-400" />
                                                </Button>
                                            </DialogTrigger>
                                            <DialogContent className="border-emerald-500/20 bg-black/90 text-white font-arcade sm:max-w-sm">
                                                <DialogHeader>
                                                    <DialogTitle>Protocol Settings</DialogTitle>
                                                    <DialogDescription>Customize game parameters.</DialogDescription>
                                                </DialogHeader>
                                                <div className="py-4 space-y-5">
                                                    {/* Problem Difficulty */}
                                                    <div className="space-y-2">
                                                        <div className="flex justify-between text-xs font-mono">
                                                            <span className="text-zinc-400">Problem Difficulty</span>
                                                            <span className="text-emerald-400">{difficulty}</span>
                                                        </div>
                                                        <Slider
                                                            min={800}
                                                            max={2000}
                                                            step={100}
                                                            value={[difficulty]}
                                                            onValueChange={(v) => setDifficulty(v[0])}
                                                            className="py-1"
                                                        />
                                                    </div>

                                                    {/* Time Limit */}
                                                    <div className="space-y-2">
                                                        <div className="flex justify-between text-xs font-mono">
                                                            <span className="text-zinc-400">Time Limit</span>
                                                            <span className="text-emerald-400">{timeLimit} min</span>
                                                        </div>
                                                        <Slider
                                                            min={1}
                                                            max={90}
                                                            step={1}
                                                            value={[timeLimit]}
                                                            onValueChange={(v) => setTimeLimit(v[0])}
                                                            className="py-1"
                                                        />
                                                    </div>

                                                    {/* Heat Threshold */}
                                                    <div className="space-y-2">
                                                        <div className="flex justify-between text-xs font-mono">
                                                            <span className="text-zinc-400">Overheat After</span>
                                                            <span className="text-emerald-400">{heatThreshold} shots</span>
                                                        </div>
                                                        <Slider
                                                            min={3}
                                                            max={15}
                                                            step={1}
                                                            value={[heatThreshold]}
                                                            onValueChange={(v) => setHeatThreshold(v[0])}
                                                            className="py-1"
                                                        />
                                                    </div>

                                                    {/* Veto Strictness */}
                                                    <div className="space-y-2">
                                                        <span className="text-xs font-mono text-zinc-400">Veto Penalty</span>
                                                        <div className="flex gap-2">
                                                            {(["low", "medium", "high"] as const).map((level) => (
                                                                <Button
                                                                    key={level}
                                                                    variant={vetoStrictness === level ? "default" : "outline"}
                                                                    size="sm"
                                                                    className={`flex-1 text-xs capitalize ${vetoStrictness === level ? "bg-emerald-600" : "border-white/10"}`}
                                                                    onClick={() => setVetoStrictness(level)}
                                                                >
                                                                    {level}
                                                                </Button>
                                                            ))}
                                                        </div>
                                                        <p className="text-[10px] text-zinc-500 font-mono">
                                                            {vetoStrictness === "low" ? "5/7/10 min" : vetoStrictness === "high" ? "10/15/20 min" : "7/10/15 min"}
                                                        </p>
                                                    </div>
                                                </div>
                                            </DialogContent>
                                        </Dialog>
                                    </div>
                                    <div className="p-3 border border-white/10 rounded-md bg-white/5 flex justify-between items-center">
                                        <span className="text-zinc-300 font-mono text-xs">{difficulty} • {timeLimit}m • {heatThreshold}🔥</span>
                                        <span className="text-[10px] bg-emerald-500/20 text-emerald-400 px-2 py-1 rounded font-arcade">Custom</span>
                                    </div>
                                </div>

                                <Button
                                    className="w-full h-12 text-lg bg-emerald-600 hover:bg-emerald-700 font-arcade tracking-wider"
                                    onClick={() => { playJoin(); handleCreate(); }}
                                    disabled={isCreating || !cfHandle.trim()}
                                >
                                    {isCreating ? (
                                        <Loader2 className="animate-spin mr-2" />
                                    ) : (
                                        <ArrowRight className="mr-2" />
                                    )}
                                    GENERATE LOBBY
                                </Button>
                            </div>
                        ) : (
                            <div className="space-y-6 animate-in fade-in slide-in-from-bottom-4">
                                <div className="p-4 bg-emerald-500/10 border border-emerald-500/30 rounded-lg text-center space-y-2">
                                    <p className="text-[10px] uppercase tracking-widest text-emerald-400 font-arcade">Secure Frequency</p>
                                    <div className="text-2xl sm:text-4xl font-mono font-bold text-white tracking-widest break-all">{gameId}</div>
                                </div>

                                <p className="text-center text-xs text-zinc-500 font-mono">
                                    Share this code with your opponent to start the battle.
                                </p>

                                <div className="flex gap-2">
                                    <Button
                                        variant="outline"
                                        className="flex-1 border-white/10 hover:bg-white/5 font-arcade text-xs"
                                        onClick={() => { playShipPlace(); copyToClipboard(); }}
                                    >
                                        <Copy className="w-3 h-3 mr-2" /> COPY ID
                                    </Button>
                                    <Button
                                        variant="default"
                                        className="flex-1 bg-emerald-600 hover:bg-emerald-700 font-arcade text-xs"
                                        onClick={() => { playSuccess(); enterLobby(); }}
                                    >
                                        ENTER <ArrowRight className="w-3 h-3 ml-2" />
                                    </Button>
                                </div>
                            </div>
                        )}
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}
