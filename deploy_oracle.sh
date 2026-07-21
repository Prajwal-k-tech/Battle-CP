#!/bin/bash
# ============================================================
# Battle CP - Oracle Cloud Deployment Script
# Builds the Rust backend locally and deploys to Oracle VM
#
# Usage:
#   ORACLE_SSH_HOST=ubuntu@<VM_IP> bash deploy_oracle.sh
#   bash deploy_oracle.sh  (reads ORACLE_SSH_HOST from .env)
# ============================================================
set -euo pipefail

echo "🚀 Battle CP - Oracle Cloud Deployment"
echo "======================================="

# ---- Config ----
REMOTE_APP_DIR="~/battlecp"
IMAGE_NAME="battlecp-backend"
CONTAINER_NAME="battlecp-backend"
NETWORK="battlecp-net"

# ---- Load env ----
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

# ---- Validate ----
if [ -z "${ORACLE_SSH_HOST:-}" ]; then
    echo "❌ ORACLE_SSH_HOST is not set."
    echo "   Usage: ORACLE_SSH_HOST=ubuntu@<VM_IP> bash deploy_oracle.sh"
    echo "   Or add ORACLE_SSH_HOST=ubuntu@<VM_IP> to your .env file"
    exit 1
fi

# ---- Check SSH connectivity ----
echo "🔍 Checking SSH connection to $ORACLE_SSH_HOST..."
if ! ssh -o ConnectTimeout=5 -o BatchMode=yes "$ORACLE_SSH_HOST" "echo OK" 2>/dev/null; then
    echo "❌ Cannot SSH to $ORACLE_SSH_HOST"
    echo "   Make sure:"
    echo "     1. The VM is running"
    echo "     2. Your SSH key is added (ssh-add -l)"
    echo "     3. Port 22 is open in the VCN Security List"
    exit 1
fi
echo "✅ SSH connection OK"

# ---- Sync files to remote ----
echo "📤 Syncing backend files to Oracle VM..."
rsync -avz --delete \
    --exclude 'target/' \
    --exclude 'node_modules/' \
    --exclude '.git/' \
    --exclude '.venv/' \
    --exclude '*.pyc' \
    ./backend/ "$ORACLE_SSH_HOST:$REMOTE_APP_DIR/backend/"
echo "✅ Files synced."

# ---- Copy Docker Compose and Nginx config ----
echo "📄 Copying deployment configs..."
scp docker-compose.yml "$ORACLE_SSH_HOST:$REMOTE_APP_DIR/"
scp nginx_battlecp.conf "$ORACLE_SSH_HOST:$REMOTE_APP_DIR/"
echo "✅ Configs copied."

# ---- Write .env on remote for docker-compose to pick up ----
echo "📝 Writing environment file on remote..."
ssh "$ORACLE_SSH_HOST" "cat > $REMOTE_APP_DIR/.env" << ENVEOF
RUST_LOG=${RUST_LOG:-info}
ALLOWED_ORIGINS=${ALLOWED_ORIGINS:-https://battle-cp.vercel.app}
DISCORD_WEBHOOK_URL=${DISCORD_WEBHOOK_URL:-}
ENVEOF
echo "✅ Environment file written."

# ---- Build and deploy remotely via SSH ----
echo "🔨 Building Docker image for ARM64 on Oracle VM (first build may take ~10 min)..."
# Note: heredoc with quoted 'REMOTE' — variables are NOT expanded locally.
# Runtime env vars are passed via the .env file above.
ssh "$ORACLE_SSH_HOST" << 'REMOTE'
    set -euo pipefail

    APP_DIR=~/battlecp
    IMAGE_NAME="battlecp-backend"
    CONTAINER_NAME="battlecp-backend"
    NETWORK="battlecp-net"

    cd "$APP_DIR/backend"

    # Pull latest Rust base images to speed up build
    docker pull rust:1.85-slim --platform linux/arm64 2>/dev/null || true

    # Build the image
    echo "🏗️  Building Docker image..."
    docker build --platform linux/arm64 -t $IMAGE_NAME:latest .

    # Ensure network exists
    docker network inspect $NETWORK &>/dev/null || docker network create $NETWORK

    # Stop and remove old container
    echo "🔄 Stopping old container..."
    docker stop $CONTAINER_NAME 2>/dev/null || true
    docker rm $CONTAINER_NAME 2>/dev/null || true

    # Source env vars for this session
    set -a; source "$APP_DIR/.env" 2>/dev/null || true; set +a

    # Start new container
    echo "🚀 Starting new container..."
    docker run -d \
        --name $CONTAINER_NAME \
        --restart unless-stopped \
        --network $NETWORK \
        -p 127.0.0.1:3000:3000 \
        -e PORT=3000 \
        -e RUST_LOG="${RUST_LOG:-info}" \
        -e ALLOWED_ORIGINS="${ALLOWED_ORIGINS:-https://battle-cp.vercel.app}" \
        -e DISCORD_WEBHOOK_URL="${DISCORD_WEBHOOK_URL:-}" \
        --memory 1g \
        --cpus 1.5 \
        $IMAGE_NAME:latest

    echo "✅ Container started."

    # Health check
    echo "⏳ Waiting for backend to be ready..."
    for i in $(seq 1 15); do
        if docker exec $CONTAINER_NAME curl -sf http://localhost:3000/health 2>/dev/null; then
            echo ""
            echo "✅ Backend is healthy!"
            break
        fi
        if [ "$i" -eq 15 ]; then
            echo ""
            echo "⚠️  Health check timed out. Check logs:"
            echo "   ssh $ORACLE_SSH_HOST 'docker logs $CONTAINER_NAME'"
        fi
        sleep 2
    done

    # Prune old images to save disk space
    docker image prune -f --filter "until=24h" 2>/dev/null || true
REMOTE

echo ""
echo "======================================="
echo "✅ Deployment Complete!"
echo "======================================="
echo ""
echo "Container: $CONTAINER_NAME"
echo "Backend:   http://localhost:3000 (local, via Nginx proxy)"
echo "Public:    https://your.domain.com (once DNS + TLS configured)"
echo ""
echo "Check logs: ssh $ORACLE_SSH_HOST 'docker logs -f $CONTAINER_NAME'"
echo ""
echo "Next steps if not done yet:"
echo "  1. Set up DNS: point your domain to the Oracle VM's public IP"
echo "  2. Run TLS: ssh $ORACLE_SSH_HOST 'sudo certbot --nginx -d your.domain.com'"
echo "  3. Update frontend: set NEXT_PUBLIC_API_URL in Vercel project settings"
