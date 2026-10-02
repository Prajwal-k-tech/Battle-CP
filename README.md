# BattleCP

A multiplayer game that combines **Battleship with competitive programming**. Players fire at an opponent's fleet; overheating locks their weapons until they solve an assigned Codeforces problem.

**[Play BattleCP](https://battle-cp.tech/)** · **[Game rules](rules.md)** · **[Architecture](ARCHITECTURE.md)** · **[Codeforces launch discussion](https://codeforces.com/blog/entry/152124)**

<img width="1920" height="955" alt="BattleCP multiplayer game interface" src="https://github.com/user-attachments/assets/4f4c59cb-ec5a-4335-b8ce-5f442fa0422c" />

## How it works

1. Create or join a room using a Codeforces handle.
2. Place your fleet and choose the match settings.
3. Fire at the opponent's grid. Shots accumulate heat.
4. Solve the assigned problem to unlock overheated weapons and continue the match.

Difficulty modes, veto penalties and sudden-death rules are explained in [rules.md](rules.md).

## Engineering

- **Rust, Axum and Tokio backend:** HTTP endpoints and WebSocket connections coordinate game state.
- **Server-side rules:** fleet placement, shot validity, game phases and submission checks are handled by the backend.
- **Codeforces integration:** retrieves problem/submission information; API requests are throttled and cached.
- **Next.js and TypeScript frontend:** lobby creation, fleet placement and live match updates.
- **Reconnect support:** restores a player's view while keeping the opponent's unhit ships hidden.

These are implementation features, not throughput benchmarks or a guarantee against every form of cheating. Match state is kept in memory; service restarts do not imply durable match recovery.

## Status

BattleCP is publicly deployed, and the [community launch thread](https://codeforces.com/blog/entry/152124) includes player feedback. This README does not claim usage or performance metrics.

The live service may not yet run the latest source revision. Check the deployment itself for its current behavior.

## Run locally

### Requirements

- A current stable Rust toolchain.
- Node.js 20.9 or later and npm.
- A Codeforces account and network access to its public API.

### Clone

```bash
git clone https://github.com/Prajwal-k-tech/Battle-CP.git
cd Battle-CP
```

### Backend configuration

Create `backend/.env` for local development:

```dotenv
PORT=4000
RUST_LOG=info
ALLOWED_ORIGINS=http://localhost:3000,http://127.0.0.1:3000
DISCORD_WEBHOOK_URL=
```

The root `.env.example` contains deployment-oriented defaults, so do not copy it unchanged for local development. Leave the optional Discord webhook unset or empty when you do not need match logging. Keep real webhook credentials out of version control.

### Frontend configuration

Create frontend/.env.local:

```dotenv
NEXT_PUBLIC_API_URL=http://localhost:4000
NEXT_PUBLIC_WS_URL=ws://localhost:4000
NEXT_PUBLIC_APP_URL=http://localhost:3000
```

Explicit local URLs are needed because the frontend otherwise defaults to the production API.

### Start both services

Backend terminal:

```bash
cd backend
cargo run
```

Frontend terminal, from the repository root:

```bash
cd frontend
npm ci
npm run dev
```

Open **http://localhost:3000**. The backend runs separately on port **4000**. If you change the frontend port, update the allowed origins and app URL to match.

## Repository guide

| Path | Purpose |
|---|---|
| backend/src/game.rs, state.rs | Game rules and state |
| backend/src/ws.rs, protocol.rs | WebSocket handling and messages |
| backend/src/cf_client.rs | Codeforces client, problem selection and API coordination |
| backend/src/background.rs | Match timers and cleanup |
| frontend/ | Next.js game interface |
| rules.md | Player-facing mechanics |
| ARCHITECTURE.md | Implementation walkthrough |

## Acknowledgments

Problem statements and submission data come from Codeforces. BattleCP is an independent project and is not affiliated with Codeforces.
