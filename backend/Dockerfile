# Build stage
FROM rust:1.85-slim AS builder
WORKDIR /app

# Install build dependencies for reqwest/openssl
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy backend files
COPY Cargo.toml Cargo.lock ./
COPY src ./src

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

EXPOSE 3000

# Run the binary
CMD ["./backend"]
