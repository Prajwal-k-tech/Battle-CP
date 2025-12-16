# Battle CP

**Real-time multiplayer naval strategy meets algorithmic problem solving.**

Battle CP fuses classic Battleship mechanics with competitive programming. Players deploy fleets, fire artillery, and must solve real Codeforces problems to unlock their systems when they overheat or to veto incoming damage.

![Lobby Interface](https://via.placeholder.com/800x400?text=Battle+CP+Screenshot)

## Core Features

- **Hybrid Warfare:** Strategy deciding positioning + Speed deciding algorithm solving.
- **Real-Time Sync:** 1Hz state synchronization via Rust WebSockets.
- **Competitive Integrity:** Integration with Codeforces API for live problem verification.
- **Custom Game Modes:** Supports 1-minute blitz rounds to 45-minute marathons.
- **Reactive UI:** Built with Next.js and Tailwind for immediate visual feedback.

## Tech Stack

- **Backend:** Rugged Rust server using `Axum` and `Tokio` for high-concurrency WebSocket handling.
- **Frontend:** Next.js 14 (TypeScript), Tailwind CSS, Framer Motion.
- **State:** In-memory `RwLock` protected game states for <5ms latency.

## Getting Started

### Prerequisites
- Node.js 18+
- Rust 1.75+
- A [Codeforces](https://codeforces.com) account.

### Development

1. **Frontend:**
   ```bash
   cd frontend
   npm install
   npm run dev
   ```

2. **Backend:**
   ```bash
   cd backend
   cargo run
   ```

3. Open `http://localhost:3000` (or configured port).

## Documentation

- [**Game Rules**](rules.md): Detailed mechanics on Heat, Vetoes, and Tiebreakers.
