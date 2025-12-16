# Battle CP - Deployment Guide

## Architecture Overview

```
┌─────────────────┐         ┌──────────────────┐
│   Vercel        │ ◄──────►│  Railway/Render  │
│   (Next.js)     │   WS    │  (Rust Backend)  │
└─────────────────┘         └──────────────────┘
         │                           │
         └───────────┬───────────────┘
                     ▼
           ┌──────────────────┐
           │  Codeforces API  │
           └──────────────────┘
```

---

## Step 1: Deploy Backend (Railway - Recommended)

### Option A: Railway (easiest for WebSockets)

1. **Push to GitHub** (if not already)

2. **Create Railway Account** at [railway.app](https://railway.app)

3. **Deploy from GitHub:**
   - New Project → Deploy from GitHub repo
   - Select `backend/` folder

4. **Add Dockerfile** to `backend/`:
   ```dockerfile
   FROM rust:1.75-alpine AS builder
   WORKDIR /app
   RUN apk add --no-cache musl-dev
   COPY . .
   RUN cargo build --release

   FROM alpine:latest
   WORKDIR /app
   COPY --from=builder /app/target/release/backend ./
   EXPOSE 3000
   CMD ["./backend"]
   ```

5. **Set Environment Variables:**
   ```
   ALLOWED_ORIGINS=https://your-app.vercel.app
   ```

6. **Get your Railway URL** (e.g., `your-backend.up.railway.app`)

### Option B: Render

Similar process - supports WebSockets, free tier available.

### Option C: Fly.io

Best latency, requires CLI setup:
```bash
fly launch
fly secrets set ALLOWED_ORIGINS=https://your-app.vercel.app
fly deploy
```

---

## Step 2: Deploy Frontend (Vercel)

1. **Push to GitHub**

2. **Import to Vercel:**
   - Go to [vercel.com](https://vercel.com)
   - Import Git repository
   - Set root directory to `frontend/`

3. **Configure Environment Variables:**
   | Variable | Value |
   |----------|-------|
   | `NEXT_PUBLIC_WS_URL` | `wss://your-backend.up.railway.app` |
   | `NEXT_PUBLIC_API_URL` | `https://your-backend.up.railway.app` |

4. **Deploy!**

---

## Step 3: Update Backend CORS

After Vercel deploy, update backend ALLOWED_ORIGINS:
```
ALLOWED_ORIGINS=https://your-app.vercel.app
```

> [!IMPORTANT]
> Use `wss://` for WebSocket URL (not `ws://`) in production!

---

## Environment Variables Summary

### Frontend
| Variable | Dev | Production |
|----------|-----|------------|
| `NEXT_PUBLIC_WS_URL` | `ws://localhost:3000` | `wss://backend.railway.app` |
| `NEXT_PUBLIC_API_URL` | `http://localhost:3000` | `https://backend.railway.app` |

### Backend
| Variable | Dev | Production |
|----------|-----|------------|
| `ALLOWED_ORIGINS` | (defaults localhost) | `https://your-vercel-app.vercel.app` |

---

## WebSocket Considerations

> [!WARNING]
> Most free hosting tiers have connection limits. For production:
> - Railway: 100 concurrent connections (free)
> - Render: WebSocket timeout issues on free tier
> - Fly.io: Best option for WebSocket reliability

---

## Custom Domain (Optional)

1. **Vercel:** Add domain in Project Settings → Domains
2. **Backend:** Update ALLOWED_ORIGINS to include custom domain
3. **DNS:** Point domain to Vercel

---

## Troubleshooting

### WebSocket Connection Fails
- Check CORS ALLOWED_ORIGINS includes frontend URL
- Ensure `wss://` not `ws://`
- Check backend logs for connection errors

### Security Headers (Added automatically)
The backend now includes industry-standard security headers:
- `Strict-Transport-Security` (HSTS)
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `X-XSS-Protection: 1; mode=block`

### CORS Errors
- Verify ALLOWED_ORIGINS env var is set correctly
- Include both `https://` and any subdomains
