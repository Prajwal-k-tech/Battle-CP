# Build stage
FROM rust:1.75-alpine AS builder
WORKDIR /app
RUN apk add --no-cache musl-dev
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# Runtime stage
FROM alpine:latest
WORKDIR /app
COPY --from=builder /app/target/release/backend ./
EXPOSE 3000
ENV RUST_LOG=info
CMD ["./backend"]
