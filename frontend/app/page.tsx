"use client";

import { MemoizedFaultyTerminal } from "@/components/ui/FaultyTerminal";
import { Button } from "@/components/ui/button";
import { Terminal, Users, ArrowRight } from "lucide-react";
import Link from "next/link";
import { motion } from "framer-motion";
import { useSound } from "@/context/SoundContext";
import { useMusic } from "@/context/MusicContext";
import { useEffect, useState } from "react";

// Stable constant for FaultyTerminal to prevent re-renders
const GRID_MUL: [number, number] = [2, 1];

export default function Home() {
  const { playJoin, playSuccess } = useSound();
  const { setPhase } = useMusic();
  const [activeGameId, setActiveGameId] = useState<string | null>(null);

  // Check for active game session on mount
  useEffect(() => {
    const activeGame = localStorage.getItem("battlecp_active_game");
    if (activeGame) {
      setActiveGameId(activeGame);
    }
  }, []);

  // Start menu music on page load
  useEffect(() => {
    setPhase("menu");
  }, [setPhase]);

  const handleAbandonGame = () => {
    localStorage.removeItem("battlecp_active_game");
    setActiveGameId(null);
  };

  return (
    <div className="relative min-h-screen w-full overflow-hidden bg-black text-white selection:bg-primary/30">
      {/* Active Game Banner */}
      {activeGameId && (
        <div className="fixed top-0 left-0 right-0 z-50 bg-yellow-500 text-black py-2 px-4 flex items-center justify-center gap-4 pointer-events-auto">
          <span className="font-mono text-sm">You have an active game session</span>
          <Link href={`/game/${activeGameId}`}>
            <Button size="sm" className="bg-black text-yellow-500 hover:bg-zinc-800 gap-2">
              REJOIN GAME <ArrowRight className="w-4 h-4" />
            </Button>
          </Link>
          <Button 
            size="sm" 
            variant="ghost" 
            className="text-black hover:bg-yellow-600"
            onClick={handleAbandonGame}
          >
            ABANDON
          </Button>
        </div>
      )}

      {/* Background with FaultyTerminal */}
      <div className="absolute inset-0 z-0">
        <MemoizedFaultyTerminal
          scale={1.5}
          gridMul={GRID_MUL}
          digitSize={1.2}
          timeScale={0.5}
          scanlineIntensity={0.5}
          noiseAmp={1}
          curvature={0.1}
          tint="#3b82f6"
          mouseReact={true}
          mouseStrength={0.5}
          brightness={0.6}
        />
        {/* Dark overlay for readability */}
        <div className="absolute inset-0 bg-black/60 z-10 pointer-events-none" />
      </div>

      <main className="relative z-20 flex min-h-screen flex-col items-center justify-center p-6 sm:p-24 pointer-events-none">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, ease: "easeOut" }}
          className="container max-w-5xl flex flex-col items-center text-center space-y-12"
        >
          {/* Hero Section */}
          <div className="space-y-6">
            <h1
              className="text-5xl sm:text-6xl md:text-8xl lg:text-9xl leading-tight text-white"
              style={{ fontFamily: '"Press Start 2P", system-ui' }}
            >
              BATTLE CP
            </h1>
            <p
              className="max-w-[800px] text-zinc-400 text-[10px] md:text-sm tracking-widest uppercase leading-loose"
              style={{ fontFamily: '"Press Start 2P", system-ui' }}
            >
              Competitive programming <span className="text-primary mx-2">X</span> Battleship
            </p>
          </div>

          {/* Action Buttons */}
          <div className="flex flex-col gap-4 min-[400px]:flex-row pt-4 pointer-events-auto">
            <Link href="/lobby/create" onClick={() => playSuccess()}>
              <Button
                size="lg"
                className="h-16 px-8 text-base gap-3 bg-emerald-600 hover:bg-emerald-500 border-none tracking-wider"
                style={{ fontFamily: '"Press Start 2P", system-ui' }}
              >
                <Terminal className="w-5 h-5" />
                START
              </Button>
            </Link>
            <Link href="/lobby/join" onClick={() => playJoin()}>
              <Button
                size="lg"
                variant="outline"
                className="h-16 px-8 text-base bg-black/50 border-white/20 hover:bg-white/10 gap-3 backdrop-blur-sm tracking-wider"
                style={{ fontFamily: '"Press Start 2P", system-ui' }}
              >
                <Users className="w-5 h-5" />
                JOIN
              </Button>
            </Link>
          </div>
        </motion.div>
      </main>

      {/* Footer */}
      <footer
        className="absolute bottom-6 w-full text-center text-zinc-600 text-[10px] z-20 opacity-50 pointer-events-none"
        style={{ fontFamily: '"Press Start 2P", system-ui' }}
      >
        SYSTEM READY // BATTLE_CP_V1.0
      </footer>
    </div>
  );
}

