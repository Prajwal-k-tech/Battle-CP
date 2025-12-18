"use client";

import React, { createContext, useContext, useEffect } from 'react';
import { sfx } from '../utils/soundSynthesizer';

interface SoundContextType {
    playFire: () => void;
    playHit: () => void;
    playMiss: () => void;
    playShipSunk: () => void;
    playJoin: () => void;
    playShipPlace: () => void;
    playInvalid: () => void;
    playAlarm: () => void;
    playSuccess: () => void;
}

const SoundContext = createContext<SoundContextType | null>(null);

export const SoundProvider = ({ children }: { children: React.ReactNode }) => {
    // Initializing on first click to satisfy browser autoplay policies
    useEffect(() => {
        const initAudio = () => {
            console.log("[Audio] Interaction detected, initializing audio...");
            sfx.init();
            window.removeEventListener('click', initAudio);
            window.removeEventListener('keydown', initAudio);
        };

        window.addEventListener('click', initAudio);
        window.addEventListener('keydown', initAudio);

        return () => {
            window.removeEventListener('click', initAudio);
            window.removeEventListener('keydown', initAudio);
        };
    }, []);

    const value = {
        playFire: () => sfx.playFire(),
        playHit: () => sfx.playHit(),
        playMiss: () => sfx.playMiss(),
        playShipSunk: () => sfx.playShipSunk(),
        playJoin: () => sfx.playJoin(),
        playShipPlace: () => sfx.playShipPlace(),
        playInvalid: () => sfx.playInvalid(),
        playAlarm: () => sfx.playAlarm(),
        playSuccess: () => sfx.playSuccess(),
    };

    return (
        <SoundContext.Provider value={value}>
            {children}
        </SoundContext.Provider>
    );
};

export const useSound = () => {
    const context = useContext(SoundContext);
    if (!context) {
        throw new Error('useSound must be used within a SoundProvider');
    }
    return context;
};
