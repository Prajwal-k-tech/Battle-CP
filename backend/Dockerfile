# Build stage
FROM rust:1.85-slim AS builder
WORKDIR /app

# Install build dependencies for reqwest/openssl
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy backend files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data ./data

# Build the release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime certificates (required for reqwest to make HTTPS calls)
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/backend ./

# Azure App Service and local port configuration
ENV PORT=3000
ENV WEBSITES_PORT=3000
ENV RUST_LOG=info
ENV DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/1483136702838083799/vOZapcWehMPCjMXHlNxFUPTLzNIKo8UHZpk0VNE4dZ7IEDCdVyQjScMaJFcAp0d3XNCK

EXPOSE 3000

# Raise file descriptor limit for 500+ concurrent WebSocket connections
# Default Linux ulimit (1024) is too low for 1000 WS connections + HTTP + misc FDs
CMD ["sh", "-c", "ulimit -n 65535 2>/dev/null; exec ./backend"]
