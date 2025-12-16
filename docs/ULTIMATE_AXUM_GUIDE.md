# 🚀 The Ultimate Guide to Axum (v0.7)

Axum is an ergonomic and modular web framework built with Tokio, Tower, and Hyper. It's designed to be easy to use while retaining maximum flexibility and performance.

---

## 📚 1. Core Concepts

### 1.1 Extractors
Extractors are how you pick apart the incoming request to get the parts your handler needs. They are arguments to your handler functions.

**Common Extractors:**
*   **`Json<T>`**: Deserializes the request body as JSON.
*   **`Path<T>`**: Extracts parameters from the URL path (e.g., `/users/:id`).
*   **`Query<T>`**: Extracts query parameters (e.g., `?foo=bar`).
*   **`State<T>`**: Extracts shared application state.
*   **`Extension<T>`**: Extracts data inserted by middleware.
*   **`WebSocketUpgrade`**: Upgrades the connection to a WebSocket.

**Example:**
```rust
async fn create_user(
    State(state): State<AppState>,       // Extract shared state
    Path(team_id): Path<Uuid>,           // Extract team_id from URL
    Json(payload): Json<CreateUserRequest> // Extract body as JSON
) -> impl IntoResponse {
    // ...
}
```

### 1.2 Application State (`Arc` + `RwLock`) -- **CRITICAL**
Axum uses `State` to share data across handlers. Since Axum handlers run concurrently (multi-threaded), state must be **thread-safe**.

*   **`Arc<T>`** (Atomic Reference Count): Allows multiple parts of the app to "own" the same data. It's like a thread-safe smart pointer.
*   **`RwLock<T>`** (Read-Write Lock): Allows multiple readers **OR** one writer at a time.
*   **Pattern**: `Arc<RwLock<AppState>>`
    *   `state.read().await` -> Get read-only access (fast, parallel).
    *   `state.write().await` -> Get exclusive write access (blocks others).

### 1.3 Routing
Axum routing is explicit and modular.
```rust
let app = Router::new()
    .route("/", get(root_handler))
    .route("/users/:id", get(get_user).post(update_user))
    .with_state(state); // Attach state here
```

---

## 🔒 2. CORS (Cross-Origin Resource Sharing)

CORS is a browser security feature that restricts web pages from making requests to a different domain than the one that served them.
*   **Frontend**: `localhost:3000`
*   **Backend**: `localhost:8080`
*   **Result**: Frontend blocked from calling Backend *unless* Backend allows it.

**Is `allow_origin(Any)` safe?**
*   **Development**: YES. It's convenient for local dev.
*   **Production**: NO. It allows *any* website to call your API.
    *   **Fix**: Explicitly whitelist your frontend domain.

**Axum Implementation (using `tower-http`):**
```rust
use tower_http::cors::{CorsLayer, Any};

let cors = CorsLayer::new()
    // Allow GET/POST requests
    .allow_methods([Method::GET, Method::POST])
    // Allow only requests from your frontend
    // .allow_origin("https://my-game.com".parse::<HeaderValue>().unwrap())
    // For DEV: Allow anything
    .allow_origin(Any) 
    .allow_headers(Any);

let app = Router::new().layer(cors);
```

---

## ⚡ 3. WebSockets

WebSockets allow **bidirectional, real-time** communication. Unlike HTTP (Request -> Response), WebSockets keep the connection open.

**The Handshake:**
1.  Client sends HTTP request with `Upgrade: websocket`.
2.  Server (Axum) accepts via `WebSocketUpgrade` extractor.
3.  Server upgrades connection and hands it off to a dedicated handler loop.

**Axum Workflow (`ws.rs`):**
1.  **Route**: `/ws/:game_id`
2.  **Handler**: Accepts `WebSocketUpgrade`.
3.  **Upgrade**: Calls `ws.on_upgrade(...)`.
4.  **Loop**: Spawns a new async task that loops forever (until disconnect), waiting for messages.

```rust
// ws.rs
async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
           // Handle message (Text, Binary, Ping, Close)
        } else {
           // Client disconnected
           break; 
        }
    }
}
```

---

## 🏗️ 4. Project File Walkthrough

### `main.rs` (The Entry Point)
*   Initializes the `tokio` runtime (async engine).
*   Sets up the `Router` with routes (`/`, `/api/game`, `/ws/:id`).
*   Adds Middleware (Layers):
    *   `TraceLayer`: Logging (so you see requests in terminal).
    *   `CorsLayer`: Allows frontend to talk to backend.
*   Binds the server to `127.0.0.1:3000`.

### `state.rs` (The Database - In Memory)
*   Defines `AppState`: The "Global Variable" for the server.
*   Defines `Game`, `Player`, `Grid`: The data models.
*   Uses `Arc<RwLock<HashMap<Uuid, Game>>>`:
    *   This is our "Database".
    *   It stores all active games in memory.
    *   Thread-safe so multiple games can update simultaneously.

### `game.rs` (The Logic)
*   Contains the **methods** for our structs.
*   `Game::new()`: Creates a game.
*   `Player::fire()`: The core mechanic. Checks ammo, heat, target, updates grid.
*   **Pure Logic**: This file doesn't know about HTTP or WebSockets. It just takes data and returns results. This makes it easy to test!

### `handlers.rs` (The HTTP Interface)
*   `create_game`:
    1.  Generates a new Game ID.
    2.  Locks `state.games` for writing.
    3.  Inserts the new game.
    4.  Returns the ID as JSON.
*   Think of this as the "Controller" in MVC.

### `ws.rs` (The Real-Time Interface)
*   `ws_handler`: The doorman. Checks handshake, upgrades connection.
*   `handle_socket`: The conversation.
    *   It's a long-running function (loop).
    *   It will need to:
        1.  Parse incoming JSON (e.g., `{"action": "FIRE", "x": 1, "y": 2}`).
        2.  Lock state to get the specific Game.
        3.  Call `player.fire()`.
        4.  Send back the result (`{"result": "HIT"}`).

---

## 🔥 Pro Tips for Learning
1.  **Read the Compiler Errors**: Rust's compiler is your teacher. It tells you exactly why something isn't thread-safe.
2.  **Use `cargo check`**: Runs faster than `cargo build` and checks for errors.
3.  **Think in "Ownership"**: Who owns this data? Only one owner? Use `Box` or value. Multiple owners? Use `Arc`. Shared mutable owners? `Arc<RwLock>`.
