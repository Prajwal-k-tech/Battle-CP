# Architecture

This is a high-level guide to the public `main` branch. It describes the main request and game-state flow, not every implementation detail or a security guarantee.

## Request and game flow

1. The Next.js client sends `POST /api/game` to create a room. The Rust backend returns a game ID and player ID.
2. Each player connects to `/ws/{game_id}` and sends a `JoinGame` message. Further WebSocket messages cover ship placement, shots, problem-solving checks and vetoes.
3. The backend applies game actions to shared server-side state and broadcasts events to the connected players. The server selects the problem required to unlock overheated weapons.
4. A background task emits one-second ticks for timer updates, phase timeouts and cleanup. Finished games remain available briefly for reconnect and result delivery, then are removed.
5. The client renders the current phase, boards, timers and problem state. On reconnect, the server sends the player's own ships and a view of the opponent's grid that hides unhit ships.

## Backend

The Rust service uses Axum and Tokio.

| Module | Responsibility |
| --- | --- |
| `backend/src/main.rs` | Loads environment settings, constructs shared state, configures HTTP/WebSocket routes, CORS and response headers, then starts the server. |
| `backend/src/state.rs` | Defines game/player state and stores active games in process memory. |
| `backend/src/game.rs` | Implements board actions, turn/game transitions, scoring and result construction. |
| `backend/src/protocol.rs` | Defines the JSON messages exchanged over WebSockets. |
| `backend/src/ws.rs` | Joins players to rooms, handles real-time messages and sends state updates. |
| `backend/src/handlers.rs` | Creates games and serves contest-problem requests. |
| `backend/src/cf_client.rs` | Retrieves and verifies Codeforces information, using cached data and a rate-limited request queue. |
| `backend/src/background.rs` | Advances timers and removes expired games. |

`POST /api/game` accepts a Codeforces handle and optional game settings. The handle is an identifier supplied by the player; it is not an authentication credential. The Codeforces API is an external dependency, so problem lookup and submission checks can be delayed or unavailable.

## Frontend

The client is a Next.js App Router application under `frontend/`. Lobby pages create or join rooms; the game page combines the placement board, combat grids, HUD and problem panel. `frontend/hooks/useGameSocket.ts` manages the WebSocket session and maps server events into UI state. `frontend/lib/backendUrls.ts` reads `NEXT_PUBLIC_API_URL` and `NEXT_PUBLIC_WS_URL`, falling back to the production API when unset.

## State and operational limits

- Active game state is held in memory. A backend restart loses current rooms and games.
- The deployment and source branch may not always run the same revision; check the live service before assuming a source change is deployed.
- Reconnect support restores state only while the server still has the game in memory.
- The implementation validates game actions, but this guide does not claim resistance to every form of cheating or abuse.

For player-facing mechanics, see [rules.md](rules.md). For setup and commands, see [README.md](README.md).
