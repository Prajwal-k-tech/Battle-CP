"use client";

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ArrowRight, Terminal, User } from "lucide-react";
import { useRouter, useSearchParams } from "next/navigation";
import { toast } from "sonner";
import { MemoizedFaultyTerminal } from "@/components/ui/FaultyTerminal";

// Stable constant for FaultyTerminal to prevent re-renders
const GRID_MUL: [number, number] = [2, 1];

import { useSound } from "@/context/SoundContext";

export default function JoinGamePage() {
    const router = useRouter();
    const searchParams = useSearchParams();
    const [lobbyId, setLobbyId] = useState("");
    const [cfHandle, setCfHandle] = useState("");
    
    // Pre-fill lobby ID from redirect query param and CF handle from localStorage
    useEffect(() => {
        const redirectGameId = searchParams.get("redirect");
        if (redirectGameId) {
            setLobbyId(redirectGameId);
        }
        
        // Pre-fill CF handle if user has one stored
        const storedHandle = localStorage.getItem("battlecp_cf_handle");
        if (storedHandle) {
            setCfHandle(storedHandle);
        }
    }, [searchParams]);
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

        // RECONNECTION SUPPORT: Reuse existing player_id if same CF handle
        // This allows the same person to rejoin from a different tab
        const storedHandle = localStorage.getItem("battlecp_cf_handle");
        const storedPlayerId = localStorage.getItem("battlecp_player_id");
        
        let playerId: string;
        if (storedHandle?.toLowerCase() === cfHandle.trim().toLowerCase() && storedPlayerId) {
            // Same person, reuse their ID for reconnection
            playerId = storedPlayerId;
        } else {
            // Different person or first time, generate new ID
            playerId = crypto.randomUUID();
            localStorage.setItem("battlecp_player_id", playerId);
        }
        
        localStorage.setItem("battlecp_cf_handle", cfHandle.trim());
        localStorage.setItem("battlecp_active_game", trimmedLobbyId);

        router.push(`/game/${trimmedLobbyId}`);
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
