#!/bin/bash
set -e

# Battle-CP GCP Deployment Script
# Deploys to Compute Engine e2-micro VM (always-free tier)

VM_NAME="battlecp-server"
ZONE="us-central1-a"
PROJECT="battle-cp-prod"

echo "🚀 Starting Battle-CP GCP Deployment..."

# Load environment variables from .env
if [ -f .env ]; then
  export $(grep -v '^#' .env | xargs)
fi

# Build Docker image locally
echo "🔨 Building Rust Docker image..."
docker build -t battlecp-backend:latest ./backend

# Save image as tar.gz
echo "📦 Saving Docker image..."
docker save battlecp-backend:latest | gzip > /tmp/battlecp-backend.tar.gz

# Transfer to VM
echo "☁️  Transferring image to GCP VM..."
gcloud compute scp /tmp/battlecp-backend.tar.gz ${VM_NAME}:/tmp/battlecp-backend.tar.gz --zone=${ZONE}

# Stop and remove old container
echo "🔄 Stopping old container..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --command="
  sudo docker stop battlecp-backend 2>/dev/null || true
  sudo docker rm battlecp-backend 2>/dev/null || true
"

# Load new image and start container
echo "🚀 Deploying new container..."
gcloud compute ssh ${VM_NAME} --zone=${ZONE} --command="
  sudo docker load < /tmp/battlecp-backend.tar.gz
  sudo docker run -d \
    --name battlecp-backend \
    --restart unless-stopped \
    -p 80:3000 \
    -e PORT=3000 \
    -e RUST_LOG=info \
    -e ALLOWED_ORIGINS='${ALLOWED_ORIGINS}' \
    -e DISCORD_WEBHOOK_URL='${DISCORD_WEBHOOK_URL}' \
    battlecp-backend:latest
"

# Clean up local tar
rm -f /tmp/battlecp-backend.tar.gz

# Get VM IP
EXTERNAL_IP=$(gcloud compute instances describe ${VM_NAME} --zone=${ZONE} --format='get(networkInterfaces[0].accessConfigs[0].natIP)')

echo ""
echo "✅ Deployment complete!"
echo ""
echo "Backend URL: http://${EXTERNAL_IP}"
echo "WebSocket:   ws://${EXTERNAL_IP}"
echo ""
echo "📋 Update your Vercel env vars:"
echo "   NEXT_PUBLIC_API_URL=http://${EXTERNAL_IP}"
echo "   NEXT_PUBLIC_WS_URL=ws://${EXTERNAL_IP}"
echo ""
echo "Don't forget to rebuild the frontend after updating env vars!"
