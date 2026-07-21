#!/bin/bash
# ============================================================
# Battle CP - Oracle Cloud VM Initial Setup
# Run this ONCE on a fresh Oracle Cloud A1 Flex instance
#
# Usage: ssh ubuntu@<VM_IP> 'bash -s' < oracle_setup.sh
# ============================================================
set -euo pipefail

echo "🔧 Battle CP - Oracle Cloud VM Setup"
echo "======================================"

# ---- System update ----
echo "📦 Updating system packages..."
sudo apt-get update && sudo apt-get upgrade -y

# ---- Install Docker ----
echo "🐳 Installing Docker..."
if ! command -v docker &> /dev/null; then
    curl -fsSL https://get.docker.com | sudo sh
    sudo usermod -aG docker "$USER"
    echo "✅ Docker installed. Log out and back in for group changes."
else
    echo "✅ Docker already installed."
fi

# ---- Install Docker Compose ----
echo "🔧 Installing Docker Compose..."
if ! command -v docker compose &> /dev/null; then
    sudo apt-get install -y docker-compose-plugin
fi
echo "✅ Docker Compose ready."

# ---- Configure Docker log rotation (prevents disk filling up) ----
echo "📝 Configuring Docker log rotation..."
sudo mkdir -p /etc/docker
if [ ! -f /etc/docker/daemon.json ]; then
    sudo tee /etc/docker/daemon.json > /dev/null <<'DOCKER_CONFIG'
{
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  },
  "default-ulimits": {
    "nofile": {
      "Name": "nofile",
      "Hard": 65535,
      "Soft": 65535
    }
  }
}
DOCKER_CONFIG
    sudo systemctl restart docker
    echo "✅ Docker configured with log rotation and ulimits."
else
    echo "✅ Docker already configured."
fi

# ---- Set up firewall ----
echo "🔥 Configuring firewall (UFW)..."
sudo apt-get install -y ufw
sudo ufw allow 22/tcp    # SSH
sudo ufw allow 80/tcp    # HTTP (for Certbot)
sudo ufw allow 443/tcp   # HTTPS (WebSocket)
sudo ufw --force enable
echo "✅ Firewall configured."

# ---- Install Nginx ----
echo "🌐 Installing Nginx..."
sudo apt-get install -y nginx
echo "✅ Nginx installed."

# ---- Install Certbot (for free TLS) ----
echo "🔒 Installing Certbot..."
sudo apt-get install -y certbot python3-certbot-nginx
echo "✅ Certbot installed."

# ---- Set up anti-idle cron ----
echo "⏰ Setting up anti-idle cron job..."
ANTI_IDLE_SCRIPT="/home/ubuntu/battlecp/oracle_anti_idle.sh"
if [ -f "$ANTI_IDLE_SCRIPT" ]; then
    (crontab -l 2>/dev/null | grep -v "oracle_anti_idle"; echo "*/5 * * * * /usr/bin/bash $ANTI_IDLE_SCRIPT > /dev/null 2>&1") | crontab -
    echo "✅ Anti-idle cron installed."
else
    echo "⚠️  Anti-idle script not found at $ANTI_IDLE_SCRIPT. Add cron manually later."
fi

# ---- Raise file descriptor limits ----
echo "📄 Raising file descriptor limits..."
# Set in limits.conf (idempotent: only add if not present)
if ! grep -q "^\* soft nofile 65535" /etc/security/limits.conf 2>/dev/null; then
    sudo tee -a /etc/security/limits.conf > /dev/null <<'LIMITS'
* soft nofile 65535
* hard nofile 65535
LIMITS
fi
# Set in systemd
sudo sed -i 's/^#DefaultLimitNOFILE=.*/DefaultLimitNOFILE=65535/' /etc/systemd/system.conf 2>/dev/null || true
sudo sed -i 's/^DefaultLimitNOFILE=.*/DefaultLimitNOFILE=65535/' /etc/systemd/system.conf 2>/dev/null || true
if ! grep -q "^DefaultLimitNOFILE=65535" /etc/systemd/system.conf 2>/dev/null; then
    echo "DefaultLimitNOFILE=65535" | sudo tee -a /etc/systemd/system.conf > /dev/null
fi
echo "✅ File descriptor limits set."

# ---- Create app directory ----
echo "📁 Creating app directory..."
mkdir -p ~/battlecp/backend
echo "✅ App directory ready at ~/battlecp"

echo ""
echo "======================================"
echo "✅ VM Setup Complete!"
echo "======================================"
echo ""
echo "Next steps:"
echo "  1. Log out and back in (for Docker group to take effect)"
echo "  2. Copy your deploy files to ~/battlecp/"
echo "  3. Set up DNS: point your domain to this VM's IP"
echo "  4. Get TLS cert: sudo certbot --nginx -d your.domain.com"
echo "  5. Deploy: ORACLE_SSH_HOST=ubuntu@<IP> bash deploy_oracle.sh"
echo ""
