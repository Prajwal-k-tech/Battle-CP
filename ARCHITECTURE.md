# Battle CP - Complete Technical Architecture

This document provides a comprehensive line-by-line explanation of the Battle CP codebase. Every file is documented with its purpose, key functions, and how it integrates with the rest of the system.

---

## Project Structure

```
BattleCP/
├── backend/           # Rust WebSocket server
│   ├── src/
│   │   ├── main.rs       # Server entry point + routing
│   │   ├── lib.rs        # Module declarations
│   │   ├── state.rs      # Data structures (Game, Player, Grid)
│   │   ├── protocol.rs   # Client/Server message types
│   │   ├── game.rs       # Game logic (fire, place_ship, winner)
│   │   ├── ws.rs         # WebSocket message handlers
│   │   ├── handlers.rs   # HTTP endpoints (create game)
│   │   ├── cf_client.rs  # Codeforces API integration
│   │   └── background.rs # Global ticker (timers, cleanup)
│   ├── tests/            # Integration tests
│   └── Cargo.toml        # Dependencies
├── frontend/          # Next.js React application
│   ├── app/              # Pages
│   ├── components/       # UI components
│   ├── hooks/            # Custom React hooks
│   ├── types/            # TypeScript definitions
│   └── context/          # React contexts
├── README.md          # Project overview
├── rules.md           # Game rules
└── .env.example       # Environment variables
```

---

## Backend Architecture (Rust)

### main.rs - Server Entry Point

**Purpose**: Initializes the HTTP server with WebSocket support.

```rust
// Key Components:
1. tracing_subscriber - Logging initialization
2. AppState - Shared state with Arc<RwLock<HashMap<Uuid, Game>>>
3. tokio::spawn(start_global_ticker) - Background timer task
4. CORS configuration from ALLOWED_ORIGINS env var
5. Security headers (X-Content-Type-Options, X-Frame-Options, HSTS)
```

**Routes**:
- `GET /` - Health check
- `POST /api/game` - Create new game (returns game_id, player_id)
- `GET /api/contest/:contest_id` - Get contest problems
- `GET /ws/:game_id` - WebSocket upgrade

---

### state.rs - Data Structures

**Purpose**: Defines all game state types.

**Key Types**:

| Type | Purpose |
|------|---------|
| `AppState` | Holds all games and CF client |
| `Game` | Single game instance |
| `Player` | Player state (grid, ships, heat) |
| `Grid` | 10x10 cell array |
| `Ship` | Ship position, size, hits |
| `GameStatus` | Waiting/PlacingShips/Playing/SuddenDeath/Finished |
| `GameConfig` | Difficulty, heat threshold, vetoes, duration |

**Thread Safety**: Uses `Arc<RwLock<T>>` for concurrent access.

---

### protocol.rs - Message Types

**Purpose**: Defines all WebSocket message structures.

**Client → Server**:
| Message | Fields |
|---------|--------|
| JoinGame | player_id, cf_handle |
| PlaceShips | ships[] |
| Fire | x, y |
| SolveCP | contest_id, problem_index |
| Veto | (none) |

**Server → Client**:
| Message | Purpose |
|---------|---------|
| GameJoined | Confirm connection |
| PlayerJoined | Opponent connected |
| ShipsConfirmed | Placement acknowledged |
| GameStart | Combat begins |
| GameUpdate | Periodic state sync |
| ShotResult | Hit/miss result |
| WeaponsLocked | Player overheated |
| WeaponsUnlocked | Solved/veto expired |
| GameOver | Game ended |
| Error | Error message |
| YourShips | Reconnection: restore ships |
| GridSync | Reconnection: restore grids |

---

### game.rs - Game Logic

**Purpose**: Core game mechanics.

**Key Functions**:

```rust
Game::new(player_id, handle, config)
// Creates game with P1, status = Waiting

Game::join(player_id, handle)
// Adds P2, validates not same player

Game::determine_winner()
// Tiebreaker: ships remaining > cells hit > sudden death

Player::fire(opponent, x, y, heat_threshold, veto_penalties)
// 1. Check if locked (veto timer check)
// 2. Process shot on opponent grid
// 3. Update stats (cells_hit/missed)
// 4. Check ship sinking
// 5. Increment heat
// 6. Lock if heat >= threshold

Player::place_ship(ship, x, y, vertical)
// 1. Validate start position (x < 10, y < 10)
// 2. Validate end position doesn't exceed 10
// 3. Check for overlap with existing ships
// 4. Place on grid

Grid::receive_shot(x, y)
// Returns "Hit", "Miss", or "Already fired here"
// Bounds checked: x >= 10 || y >= 10 returns "Out of bounds"
```

---

### ws.rs - WebSocket Handlers

**Purpose**: Processes all real-time messages.

**Flow**:
1. `ws_handler` - Upgrades HTTP to WebSocket
2. `handle_socket` - Main connection loop
   - Subscribes to game broadcast channel
   - Handles client messages via `handle_client_message`
   - Handles broadcast events (Tick, Message)

**Message Handlers**:

