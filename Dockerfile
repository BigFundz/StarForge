# Multi-stage build for optimal image size and build consistency
FROM rust:1-bookworm as builder

WORKDIR /build

# Install system deps commonly needed for Rust + TLS
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    pkg-config \
    libssl-dev \
    curl \
 && rm -rf /var/lib/apt/lists/*

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./

# Cache dependencies by building a dummy binary first
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --locked && \
    rm -rf src

# Copy source and build the actual binary
COPY . .
RUN cargo build --release --locked && \
    cargo install --path . --locked

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /workspace

# Install only runtime deps and Stellar CLI
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    git \
 && rm -rf /var/lib/apt/lists/*

# Install Stellar CLI (used by `starforge shell` sandbox execution)
RUN (curl -fsSL https://stellar.org/install.sh | bash) || true

# Copy binary from builder
COPY --from=builder /usr/local/cargo/bin/starforge /usr/local/bin/

# Set starforge as default entrypoint
ENTRYPOINT ["starforge"]
CMD ["--help"]

