#!/bin/bash
set -e
echo "🚀 Starting BattleCP Backend Deployment..."

# Get ACR credentials
echo "🔑 Fetching Azure Container Registry credentials..."
PASSWORD=$(az acr credential show -n battlecpcr --query "passwords[0].value" -o tsv)
USERNAME=$(az acr credential show -n battlecpcr --query "username" -o tsv)

if [ -z "$PASSWORD" ] || [ -z "$USERNAME" ]; then
    echo "❌ Error: Could not fetch ACR credentials. Are you logged in via 'az login'?"
    exit 1
fi

# Login to Docker
echo "🐳 Logging into Docker registry..."
echo $PASSWORD | sudo docker login battlecpcr.azurecr.io -u $USERNAME --password-stdin

# Build the Docker image
echo "🔨 Building Rust Docker image (this may take a few minutes)..."
sudo docker build -t battlecpcr.azurecr.io/battlecp-backend:latest ./backend

# Push the Docker image
echo "☁️ Pushing image to Azure..."
sudo docker push battlecpcr.azurecr.io/battlecp-backend:latest

# Update the Container App
echo "🔄 Updating Azure Container App to use the new image..."
az containerapp registry set \
  --name battlecp-backend \
  --resource-group battlecp-rg \
  --server battlecpcr.azurecr.io \
  --username $USERNAME \
  --password $PASSWORD

az containerapp update \
  --name battlecp-backend \
  --resource-group battlecp-rg \
  --image battlecpcr.azurecr.io/battlecp-backend:latest \
  --min-replicas 1 \
  --max-replicas 1 \
  --cpu 1.0 \
  --memory 2.0Gi \
  --set-env-vars "PORT=3000" "RUST_LOG=info" "ALLOWED_ORIGINS=https://battle-cp.vercel.app" "DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/1481384863276859442/9DDk-Bu7OBCyr4q643O9DsD4YqKMbiSGxqhHd5gWR4_A2ZqCRHmMkO_UFF2VgbtVxyhL" "DEPLOY_TIMESTAMP=$(date +%s)"

echo "✅ Deployment complete! Your backend is live at:"
az containerapp show \
  --name battlecp-backend \
  --resource-group battlecp-rg \
  --query properties.configuration.ingress.fqdn \
  -o tsv
