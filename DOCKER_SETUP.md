# Docker Setup Guide

StarForge provides Docker and Docker Compose configurations for reproducible development and testing environments.

## Quick Start

### Prerequisites
- Docker 20.10+
- Docker Compose 2.0+ (for multi-container workflows)

### Running StarForge with Docker

**Execute a single command:**
```bash
docker run --rm ghcr.io/nanle-code/starforge:latest <command>
```

**Example: Get version**
```bash
docker run --rm ghcr.io/nanle-code/starforge:latest --version
```

**Example: Deploy a contract**
```bash
docker run --rm -v $(pwd):/workspace ghcr.io/nanle-code/starforge:latest deploy --wasm ./contract.wasm --network testnet
```

## Development Setup

### Building Locally

**Build production image:**
```bash
docker build -t starforge:local .
```

**Build development image:**
```bash
docker build -f Dockerfile.dev -t starforge:dev .
```

### Using Docker Compose

Docker Compose sets up StarForge with a local Stellar + Soroban environment.

**Start all services:**
```bash
docker-compose up -d
```

This brings up:
- `stellar-testnet`: Local Stellar/Soroban instance on `http://localhost:8000`
- `soroban-rpc`: Dedicated Soroban RPC on `http://localhost:8001`
- `starforge`: Development container with live editing

**Run commands:**
```bash
# Execute a command in the starforge container
docker-compose run --rm starforge starforge deploy --wasm ./token.wasm --network docker-testnet

# Interactive shell
docker-compose run --rm starforge bash

# Live development (edits to code auto-reload)
docker-compose run --rm starforge cargo run -- <command>
```

**Stop services:**
```bash
docker-compose down
```

### Development Workflow

1. **Start containers:**
   ```bash
   docker-compose up -d
   ```

2. **Edit code** (changes reflect immediately due to volume mount)

3. **Test in container:**
   ```bash
   docker-compose run --rm starforge cargo test
   ```

4. **Debug with hot-reload:**
   ```bash
   docker-compose run --rm starforge cargo run -- wallet list
   ```

5. **Stop when done:**
   ```bash
   docker-compose down
   ```

## Image Variants

### Production Image (`Dockerfile`)
- **Size**: ~100MB (optimized with multi-stage build)
- **Base**: `debian:bookworm-slim`
- **Includes**: Binary only, Stellar CLI
- **Use case**: Running deployed versions, CI/CD pipelines
- **Command**: `starforge <args>` (default entrypoint)

### Development Image (`Dockerfile.dev`)
- **Size**: ~2GB (includes Rust toolchain)
- **Base**: `rust:1-bookworm`
- **Includes**: Full build environment, cargo
- **Use case**: Development, testing, contributions
- **Command**: `cargo run -- <args>`

## Environment Variables

All StarForge environment variables are supported in containers:

```bash
# Example with custom RPC URL
docker run \
  -e STELLAR_RPC_URL=http://soroban-rpc:8000 \
  -e STELLAR_NETWORK=local \
  starforge:local deploy --wasm ./contract.wasm
```

## Common Workflows

### Deploy to Local Network
```bash
docker-compose run --rm starforge \
  starforge deploy \
  --wasm ./target/wasm32-unknown-unknown/release/contract.wasm \
  --network docker-testnet
```

### Run Test Suite in Container
```bash
docker-compose run --rm starforge cargo test --locked
```

### Create a Plugin
```bash
docker-compose run --rm starforge \
  starforge plugin create my-plugin
```

### Hardware Wallet Testing (Ledger)
```bash
# Note: USB passthrough required
docker run \
  --device /dev/bus/usb \
  -e STELLAR_HARDWARE_WALLET=ledger \
  starforge:local wallet import --hardware ledger
```

## Troubleshooting

### Build fails with "SSL error"
Ensure you're connected to the internet. The Dockerfile installs Stellar CLI via a remote script.

### Cargo cache not persisting
Volume `cargo-cache` is defined in `docker-compose.yml` for cache persistence. Clean it if needed:
```bash
docker volume rm starforge_cargo-cache
```

### Permission denied on mounted volumes
Use `--user` flag when needed:
```bash
docker-compose run --rm --user root starforge bash
```

### Network connectivity in containers
Services communicate via hostnames defined in `docker-compose.yml`:
- StarForge → Soroban RPC: `http://soroban-rpc:8000`
- Soroban RPC → Stellar: Uses internal Stellar quickstart networking

### Container exits immediately
Check logs:
```bash
docker-compose logs starforge
```

## Best Practices

1. **Use `--rm` flag**: Automatically removes containers after execution
2. **Mount volumes correctly**: `-v $(pwd):/workspace` ensures file access
3. **Use `docker-compose`**: Provides consistent multi-container environment
4. **Cache dependencies**: Dev image pre-builds dependencies to speed up iterations
5. **Pin versions**: Use specific image tags in production (avoid `:latest`)

## Contributing

When contributing, use the development image and Docker Compose:

```bash
# Build from your branch
docker build -f Dockerfile.dev -t starforge:dev-branch .

# Test changes
docker-compose run --rm starforge cargo test

# Run linter and formatter checks
docker-compose run --rm starforge cargo fmt --all --check
docker-compose run --rm starforge cargo clippy --locked
```

## See Also

- [DEVELOPER_GUIDE.md](DEVELOPER_GUIDE.md) - Full development setup
- [README.md](README.md) - General project information
- [docker-compose.yml](docker-compose.yml) - Service configuration
