# Battle CP - Deployment Guide

This guide covers deploying Battle CP to production environments.

---

## Prerequisites

- **Backend**: Rust 1.75+
- **Frontend**: Node.js 18+
- **Domain**: With SSL certificate (required for WSS)

---

## 🏆 RECOMMENDED: Vercel (Frontend) + Azure Container Apps (Backend)

## 🏆 RECOMMENDED: Vercel (Frontend) + Azure Container Apps (Backend)

**Best for**: Students with Azure for Students ($100 credits), reliable uptime, automatic HTTPS.

### Why this stack?
- You already have **Vercel** for the frontend (free).
- **Azure Container Apps** easily hosts your `backend/Dockerfile`.
- Due to strict restrictions on Azure for Students accounts (where Entra ID permissions are missing, and cloud builders are disabled), **deploying via GitHub Actions is not possible**. We must build the Docker container locally and push it.

### Prerequisites (Once per computer)
1. Install [Docker Desktop](https://www.docker.com/products/docker-desktop/) and ensure it is running.
2. Install the [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/install-azure-cli).
3. Open a terminal and run `az login` to authenticate with your Azure account.

### Step 1: Initial Azure Setup (Run Once)
Create the Container Registry and App in your Azure account. Run these commands sequentially in your terminal:

```bash
# 1. Create a Resource Group
az group create --name battlecp-rg --location centralindia

# 2. Create the Container Registry
az acr create --resource-group battlecp-rg --name battlecpcr --sku Basic --admin-enabled true

# 3. Create a Container Apps Environment
az containerapp env create --name battlecp-backend-env --resource-group battlecp-rg --location centralindia

# 4. Create the empty Container App
az containerapp create \
  --name battlecp-backend \
  --resource-group battlecp-rg \
  --environment battlecp-backend-env \
  --image mcr.microsoft.com/azuredocs/containerapps-helloworld:latest \
  --target-port 3000 \
  --ingress external
```

### Step 2: Deploy Code to Azure
We have created an automation script to make this a 1-click process for future code updates.

Whenever you change code in the `backend/` directory, just open a terminal in the project root and run:

```bash
./deploy_backend.sh
```

*(This script securely logs into Azure, runs `sudo docker build`, pushes the image to your private registry, and restarts your live API).*

### Step 3: Update Frontend Environment Variables
1. Run `az containerapp show -n battlecp-backend -g battlecp-rg --query properties.configuration.ingress.fqdn -o tsv` to get your live URL.
2. Go back to your [Vercel Dashboard](https://vercel.com) -> Select `Battle-CP` -> **Settings** -> **Environment Variables**.
3. Update the keys to point to the new Azure URL:
   ```env
   NEXT_PUBLIC_API_URL=https://<YOUR_AZURE_FQDN>
   NEXT_PUBLIC_WS_URL=wss://<YOUR_AZURE_FQDN>
   ```
   *(Note: Make sure the WS URL uses `wss://` and HTTP uses `https://`)*
4. Go to **Deployments** and click **Redeploy**.

### Step 4: Test
Open your Vercel URL. Try creating a game. Both sides will securely connect via Azure-powered WebSockets!

**Cost**: $0 out of pocket (Vercel free tier + Azure $100 credits).

---

## Alternative: Vercel (Frontend) + DigitalOcean Droplet (Backend) — Manual Setup

**Best for**: Full control, cheapest at scale, learn DevOps

### Why this stack?
- Same Vercel frontend (free)
- Manual Droplet with systemd + nginx gives you complete control
- Droplet: $6/mo, covered by credits

### Step 1-2: (Same as above) Deploy Vercel frontend, get credits

### Step 3: Create DigitalOcean Droplet

1. Go to [DigitalOcean Cloud Console](https://cloud.digitalocean.com)
2. Click **Create** → **Droplets**
3. Configure:
   - **Region**: Closest to your users
   - **Image**: Ubuntu 24.04 LTS
   - **Size**: Basic → $6/mo (1 GB RAM, 1 vCPU)
   - **Authentication**: SSH Key (recommended)
   - **Hostname**: `battlecp-backend`
4. Click **Create** and note the **IP address**

### Step 4: Bootstrap and Build (on Droplet)

```bash
# SSH into your droplet
ssh root@YOUR_DROPLET_IP

# Update system
apt update && apt upgrade -y

# Install dependencies
apt install -y build-essential pkg-config libssl-dev git curl nginx certbot python3-certbot-nginx

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Clone, build, and install backend
cd ~
git clone https://github.com/YOUR_USERNAME/Battle-CP.git
cd Battle-CP/backend
cargo build --release
# This takes 3-5 minutes first time
```

### Step 5: Create systemd Service

Create `/etc/systemd/system/battlecp-backend.service`:

```bash
cat > /etc/systemd/system/battlecp-backend.service << 'EOF'
[Unit]
Description=Battle CP Backend
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/root/Battle-CP/backend
ExecStart=/root/Battle-CP/backend/target/release/backend
Environment=PORT=3001
Environment=ALLOWED_ORIGINS=https://your-project-xxx.vercel.app
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now battlecp-backend
systemctl status battlecp-backend
```

Replace `your-project-xxx` with your Vercel domain.

### Step 6: (Optional) Add Custom Domain with HTTPS

If you have a domain (e.g., `api.yourdomain.com`):

```bash
# Point DNS A record to your droplet IP first

# Update ALLOWED_ORIGINS in the systemd service
nano /etc/systemd/system/battlecp-backend.service
# Change: ALLOWED_ORIGINS=https://yourdomain.com

systemctl restart battlecp-backend

# Get SSL cert with certbot
certbot certonly --standalone -d api.yourdomain.com

# Configure nginx as reverse proxy
cat > /etc/nginx/sites-available/api.yourdomain.com << 'EOF'
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
        proxy_pass http://127.0.0.1:3001;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_read_timeout 86400;
    }
}
EOF

ln -s /etc/nginx/sites-available/api.yourdomain.com /etc/nginx/sites-enabled/
nginx -t
systemctl restart nginx
```

Then in Vercel update env vars to `NEXT_PUBLIC_WS_URL=wss://api.yourdomain.com`.

**Cost**: $6/mo (Droplet) + $0 Vercel = ~33 months free with credits

---

## 🆓 FREE ALTERNATIVES

### Option 1: Fly.io (Backend) + Vercel (Frontend) — Completely Free Tier

```bash
# Create frontend environment file
cd ~/Battle-CP/frontend
cat > .env.local << 'EOF'
NEXT_PUBLIC_API_URL=http://YOUR_DROPLET_IP:3001
NEXT_PUBLIC_WS_URL=ws://YOUR_DROPLET_IP:3001
EOF

# Replace YOUR_DROPLET_IP with actual IP
nano .env.local
```

### Step 7: Start with PM2

```bash
# Start backend
cd ~/Battle-CP/backend
PORT=3001 ALLOWED_ORIGINS="http://YOUR_DROPLET_IP:3000" pm2 start ./target/release/backend --name battlecp-backend

# Start frontend
cd ~/Battle-CP/frontend
pm2 start npm --name battlecp-frontend -- start -- -p 3000

# Save PM2 config (survives reboot)
pm2 save
pm2 startup
# Run the command it outputs!
```

### Step 8: Open Firewall

```bash
# Allow HTTP, HTTPS, and your ports
ufw allow 22
ufw allow 80
ufw allow 443
ufw allow 3000
ufw allow 3001
ufw --force enable
```

### Step 9: Test It!

Open in browser: `http://YOUR_DROPLET_IP:3000`

You should see Battle CP! 🎉

### Step 10: (Optional) Add a Domain with HTTPS

If you have a domain (or get a free one from [Freenom](https://freenom.com)):

1. Point your domain's A record to your droplet IP
2. Configure Caddy:

```bash
cat > /etc/caddy/Caddyfile << 'EOF'
yourdomain.com {
    reverse_proxy localhost:3000
}

api.yourdomain.com {
    reverse_proxy localhost:3001
}
EOF

systemctl restart caddy
```

3. Update frontend env:
```bash
cd ~/Battle-CP/frontend
cat > .env.local << 'EOF'
NEXT_PUBLIC_API_URL=https://api.yourdomain.com
NEXT_PUBLIC_WS_URL=wss://api.yourdomain.com
EOF

npm run build
pm2 restart battlecp-frontend
```

4. Update backend CORS:
```bash
pm2 delete battlecp-backend
cd ~/Battle-CP/backend
PORT=3001 ALLOWED_ORIGINS="https://yourdomain.com" pm2 start ./target/release/backend --name battlecp-backend
pm2 save
```

### Useful Commands

```bash
# View logs
pm2 logs battlecp-backend
pm2 logs battlecp-frontend

# Restart after code changes
cd ~/Battle-CP && git pull
cd backend && cargo build --release
cd ../frontend && npm run build
pm2 restart all

# Check status
pm2 status
```

### Cost Breakdown

| Resource | Monthly Cost |
|----------|--------------|
| $6 Droplet (1GB) | $6 |
| **Your Cost** | **$0** (covered by $200 credits) |
| **Duration** | ~33 months! |

---

## Option 2: Docker Deployment

### Backend Dockerfile

```dockerfile
FROM rust:1.75-slim AS builder
WORKDIR /app

# Install build dependencies for reqwest/openssl
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY backend/ .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

# Install runtime certificates
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/backend ./
ENV PORT=3000
ENV WEBSITES_PORT=3000
EXPOSE 3000
CMD ["./backend"]
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

**Why Fly.io?**
- Free tier: 3 shared-cpu-1x 256MB VMs per month
- Perfect for testing, personal use
- Auto-scales, handles WebSockets natively
- Simple deployment from CLI

### Steps:

1. **Frontend**: Deploy on Vercel (same as above, free tier)
2. **Backend**: Deploy on Fly.io
   ```bash
   # Install fly CLI
   curl -L https://fly.io/install.sh | sh
   
   # Sign up
   flyctl auth signup
   
   # Create fly.toml in repo root
   mkdir fly-app && cd fly-app
   cat > fly.toml << 'EOF'
   app = "battlecp-backend"
   
   [env]
   ALLOWED_ORIGINS = "https://your-vercel-domain.vercel.app"
   PORT = "3001"
   
   [build]
   dockerfile = "../backend/Dockerfile"
   
   [[services]]
   protocol = "tcp"
   internal_port = 3001
   processes = ["app"]
   
   [[services.ports]]
   port = 443
   handlers = ["tls"]
   
   [[services.ports]]
   port = 80
   EOF
   
   # Deploy
   flyctl deploy
   # Gets a domain like: battlecp-backend-xxxxx.fly.dev
   ```

3. **Update Vercel env vars**:
   ```
   NEXT_PUBLIC_API_URL=https://battlecp-backend-xxxxx.fly.dev
   NEXT_PUBLIC_WS_URL=wss://battlecp-backend-xxxxx.fly.dev
   ```

**Cost**: $0 (both free tiers)

---

### Option 2: You Run Your Own Server at Home — Completely Free (Advanced)

**Best for**: Learning, testing with friends, keeping full control

Can you host it from your home computer? Yes, but:

**Pros:**
- Completely free (your electricity cost)
- Full control
- Great for learning networking, Docker, security

**Cons:**
- Requires a public IP or tunnel (most home ISPs block port 80/443)
- Uptime depends on your machine running 24/7
- Dynamic IP requires DDNS or tunnel
- No SLA or monitoring
- Internet speed is often asymmetric (slow upload)

**How:**

1. **Option A: Use a Tunnel (Easiest)**
   - Use ngrok, Cloudflare Tunnel, or Tailscale to expose your local server
   - Example with Cloudflare Tunnel:
     ```bash
     # Install cloudflared
     brew install cloudflare/cloudflare/cloudflared  # macOS
     # or apt install cloudflared  # Linux
     
     # Tunnel your local backend
     cloudflared tunnel --url http://localhost:3001 --hostname api.yourdomain.com
     # (requires DNS pointing to Cloudflare)
     ```
   - Frontend on Vercel points to the tunnel domain
   - **Limitation**: Free Cloudflare Tunnel has limits; paid tiers exist

2. **Option B: Port Forward (If ISP Allows)**
   - Forward your router's port 80/443 to your machine
   - Use a DDNS service (DuckDNS, No-IP) to handle dynamic IP
   - Set up nginx + certbot on your machine (same as Droplet steps)
   - This requires your ISP to not block inbound ports (many do)

3. **Option C: Hybrid — Run Backend Locally, Frontend on Vercel (Best for Testing)**
   - Run `cargo run` on your home machine
   - Expose with ngrok: `ngrok http 3001`
   - Use ngrok URL in Vercel env vars: `NEXT_PUBLIC_WS_URL=wss://your-ngrok-domain.com`
   - **Perfect for**: testing with a friend before deploying
   - **Cost**: Ngrok free tier ~40 conn/min, paid tiers $5+

**My recommendation**: Use Fly.io free tier first for learning, then DigitalOcean App Platform with your $200 credits for reliable production.

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
