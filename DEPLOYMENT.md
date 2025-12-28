# Battle CP - Deployment Guide

This guide covers deploying Battle CP to production environments.

---

## Prerequisites

- **Backend**: Rust 1.75+
- **Frontend**: Node.js 18+
- **Domain**: With SSL certificate (required for WSS)

---

## Option 1: Docker Deployment (Recommended)

### Backend Dockerfile

```dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
COPY backend/ .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
EXPOSE 3000
CMD ["backend"]
```

### Frontend Dockerfile

```dockerfile
FROM node:18-alpine AS builder
WORKDIR /app
COPY frontend/ .
RUN npm install && npm run build

FROM node:18-alpine
WORKDIR /app
COPY --from=builder /app/.next ./.next
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/package.json ./
EXPOSE 3000
CMD ["npm", "start"]
```

### docker-compose.yml

```yaml
version: '3.8'
services:
  backend:
    build:
      context: .
      dockerfile: Dockerfile.backend
    ports:
      - "3001:3000"
    environment:
      - PORT=3000
      - ALLOWED_ORIGINS=https://yourdomain.com
    restart: unless-stopped

  frontend:
    build:
      context: .
      dockerfile: Dockerfile.frontend
    ports:
      - "3000:3000"
    environment:
      - NEXT_PUBLIC_API_URL=https://api.yourdomain.com
      - NEXT_PUBLIC_WS_URL=wss://api.yourdomain.com
    depends_on:
      - backend
    restart: unless-stopped
```

---

## Option 2: Cloud Platform Deployment

### Backend on Render/Railway/Fly.io

1. **Create new Web Service**
2. **Connect GitHub repo**
3. **Build command**: `cd backend && cargo build --release`
4. **Start command**: `./target/release/backend`
5. **Environment variables**:
   ```
   PORT=10000
   ALLOWED_ORIGINS=https://your-frontend.vercel.app
   ```

### Frontend on Vercel

1. **Import GitHub repo**
2. **Root directory**: `frontend`
3. **Build command**: `npm run build`
4. **Environment variables**:
   ```
   NEXT_PUBLIC_API_URL=https://your-backend.fly.dev
   NEXT_PUBLIC_WS_URL=wss://your-backend.fly.dev
   ```

---

## Option 3: VPS Deployment (DigitalOcean/Linode)

### 1. Server Setup

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install dependencies
sudo apt install -y build-essential pkg-config libssl-dev nginx certbot python3-certbot-nginx

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Node.js
curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
sudo apt install -y nodejs
```

### 2. Clone and Build

```bash
git clone https://github.com/yourusername/BattleCP.git
cd BattleCP

# Build backend
cd backend
cargo build --release

# Build frontend
cd ../frontend
npm install
npm run build
```

### 3. Create systemd Services

**Backend service** (`/etc/systemd/system/battlecp-backend.service`):

```ini
[Unit]
Description=Battle CP Backend
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/home/deploy/BattleCP/backend
ExecStart=/home/deploy/BattleCP/backend/target/release/backend
Environment=PORT=3001
Environment=ALLOWED_ORIGINS=https://yourdomain.com
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**Frontend service** (`/etc/systemd/system/battlecp-frontend.service`):

```ini
[Unit]
Description=Battle CP Frontend
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/home/deploy/BattleCP/frontend
ExecStart=/usr/bin/npm start
Environment=PORT=3000
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable battlecp-backend battlecp-frontend
sudo systemctl start battlecp-backend battlecp-frontend
```

### 4. Nginx Configuration

```nginx
# /etc/nginx/sites-available/battlecp

# Frontend
server {
    listen 80;
    server_name yourdomain.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    location / {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
    }
}

# Backend API
server {
    listen 80;
    server_name api.yourdomain.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name api.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/api.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.yourdomain.com/privkey.pem;

    location / {
        proxy_pass http://localhost:3001;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_cache_bypass $http_upgrade;
        
        # WebSocket specific
        proxy_read_timeout 86400;
    }
}
```

```bash
sudo ln -s /etc/nginx/sites-available/battlecp /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

### 5. SSL with Let's Encrypt

```bash
sudo certbot --nginx -d yourdomain.com -d api.yourdomain.com
```

---

## Environment Variables Reference

### Backend

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `PORT` | No | 3000 | Server port |
| `ALLOWED_ORIGINS` | Yes | localhost | Comma-separated CORS origins |

### Frontend

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `NEXT_PUBLIC_API_URL` | Yes | http://localhost:3000 | Backend API URL |
| `NEXT_PUBLIC_WS_URL` | Yes | ws://localhost:3000 | WebSocket URL |

---

## Health Checks

### Backend

```bash
curl https://api.yourdomain.com/
# Expected: "Battle CP Backend Online"
```

### WebSocket

```bash
# Using websocat
websocat wss://api.yourdomain.com/ws/00000000-0000-0000-0000-000000000000
# Expected: {"type":"Error","message":"Game not found"}
```

---

## Monitoring

### Logs

```bash
# Backend logs
journalctl -u battlecp-backend -f

# Frontend logs  
journalctl -u battlecp-frontend -f
```

### Metrics to Watch

- WebSocket connection count
- Game creation rate
- CF API latency
- Memory usage (games are in-memory)

---

## Troubleshooting

### WebSocket Connection Fails

1. Check CORS origins include your domain
2. Verify SSL is properly configured for WSS
3. Check nginx WebSocket proxy settings

### High Memory Usage

Games are stored in-memory. With default cleanup:
- Finished games removed after 5 minutes
- Waiting/PlacingShips games removed after 30 minutes

For high-traffic deployment, consider:
- Reducing cleanup thresholds
- Adding max game limits
- Using Redis for state persistence

### CF API Errors

- Check if handle is valid on Codeforces directly
- API has rate limits - verify timeout is working (15s)
- Check network connectivity to codeforces.com
