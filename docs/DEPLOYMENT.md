# Battle CP - Deployment Guide

## Production Hosting

### Frontend (Next.js)
Deploy to Vercel, Netlify, or any static hosting.

**Environment Variables:**
```
NEXT_PUBLIC_API_URL=https://your-backend.domain.com
NEXT_PUBLIC_WS_URL=wss://your-backend.domain.com
```

Build: `npm run build`

---

### Backend (Rust/Axum)
Deploy to Railway, Fly.io, or any VPS.

The backend binds to `0.0.0.0:3000`, so it accepts connections on all interfaces.

**Optional environment variables:**
- `PORT` - Override default port 3000
- `RUST_LOG` - Logging level (info, debug, trace)

Build: `cargo build --release`
Run: `./target/release/backend`

---

## CORS
Currently allows:
- `http://localhost:3000`
- `http://localhost:3001`
- `http://127.0.0.1:3000`
- `http://127.0.0.1:3001`

For production, update `backend/src/main.rs` to include your domain:
```rust
.allow_origin([
    "https://your-frontend.domain.com".parse::<HeaderValue>()?,
    // ... other origins
])
```

---

## WebSocket
Uses standard WebSocket upgrade on `/ws/:game_id?player_id=...`
Ensure your hosting provider supports WebSocket connections.
