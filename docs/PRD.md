# Battle CP - Product Requirements Document (PRD)

## 1. Executive Summary
Battle CP is a real-time, competitive multiplayer game that blends the strategic naval combat of Battleship with the intellectual rigor of Competitive Programming (CP). Players duel in a 1v1 format where missing shots incurs a "CP Penalty," requiring them to solve algorithmic problems to regain combat effectiveness. The game features a high-fidelity "Dark Futuristic/Cyberpunk" aesthetic.

## 2. Core Game Mechanics (Strict "Battle CP")

### 2.1 The Game Loop
*   **Real-Time Firing**: Simultaneous firing. No turns.
*   **Ammo (Burst Limit)**: Cap = 5 shots. Regen = 1 shot / 3s.
    *   *Clarification*: This limits your **burst** speed. You can't fire 7 shots instantly. You fire 5, then wait for regen to fire the next 2.
*   **Heat (Sustained Limit)**: **EVERY SHOT** adds +1 Heat.
*   **Overheat**: At **7 Heat**, weapons **LOCK**.
    *   *Implication*: You are forced to solve a problem every 7 shots. This creates a rhythm: Burst -> Regen -> Burst -> **SOLVE**.

### 2.2 Unlocking
1. **Solve**: Complete problem → unlock, Heat resets to 0.
2. **Veto**: Skip problem → wait **Veto Penalty**.
*   **Reset**: Unlocking resets Heat to 0. Ammo remains at current value.

### 2.3 Veto Rules (Progressive)
*   **Progression**: 7 min → 12 min → 20 min.
*   **Max Vetoes**: 3.

### 2.4 Victory Conditions
*   **Annihilation**: Sink all enemy ships.
*   **Timeout**: Player with more ships alive wins.

---

### 2.5 Configuration Table (Host Controls)

| Setting | Options | Default | Notes |
|---|---|---|---|
| Difficulty | 700–1200 | **800** | Lower = faster solve |
| Overheat Threshold | 5, 7, 10, 15 | **7** | Shots before forced CP |
| Veto Progression | Standard / Aggressive | **7, 12, 20** | Custom curve |
| Max Game Time | 20–60 min, None | **45 min** | Increased due to forced CP |

### 2.6 Fixed Parameters (MVP)
*   Grid: 10×10 • Ships: 17 cells • Ammo: 1/3s, cap 5
*   **Placement**: Manual Drag-and-Drop (Default) + "Randomize" Button (Optional)

---

### 2.7 Game Length Analysis (Strict Mode)

**Logic**: CP is now a core pacing mechanic.
*   **Total Shots to Win**: ~50.
*   **Forced CP Breaks**: 50 shots / 7 threshold = **~7 problems**.
*   **Time Breakdown**:
    *   **Shooting**: ~5 mins.
    *   **CP Solving**: 7 problems × 5 mins = 35 mins.
    *   **Total**: **~40-45 minutes**.

---

### 2.8 Edge Cases & Safeguards
*   **AFK**: No strict auto-veto. If opponent goes AFK, you win by destroying their stationary ships or by timeout.
*   **Problem repetition**: Never give same problem twice per game.
*   **Handle verification**: None for MVP (Trust system).

## 3. Technical Architecture

### 3.1 Tech Stack
*   **Frontend**: Next.js (React), TailwindCSS, `motion` (Framer Motion), Shadcn/UI.
*   **Backend**: Rust (Axum), Tokio (Async Runtime).
*   **State**: **In-Memory** (Arc<RwLock<GameState>>). No Database for MVP (simplifies deployment).
*   **Communication**: WebSockets (Real-time state sync).
*   **External API**: Codeforces API.

### 3.2 Rust Backend Justification
*   **Concurrency**: Handling multiple 1v1 lobbies with high-frequency state updates (ammo regen, firing, locking) requires robust concurrency. Rust's Actor-like patterns (via Tokio tasks) are perfect for this.
*   **Performance**: Zero-cost abstractions ensure the server can handle many concurrent games with minimal resource footprint compared to Node.js.
*   **Correctness**: Rust's type system prevents data races in the complex real-time state management.

## 4. Design System (Modern Dark / Linear-Style)

### 4.1 Theme "Void & Focus"
*   **Aesthetic**: Clean, professional, and immersive.
*   **Inspirations**:
    *   **Linear.app**: For the grid system, subtle borders, and high-performance feel.
    *   **Reflect.app**: For the glassmorphism effects, noise textures, and fluid micro-interactions.
*   **Background**: Layered dark grays (`#0a0a0a` to `#171717`). No pitch black.
*   **Accents**:
    *   **Primary**: Electric Blue (`#3b82f6`) for active states.
    *   **Action**: Subtle Blue/Violet gradients.
    *   **Danger**: Muted Red/Orange for hits/misses.
*   **Typography**: `Inter` (UI), `JetBrains Mono` (Code), `Space Grotesk` (Headings).
*   **Motion**: `framer-motion` (motion.dev). Fast, non-linear transitions (0.2s ease-out). No slow "floaty" animations.

### 4.2 UI Components
*   **Subtle Borders**: 1px borders with very low opacity (`rgba(255,255,255,0.1)`).
*   **Micro-interactions**: Buttons scale down slightly on click. Hover states are instant.
*   **Glassmorphism**: Used for the "Game HUD" and "Ship Placement" overlays. High blur + Noise texture.



## 5. MVP Scope
*   **Lobby System**: Create Game -> Generate Link -> Friend Joins.
*   **Ship Placement**: Drag and drop ships (Standard fleet: 1x5, 1x4, 2x3, 1x2).
*   **Codeforces Integration**:
    *   Enter Handle.
    *   Fetch random problem by rating (800-1200 for MVP).
    *   Verify submission status via polling `user.status`.

## 6. Implementation Plan

### Phase 1: Project Setup
*   Initialize Monorepo.
*   Setup Rust Axum server with WebSocket route.
*   Setup Next.js with Tailwind + Shadcn.

### Phase 2: Core Game Logic (Rust)
*   Implement `GameSession` struct (Grid state, Ammo, Heat).
*   Implement WebSocket message handlers (`Fire`, `PlaceShips`, `GameStateUpdate`).
*   Implement "Ammo Regen" ticker.

### Phase 3: Frontend & UI
*   Build "Cyberpunk Grid" component.
*   Build "HUD" (Ammo counter, Heat gauge).
*   Integrate WebSocket client.

### Phase 4: CP Integration
*   Implement Codeforces API client in Rust.
*   Build "Problem Modal" in Frontend.
*   Implement "Verify Submission" logic.

### Phase 5: Polish
*   Add sound effects (optional).
*   Refine animations.
