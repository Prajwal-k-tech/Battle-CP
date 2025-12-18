export class SoundSynthesizer {
    public ctx: AudioContext | null = null;
    private masterGain: GainNode | null = null;
    private initialized = false;

    constructor() {
        if (typeof window !== 'undefined') {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            const AudioContextClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
            if (AudioContextClass) {
                this.ctx = new AudioContextClass();
                this.masterGain = this.ctx.createGain();
                this.masterGain.gain.value = 0.8; // Louder SFX to be audible over music
                this.masterGain.connect(this.ctx.destination);
                console.log("[Audio] SoundSynthesizer constructed. Ctx state:", this.ctx.state);
            } else {
                console.error("[Audio] Web Audio API not supported");
            }
        }
    }

    public async init() {
        if (!this.ctx) return;
        console.log("[Audio] init() called. Current state:", this.ctx.state);

        if (this.ctx.state === 'suspended') {
            try {
                await this.ctx.resume();
                console.log("[Audio] AudioContext resumed. State:", this.ctx.state);
            } catch (e) {
                console.error("[Audio] Failed to resume context:", e);
            }
        }
        this.initialized = true;
    }

    // --- COMBAT FLUID SOUNDS ---

    // Retro laser pew
    public playFire() {
        if (!this.ctx || !this.masterGain) {
            console.warn("[Audio] playFire ignored - no context");
            return;
        }
        console.log("[Audio] playFire triggered");
        const t = this.ctx.currentTime;

        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.connect(gain);
        gain.connect(this.masterGain);

        osc.type = 'sawtooth';
        osc.frequency.setValueAtTime(800, t);
        osc.frequency.exponentialRampToValueAtTime(100, t + 0.15);

        gain.gain.setValueAtTime(0.3, t);
        gain.gain.exponentialRampToValueAtTime(0.01, t + 0.15);

        osc.start(t);
        osc.stop(t + 0.15);
    }

    // HEAVY EXPLOSION (noisy crunch with low rumble)
    public playHit() {
        if (!this.ctx || !this.masterGain) return;
        const t = this.ctx.currentTime;

        // Low rumble base
        const bass = this.ctx.createOscillator();
        const bassGain = this.ctx.createGain();
        bass.connect(bassGain);
        bassGain.connect(this.masterGain);
        bass.type = 'sine';
        bass.frequency.setValueAtTime(80, t);
        bass.frequency.exponentialRampToValueAtTime(30, t + 0.4);
        bassGain.gain.setValueAtTime(0.6, t);
        bassGain.gain.exponentialRampToValueAtTime(0.01, t + 0.4);
        bass.start(t);
        bass.stop(t + 0.4);

        // Noisy crackle overlay
        const bufferSize = this.ctx.sampleRate * 0.3;
        const buffer = this.ctx.createBuffer(1, bufferSize, this.ctx.sampleRate);
        const data = buffer.getChannelData(0);
        for (let i = 0; i < bufferSize; i++) {
            data[i] = Math.random() * 2 - 1;
        }
        const noise = this.ctx.createBufferSource();
        noise.buffer = buffer;
        const noiseGain = this.ctx.createGain();
        const filter = this.ctx.createBiquadFilter();
        filter.type = 'highpass';
        filter.frequency.value = 800;
        noise.connect(filter);
        filter.connect(noiseGain);
        noiseGain.connect(this.masterGain);
        noiseGain.gain.setValueAtTime(0.4, t);
        noiseGain.gain.exponentialRampToValueAtTime(0.01, t + 0.25);
        noise.start(t);
    }

    // WATER SPLASH (high pitch descending bloop)  
    public playMiss() {
        if (!this.ctx || !this.masterGain) return;
        const t = this.ctx.currentTime;

        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.connect(gain);
        gain.connect(this.masterGain);

        osc.type = 'sine';
        osc.frequency.setValueAtTime(900, t); // Start high
        osc.frequency.exponentialRampToValueAtTime(200, t + 0.15); // Drop fast

        gain.gain.setValueAtTime(0.25, t);
        gain.gain.linearRampToValueAtTime(0.01, t + 0.15);

        osc.start(t);
        osc.stop(t + 0.15);
    }

    // Deep boom
    public playShipSunk() {
        if (!this.ctx || !this.masterGain) return;
        const t = this.ctx.currentTime;

        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.connect(gain);
        gain.connect(this.masterGain);

        osc.type = 'sine'; // deeply modulated sine
        osc.frequency.setValueAtTime(150, t);
        osc.frequency.exponentialRampToValueAtTime(30, t + 1.0);

        gain.gain.setValueAtTime(0.8, t);
        gain.gain.exponentialRampToValueAtTime(0.01, t + 1.0);

        osc.start(t);
        osc.stop(t + 1.0);

        // Add some noise for texture
        this.playHit();
    }

    // --- UI SOUNDS ---

    // High pitch connect
    public playJoin() {
        if (!this.ctx || !this.masterGain) return;
        const t = this.ctx.currentTime;

        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.connect(gain);
        gain.connect(this.masterGain);

        osc.type = 'sine';
        osc.frequency.setValueAtTime(1200, t);
        osc.frequency.setValueAtTime(1800, t + 0.1);

        gain.gain.setValueAtTime(0.1, t);
        gain.gain.linearRampToValueAtTime(0.01, t + 0.2);

        osc.start(t);
        osc.stop(t + 0.2);
    }

    // Clack
    public playShipPlace() {
        if (!this.ctx || !this.masterGain) return;
        const t = this.ctx.currentTime;

        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.connect(gain);
        gain.connect(this.masterGain);

        osc.type = 'square';
        osc.frequency.setValueAtTime(200, t);

        gain.gain.setValueAtTime(0.1, t);
        gain.gain.exponentialRampToValueAtTime(0.01, t + 0.05);

        osc.start(t);
        osc.stop(t + 0.05);
    }

    // Buzz
    public playInvalid() {
        if (!this.ctx || !this.masterGain) return;
        const t = this.ctx.currentTime;

        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.connect(gain);
        gain.connect(this.masterGain);

        osc.type = 'sawtooth';
        osc.frequency.setValueAtTime(150, t);
        osc.frequency.linearRampToValueAtTime(100, t + 0.2);

        gain.gain.setValueAtTime(0.2, t);
        gain.gain.linearRampToValueAtTime(0.01, t + 0.2);

        osc.start(t);
        osc.stop(t + 0.2);
    }

    // Siren
    public playAlarm() {
        if (!this.ctx || !this.masterGain) return;
        const t = this.ctx.currentTime;

        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.connect(gain);
        gain.connect(this.masterGain);

        osc.type = 'square';
        osc.frequency.setValueAtTime(600, t);
        osc.frequency.linearRampToValueAtTime(800, t + 0.2);

        gain.gain.setValueAtTime(0.1, t);
        gain.gain.linearRampToValueAtTime(0.01, t + 0.4);

        osc.start(t);
        osc.stop(t + 0.4);
    }

    // Success Chime (Major Triad)
    public playSuccess() {
        if (!this.ctx || !this.masterGain) return;
        const t = this.ctx.currentTime;

        const notes = [523.25, 659.25, 783.99]; // C5, E5, G5

        notes.forEach((freq, i) => {
            const osc = this.ctx!.createOscillator();
            const gain = this.ctx!.createGain();

            osc.connect(gain);
            gain.connect(this.masterGain!);

            osc.type = 'triangle';
            osc.frequency.setValueAtTime(freq, t + i * 0.05);

            gain.gain.setValueAtTime(0.1, t + i * 0.05);
            gain.gain.exponentialRampToValueAtTime(0.01, t + i * 0.05 + 0.4);

            osc.start(t + i * 0.05);
            osc.stop(t + i * 0.05 + 0.4);
        });
    }
}

export const sfx = new SoundSynthesizer();
