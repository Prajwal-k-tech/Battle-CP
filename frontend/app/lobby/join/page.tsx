"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ArrowRight, Terminal, User } from "lucide-react";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { MemoizedFaultyTerminal } from "@/components/ui/FaultyTerminal";

// Stable constant for FaultyTerminal to prevent re-renders
const GRID_MUL: [number, number] = [2, 1];

import { useSound } from "@/context/SoundContext";

export default function JoinGamePage() {
    const router = useRouter();
    const [lobbyId, setLobbyId] = useState("");
    const [cfHandle, setCfHandle] = useState("");
    const { playJoin } = useSound();

    const handleJoin = (e: React.FormEvent) => {
        e.preventDefault();

        if (!cfHandle.trim()) {
            toast.error("Please enter your Codeforces handle");
            return;
        }

        // Validate lobby code format (UUIDs are 36 characters with dashes)
        const trimmedLobbyId = lobbyId.trim();
        const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
        if (!uuidRegex.test(trimmedLobbyId)) {
            toast.error("Invalid lobby code format", { description: "Please enter a valid game code" });
            return;
        }

        // Generate a proper UUID for player ID (must match backend UUID format)
        const newPlayerId = crypto.randomUUID();
        localStorage.setItem("battlecp_player_id", newPlayerId);
        localStorage.setItem("battlecp_cf_handle", cfHandle.trim());

        router.push(`/game/${lobbyId.trim()}`);
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
                    tint="#a855f7"
                    mouseReact={true}
                    mouseStrength={0.5}
                    brightness={0.6}
                />
            </div>

            <div className="relative z-10 w-full max-w-md p-6">
                <Card className="border-purple-500/20 bg-black/80 backdrop-blur-xl">
                    <CardHeader>
                        <CardTitle className="text-2xl font-heading text-purple-500">JOIN EXISTING</CardTitle>
                        <CardDescription>Enter the frequency code to join a lobby.</CardDescription>
                    </CardHeader>
                    <CardContent>
                        <form onSubmit={handleJoin} className="space-y-4">
                            {/* CF Handle Input */}
                            <div className="space-y-2">
                                <label className="text-sm text-zinc-400 flex items-center gap-2">
                                    <User className="w-4 h-4" />
                                    Codeforces Handle
                                </label>
                                <Input
                                    className="h-12 bg-black/50 border-white/10 focus-visible:ring-purple-500 font-mono"
                                    placeholder="e.g. tourist"
                                    value={cfHandle}
                                    onChange={(e) => setCfHandle(e.target.value)}
                                />
                            </div>

                            {/* Lobby Code Input */}
                            <div className="space-y-2">
                                <label className="text-sm font-mono text-zinc-400">FREQUENCY CODE</label>
                                <div className="relative">
                                    <Terminal className="absolute left-3 top-3 h-5 w-5 text-purple-500/50" />
                                    <Input
                                        className="pl-10 h-12 bg-black/50 border-white/10 focus-visible:ring-purple-500 font-mono text-lg uppercase placeholder:normal-case"
                                        placeholder="e.g. X7K9P"
                                        value={lobbyId}
                                        onChange={(e) => setLobbyId(e.target.value)}
                                    />
                                </div>
                            </div>

                            <Button
                                type="submit"
                                className="w-full h-12 text-lg bg-purple-600 hover:bg-purple-700"
                                disabled={lobbyId.length < 3 || !cfHandle.trim()}
                                onClick={() => playJoin()}
                            >
                                CONNECT <ArrowRight className="w-4 h-4 ml-2" />
                            </Button>
                        </form>
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}
