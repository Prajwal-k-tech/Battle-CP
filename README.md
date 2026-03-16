# Battle CP

**Competitive Programming meets Battleship** — Real-time multiplayer game combining Codeforces problems with classic naval combat.

<img width="1920" height="955" alt="image" src="https://github.com/user-attachments/assets/4f4c59cb-ec5a-4335-b8ce-5f442fa0422c" />

## Prerequisites

Before running locally, ensure you have:
- **Node.js** 18+ ([download](https://nodejs.org/))
- **Rust** 1.75+ ([install rustup](https://rustup.rs/))
- **A Codeforces account** ([create one](https://codeforces.com/))

## Environment Setup

### 1. Clone the Repository

```bash
git clone https://github.com/yourusername/Battle-CP.git
cd Battle-CP
```

### 2. Set Up Environment Variables

Copy the example environment files and fill in your values:

```bash
# Root directory (for backend config)
cp .env.example .env

# Frontend directory (optional, for local dev)
cd frontend
cp .env.example .env.local
cd ..
```

Edit `.env` in the root directory:
```bash
# Required for Discord match logging (optional for local dev)
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN

# Backend configuration
RUST_LOG=info
PORT=3000
ALLOWED_ORIGINS=https://battle-cp.vercel.app
```

**Note:** For local development without webhook logging, you can leave `DISCORD_WEBHOOK_URL` empty or skip it entirely.

## Running Locally

### Backend (Rust)

```bash
cd backend
cargo run
```

The backend will start on `http://localhost:3000` by default.

### Frontend (Next.js)

In a new terminal:

```bash
cd frontend
npm install
npm run dev
```

The frontend will start on `http://localhost:3000` (or fallback to 3001/3002 if 3000 is occupied).

### Access the Game

Open your browser to: **`http://localhost:3000`**

## Documentation

- [**Game Rules**](rules.md) — Detailed rules, difficulty modes, and mechanics
- [**Architecture**](ARCHITECTURE.md) — System design and technical overview
