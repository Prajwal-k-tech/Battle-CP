"use client";

import { MemoizedFaultyTerminal } from "@/components/ui/FaultyTerminal";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { Terminal, Users, ArrowRight, ScrollText } from "lucide-react";
import Link from "next/link";
import { motion } from "framer-motion";
import { useSound } from "@/context/SoundContext";
import { useMusic } from "@/context/MusicContext";
import { useEffect, useState } from "react";
import Script from "next/script";

// Stable constant for FaultyTerminal to prevent re-renders
const GRID_MUL: [number, number] = [2, 1];

export default function Home() {
  const { playJoin, playSuccess, playShipPlace } = useSound();
  const { setPhase } = useMusic();
  const [activeGameId, setActiveGameId] = useState<string | null>(null);

  // JSON-LD Structured Data
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "VideoGame",
    "name": "Battle CP",
    "description": "A competitive programming strategy game combining Battleship mechanics with Codeforces problems.",
    "genre": ["Strategy", "Educational", "Puzzle"],
    "gamePlatform": "Web Browser",
    "author": {
      "@type": "Person",
      "name": "oGhostyyy",
      "url": "https://linktr.ee/oGhostyyy"
    },
    "applicationCategory": "Game",
    "operatingSystem": "Any"
  };

  // Breadcrumb Schema for SEO
  const breadcrumbSchema = {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    "itemListElement": [
      {
        "@type": "ListItem",
        "position": 1,
        "name": "Home",
        "item": process.env.NEXT_PUBLIC_APP_URL || "https://battle-cp.vercel.app"
      },
      {
        "@type": "ListItem",
        "position": 2,
        "name": "Create Game",
        "item": `${process.env.NEXT_PUBLIC_APP_URL || "https://battle-cp.vercel.app"}/lobby/create`
      },
      {
        "@type": "ListItem",
        "position": 3,
        "name": "Join Game",
        "item": `${process.env.NEXT_PUBLIC_APP_URL || "https://battle-cp.vercel.app"}/lobby/join`
      }
    ]
  };

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
        <Script
          id="structured-data"
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
        />
        <Script
          id="breadcrumb-schema"
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: JSON.stringify(breadcrumbSchema) }}
        />
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
            <Dialog>
              <DialogTrigger asChild>
                <Button
                  size="lg"
                  variant="outline"
                  className="h-16 px-8 text-base bg-black/50 border-white/20 hover:bg-white/10 gap-3 backdrop-blur-sm tracking-wider"
                  style={{ fontFamily: '"Press Start 2P", system-ui' }}
                  onClick={() => playShipPlace()}
                >
                  <ScrollText className="w-5 h-5" />
                  RULES
                </Button>
              </DialogTrigger>
              <DialogContent className="border-primary/30 bg-black/95 text-white max-w-2xl max-h-[85vh] overflow-y-auto backdrop-blur-xl">
                <DialogHeader>
                  <DialogTitle
                    className="text-primary text-xl tracking-widest"
                    style={{ fontFamily: '"Press Start 2P", system-ui' }}
                  >
                    RULES OF ENGAGEMENT
                  </DialogTitle>
                </DialogHeader>
                <div className="space-y-5 text-sm font-mono text-zinc-300 leading-relaxed pr-1">

                  <RulesSection title="1. OBJECTIVE">
                    Destroy the enemy fleet before time runs out. If time expires, the player with the most ships remaining wins.
                  </RulesSection>

                  <RulesSection title="2. DEPLOYMENT PHASE">
                    <ul className="space-y-1 list-disc list-inside text-zinc-400">
                      <li>Place your <span className="text-white">5 ships</span> on the 10×10 grid.</li>
                      <li>Ships cannot overlap. Touching (adjacent) is allowed.</li>
                      <li>Both players must confirm placement to begin combat.</li>
                    </ul>
                  </RulesSection>

                  <RulesSection title="3. COMBAT &amp; HEAT">
                    <ul className="space-y-1 list-disc list-inside text-zinc-400">
                      <li>Click a cell on the enemy grid to fire.</li>
                      <li>Every shot generates <span className="text-orange-400">Heat</span>.</li>
                      <li>Reach the <span className="text-red-400">Heat Threshold</span> (default <span className="text-red-400">9 shots</span>) → weapons <span className="text-red-400">OVERHEAT</span>. You cannot fire while locked.</li>
                    </ul>
                    <div className="mt-2 p-2 border border-white/10 rounded bg-white/5 text-zinc-400">
                      <span className="text-emerald-400">Active cool-down:</span> Solve a Codeforces problem to instantly reset all heat.
                    </div>
                  </RulesSection>

                  <RulesSection title="4. VETO MECHANIC">
                    <ul className="space-y-1 list-disc list-inside text-zinc-400">
                      <li>You have <span className="text-white">3 Vetoes</span> (configurable).</li>
                      <li>Use a Veto to skip the assigned problem at the cost of a timed penalty.</li>
                      <li>When the timer expires, a <span className="text-yellow-400">new problem is assigned</span> — you must solve it to unlock.</li>
                      <li>Penalty durations (default Low): <span className="text-yellow-400">1 min → 2 min → 3 min</span> (escalating).</li>
                      <li>Your opponent can still fire at you during your penalty.</li>
                    </ul>
                  </RulesSection>

                  <RulesSection title="5. TIE-BREAKERS">
                    If the game timer ends:
                    <ol className="space-y-1 list-decimal list-inside text-zinc-400 mt-1">
                      <li><span className="text-white">Primary:</span> Most ships remaining wins.</li>
                      <li><span className="text-white">Secondary:</span> Most cells hit on enemy grid wins.</li>
                      <li><span className="text-white">Final:</span> Sudden Death.</li>
                    </ol>
                  </RulesSection>

                  <RulesSection title="6. SUDDEN DEATH">
                    <ul className="space-y-1 list-disc list-inside text-zinc-400">
                      <li>Entered only when ships <em>and</em> cells-hit are exactly equal at time-up.</li>
                      <li>No state is reset — all locks, heat, and veto timers persist.</li>
                      <li>First player to land a confirmed <span className="text-emerald-400">HIT</span> wins.</li>
                      <li>A Miss does NOT win. Only a Hit ends Sudden Death.</li>
                    </ul>
                  </RulesSection>

                  <RulesSection title="7. DIFFICULTY MODES">
                    <div className="space-y-2 text-zinc-400">
                      <p><span className="text-blue-400">CF Mode:</span> Problems are assigned at an exact Codeforces rating (800 – 3500). Choose your precise target difficulty.</p>
                      <p><span className="text-purple-400">Band Mode:</span> Problems are drawn from a clist.by rating range. Pick a named tier instead of an exact number:</p>
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-1 mt-1 text-xs">
                        {[
                          ["Super Easy", "clist 0–300",     "text-emerald-400"],
                          ["Easy",       "clist 301–600",   "text-green-400"],
                          ["Medium",     "clist 601–1000",  "text-yellow-400"],
                          ["Hard",       "clist 1001–1500", "text-orange-400"],
                          ["Very Hard",  "clist 1501+",     "text-red-400"],
                        ].map(([name, range, color]) => (
                          <div key={name} className="flex justify-between px-2 py-1 bg-white/5 rounded border border-white/5">
                            <span className={color}>{name}</span>
                            <span className="text-zinc-500">{range}</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  </RulesSection>

                </div>
              </DialogContent>
            </Dialog>
          </div>
        </motion.div>
      </main>

      {/* Footer */}
      <footer className="absolute bottom-6 w-full z-20 pointer-events-none">
        <div
          className="mx-auto flex items-center justify-center gap-2 pointer-events-auto text-[10px]"
          style={{ fontFamily: '"Press Start 2P", system-ui' }}
        >
          <span className="text-zinc-300 opacity-80">SYSTEM READY</span>
          <span className="text-zinc-400 opacity-70">//</span>
          <span className="text-zinc-300 opacity-80">BATTLE_CP_V1.0</span>
          <span className="text-zinc-400 opacity-70">//</span>
          <span className="text-zinc-300 opacity-80">Made by</span>
          <a
            href="https://linktr.ee/oGhostyyy"
            target="_blank"
            rel="noopener noreferrer"
            className="text-primary opacity-80 hover:text-white"
          >
            &nbsp;oGhostyyy
          </a>
        </div>
      </footer>
    </div>
  );
}

function RulesSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-2">
      <h3
        className="text-primary text-[10px] tracking-widest border-b border-primary/20 pb-1"
        style={{ fontFamily: '"Press Start 2P", system-ui' }}
      >
        {title}
      </h3>
      <div className="text-zinc-300 text-xs leading-relaxed">{children}</div>
    </div>
  );
}
