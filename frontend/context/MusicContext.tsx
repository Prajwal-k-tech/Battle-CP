"use client";

import React, { createContext, useContext, useRef, useEffect, useCallback, useState } from "react";

type MusicPhase = "menu" | "placement" | "combat" | "sudden_death" | "victory" | "defeat";

interface MusicContextType {
    currentPhase: MusicPhase;
    setPhase: (phase: MusicPhase) => void;
    setVolume: (volume: number) => void;
    volume: number;
}

const MusicContext = createContext<MusicContextType>({
    currentPhase: "menu",
    setPhase: () => { },
    setVolume: () => { },
    volume: 0.25,
});

export const useMusic = () => useContext(MusicContext);

// Main game tracks - will shuffle and loop
const COMBAT_TRACKS = [
    "/sounds/Main game sounds/xX_TF_CNS_CeratopSID_06_Xx [Y5NKVsbxTzk].mp3",
    "/sounds/Main game sounds/xX_TF_CNS_FloatingBits_Mp3_Xx [F3tMmemgJvE].mp3",
    "/sounds/Main game sounds/xX_TF_CNS_FreeDoom_Ver07_24bit_Wav_Xx [GrcWZWZCC8g].mp3",
    "/sounds/Main game sounds/xX_TF_CNS_MegaUltraHeavy_DeleteThiz_Xx [82IksSrJeiU].mp3",
    "/sounds/Main game sounds/xX_TF_CNS_SpaceCube50_mstrd_wav_Xx [bCeLEUX5npU].mp3",
    "/sounds/Main game sounds/xX_TF_CNS_SyntaxCNS_PWM_Stere0_Xx [RnRc5TYAvmE].mp3",
    "/sounds/Main game sounds/xX_TF_CNS_WAREz_FinalVer2b_24bit_Wav_Xx [8VWlVTkGCZk].mp3",
];

const PHASE_TRACKS: Record<Exclude<MusicPhase, "combat">, string> = {
    menu: "/sounds/ambient_theme.mp3",
    placement: "/sounds/placementphase.mp3",
    sudden_death: "/sounds/Sudden_death.mp3",
    victory: "/sounds/winner-game-sound-404167.mp3",
    defeat: "/sounds/game_loser.mp3",
};

function shuffleArray<T>(array: T[]): T[] {
    const shuffled = [...array];
    for (let i = shuffled.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
    }
    return shuffled;
}

export function MusicProvider({ children }: { children: React.ReactNode }) {
    const [currentPhase, setCurrentPhase] = useState<MusicPhase>("menu");
    const [volume, setVolumeState] = useState(0.25); // 25% volume for music
    const audioRef = useRef<HTMLAudioElement | null>(null);
    const combatPlaylistRef = useRef<string[]>([]);
    const combatIndexRef = useRef(0);
    const initialized = useRef(false);

    // Initialize audio element
    useEffect(() => {
        if (typeof window === "undefined" || initialized.current) return;
        initialized.current = true;

        audioRef.current = new Audio();
        audioRef.current.volume = volume;
        audioRef.current.loop = false; // We handle looping manually

        return () => {
            if (audioRef.current) {
                audioRef.current.pause();
                audioRef.current = null;
            }
        };
    }, []);

    // Handle track ended - for combat playlist cycling
    const handleTrackEnded = useCallback(() => {
        if (!audioRef.current) return;

        if (currentPhase === "combat") {
            // Move to next track in playlist
            combatIndexRef.current = (combatIndexRef.current + 1) % combatPlaylistRef.current.length;
            audioRef.current.src = combatPlaylistRef.current[combatIndexRef.current];
            audioRef.current.play().catch(() => { }); // Ignore autoplay errors
        } else {
            // Loop single track
            audioRef.current.currentTime = 0;
            audioRef.current.play().catch(() => { });
        }
    }, [currentPhase]);

    // Attach ended listener
    useEffect(() => {
        const audio = audioRef.current;
        if (!audio) return;

        audio.addEventListener("ended", handleTrackEnded);
        return () => audio.removeEventListener("ended", handleTrackEnded);
    }, [handleTrackEnded]);

    // Phase change handler
    const setPhase = useCallback((phase: MusicPhase) => {
        if (phase === currentPhase) return;
        setCurrentPhase(phase);

        if (!audioRef.current) return;

        audioRef.current.pause();

        if (phase === "combat") {
            // Shuffle and start combat playlist
            combatPlaylistRef.current = shuffleArray(COMBAT_TRACKS);
            combatIndexRef.current = 0;
            audioRef.current.src = combatPlaylistRef.current[0];
        } else {
            audioRef.current.src = PHASE_TRACKS[phase];
        }

        audioRef.current.volume = volume;
        audioRef.current.play().catch(() => {
            // Autoplay blocked - will play on user interaction
            console.log("[Music] Autoplay blocked, waiting for user interaction");
        });
    }, [currentPhase, volume]);

    // Volume change handler
    const setVolume = useCallback((newVolume: number) => {
        setVolumeState(newVolume);
        if (audioRef.current) {
            audioRef.current.volume = newVolume;
        }
    }, []);

    // Start menu music on first user interaction
    useEffect(() => {
        if (typeof window === "undefined") return;

        const startMusic = () => {
            if (audioRef.current && audioRef.current.paused && currentPhase === "menu") {
                audioRef.current.src = PHASE_TRACKS.menu;
                audioRef.current.volume = volume;
                audioRef.current.play().catch(() => { });
            }
            window.removeEventListener("click", startMusic);
            window.removeEventListener("keydown", startMusic);
        };

        window.addEventListener("click", startMusic);
        window.addEventListener("keydown", startMusic);

        return () => {
            window.removeEventListener("click", startMusic);
            window.removeEventListener("keydown", startMusic);
        };
    }, [currentPhase, volume]);

    return (
        <MusicContext.Provider value={{ currentPhase, setPhase, setVolume, volume }}>
            {children}
        </MusicContext.Provider>
    );
}