| Handler | Security Checks |
|---------|-----------------|
| JoinGame | Validates CF handle, prevents self-play |
| PlaceShips | Blocks after game starts, validates fleet composition |
| Fire | Checks game status, validates player in game |
| SolveCP | Rate limits (10s), blocks during veto |
| Veto | Must be locked, has vetoes remaining |

---

### handlers.rs - HTTP Endpoints

**Purpose**: REST API for game creation.

**create_game**:
1. Validates CF handle exists (fail-closed)
2. Parses config with clamped values:
   - difficulty: 800-3500
   - heat_threshold: 3-20
   - duration: 5-120 minutes
3. Creates Game and inserts into state

---

### cf_client.rs - Codeforces API

**Purpose**: Verifies submissions and users.

**Functions**:
| Function | Purpose |
|----------|---------|
| `verify_user_exists` | Checks handle validity (10 min cache) |
| `verify_submission` | Checks for accepted solution (last 10 submissions) |
| `fetch_contest_problems` | Gets problem list (5 min cache) |

**Safety**: 15 second HTTP timeout prevents hanging.

---

### background.rs - Global Ticker

**Purpose**: Handles timed events.

**Every 1 second**:
1. Broadcast `Tick` to all games
2. Check veto timer expiry (P1 and P2)
3. Check game timeout → determine winner or sudden death

**Game Cleanup**:
- Finished games: 5 minutes
- Waiting games: 30 minutes
- PlacingShips games: 30 minutes

---

## Frontend Architecture (Next.js/React)

### hooks/useGameSocket.ts

**Purpose**: WebSocket connection and state management.

**State**:
- `gameState` - Full game state
- `isConnected` - Connection status
- `gameNotFound` - Prevents reconnection loops

**Reconnection**:
- Exponential backoff (max 10s)
- Max 5 attempts
- Stops on "Game not found" error

**Actions**:
- `fire(x, y)` - Fires at coordinates
- `placeShips(ships)` - Submits ship placement
- `solveCP(contestId, index)` - Submits solution
- `veto()` - Uses veto

---

### types/game.ts

**Purpose**: TypeScript type definitions.

Mirrors backend protocol.rs exactly for type safety.

---

### components/game/

| Component | Purpose |
|-----------|---------|
| PlacementBoard | Drag-and-drop ship placement |
| CombatGrid | Dual grid display during combat |
| HUD | Heat bar, timer, stats |
| ProblemPanel | CF problem display when locked |
| VictoryModal | End game screen |

---

## Deployment

### Current: Vercel + GCP Cloud VM

**Frontend: Vercel**
- Domain: `battle-cp.tech` (primary), `battle-cp.vercel.app` (redirects to .tech)
- Auto-deploys from GitHub main branch
- SSL auto-provisioned by Vercel

**Backend: GCP Compute Engine (always-free e2-micro)**
- Instance: `battlecp-server` (zone: `us-central1-a`)
- IP: `34.121.245.90`
- Domain: `api.battle-cp.tech` (SSL via Let's Encrypt)
- Fallback: `battle-cp.duckdns.org` (full stack, documented in `DNS_MIGRATION.md`)

**Architecture:**
```
battle-cp.tech          → Vercel (Next.js frontend)
api.battle-cp.tech      → GCP VM (Rust backend via nginx → Docker :3000)
battle-cp.vercel.app    → 301 redirect to battle-cp.tech
battle-cp.duckdns.org   → GCP VM full stack fallback (nginx → frontend :3001 + backend :3000)
```

**Deployment Scripts:**
- `deploy_gcp.sh` — Build & deploy backend + frontend to GCP VM
- `deploy_backend.sh` — Legacy Azure Container Apps deployment (unused)
- `deploy_oracle.sh` — Oracle Cloud deployment (unused, kept as reference)
- `oracle_setup.sh` — One-time Oracle VM setup (kept as reference)
- `oracle_anti_idle.sh` — Oracle anti-idle cron (kept as reference)
- `nginx_battlecp.conf` — Oracle nginx config (kept as reference)

**GCP VM Services:**
- Docker: `battlecp-backend` on `127.0.0.1:3000`
- Next.js: `battlecp-frontend.service` on `* :3001`
- Nginx: Reverse proxy (80/443) for both `battle-cp.duckdns.org` and `api.battle-cp.tech`

**Deploy Flow (GCP):**
```bash
# 1. One-time: Set up GCP VM (install Docker, nginx, certbot)
# 2. Deploy
bash deploy_gcp.sh

# 3. SSL (auto-renews)
ssh to VM: sudo certbot --nginx -d api.battle-cp.tech
```

### Environment Variables

```bash
# Backend
PORT=3000
RUST_LOG=info
ALLOWED_ORIGINS=https://battle-cp.tech,https://battle-cp.vercel.app
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...

# Frontend (Vercel env vars)
NEXT_PUBLIC_API_URL=https://api.battle-cp.tech
NEXT_PUBLIC_WS_URL=wss://api.battle-cp.tech
```

### DNS Migration

When the `.tech` domain expires, see `DNS_MIGRATION.md` for rollback instructions to DuckDNS.


---

## Security

- All user IDs are UUIDs (unguessable)
- CORS configured via environment
- Security headers: X-Content-Type-Options, X-Frame-Options, HSTS
