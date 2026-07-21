#!/bin/bash
set -e

VM_NAME="battlecp-server"
ZONE="us-central1-a"
PROJECT="battle-cp-prod"

echo "🚀 Deploying Battle-CP to GCP..."

# Load env vars
if [ -f .env ]; then
  export $(grep -v '^#' .env | xargs)
fi

# 1. Build backend Docker image
echo "🔨 Building backend Docker image..."
docker build -t battlecp-backend:latest ./backend
docker save battlecp-backend:latest | gzip > /tmp/battlecp-backend.tar.gz

# 2. Build frontend
echo "🔨 Building frontend..."
cd frontend && npm run build && cd ..
tar czf /tmp/frontend.tar.gz --exclude='node_modules/.cache' --exclude='.next/cache' \
  .next node_modules package.json public app lib hooks types context utils

# 3. Transfer to VM
echo "☁️  Transferring to GCP VM..."
gcloud compute scp /tmp/battlecp-backend.tar.gz ${VM_NAME}:/tmp/ --zone=${ZONE}
gcloud compute scp /tmp/frontend.tar.gz ${VM_NAME}:/tmp/ --zone=${ZONE}

# 4. Deploy backend
echo "🔄 Deploying backend..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --command="
  sudo docker stop battlecp-backend 2>/dev/null || true
  sudo docker rm battlecp-backend 2>/dev/null || true
  sudo docker load < /tmp/battlecp-backend.tar.gz
  sudo docker run -d \
    --name battlecp-backend \
    --restart unless-stopped \
    -p 127.0.0.1:3000:3000 \
    -e PORT=3000 \
    -e RUST_LOG=info \
    -e ALLOWED_ORIGINS='https://battle-cp.vercel.app,http://localhost:3000' \
    -e DISCORD_WEBHOOK_URL='${DISCORD_WEBHOOK_URL}' \
    battlecp-backend:latest
"

# 5. Deploy frontend
echo "🔄 Deploying frontend..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --command="
  sudo rm -rf /opt/battlecp-frontend
  sudo mkdir -p /opt/battlecp-frontend
  cd /opt/battlecp-frontend
  sudo tar xzf /tmp/frontend.tar.gz
  sudo systemctl restart battlecp-frontend
"

# 6. Reload nginx
echo "🔄 Reloading nginx..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --command="sudo systemctl reload nginx"

# Cleanup
rm -f /tmp/battlecp-backend.tar.gz /tmp/frontend.tar.gz

EXTERNAL_IP=$(gcloud compute instances describe ${VM_NAME} --zone=${ZONE} --format='get(networkInterfaces[0].accessConfigs[0].natIP)')
echo ""
echo "✅ Deployed! Live at: http://${EXTERNAL_IP}"
