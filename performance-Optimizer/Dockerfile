# Dockerfile - Multi-stage Docker build for P2P File Converter

# Build stage
FROM rust:1.75-slim AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libfontconfig1-dev \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy dependency files
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release && rm -rf src

# Copy source code
COPY src ./src
COPY examples ./examples
COPY benches ./benches

# Build the actual application
RUN cargo build --release --bin p2p-converter

# Runtime stage
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libfontconfig1 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN groupadd -r p2pconv && useradd -r -g p2pconv p2pconv

# Create necessary directories
RUN mkdir -p /app/data /app/config /app/logs && \
    chown -R p2pconv:p2pconv /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/p2p-converter /usr/local/bin/p2p-converter

# Copy configuration files
COPY config/docker.toml /app/config/default.toml

# Set up volumes
VOLUME ["/app/data", "/app/logs"]

# Switch to non-root user
USER p2pconv

# Set working directory
WORKDIR /app

# Expose default port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD p2p-converter --help || exit 1

# Set default command
CMD ["p2p-converter", "--config", "/app/config/default.toml", "--listen", "/ip4/0.0.0.0/tcp/8080"]

# Multi-architecture build support
FROM runtime AS runtime-amd64
FROM runtime AS runtime-arm64

# Final stage
FROM runtime-${TARGETARCH} AS final
