# ── Build a Docker container for traits.build ────────────────────────────────
FROM rust:latest AS builder

WORKDIR /build

# 1. Copy dependency manifests first (cached layer — only rebuilds when deps change)
COPY Cargo.toml Cargo.lock ./
COPY traits/kernel/plugin_api/Cargo.toml traits/kernel/plugin_api/Cargo.toml
COPY traits/www/traits/build/Cargo.toml traits/www/traits/build/Cargo.toml

# 2. Create stub source files so cargo can resolve the workspace and cache deps
#    Note: real `src` is a symlink → `traits/kernel/main`, but we create a real dir here
#    for dep caching. Step 4 removes it before COPY to avoid BuildKit symlink conflict.
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
    && mkdir -p traits/kernel/plugin_api/src && echo '' > traits/kernel/plugin_api/src/lib.rs \
    && mkdir -p traits/www/traits/build/src && echo 'pub fn dummy() {}' > traits/www/traits/build/src/lib.rs

# 3. Build dependencies only (this layer is cached until Cargo.toml/lock changes)
RUN cargo build --release 2>/dev/null || true

# 4. Remove stub dirs that conflict with symlinks in the real source tree
RUN rm -rf src

# 5. Copy real source (invalidates only from here)
COPY . .

# 6. Touch source files to ensure they rebuild (stubs may have newer timestamps)
RUN find traits -name '*.rs' -exec touch {} + && find traits -name '*.toml' -exec touch {} +

# 7. Build the release binary
RUN cargo build --release

# ── Runtime image ───────────────────────────────────────────────────────
FROM debian:trixie-slim

# Install runtime dependencies (Rust-only kernel — no JS/Python workers)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /traits

# Copy the binary and trait definitions (for TOML registry)
COPY --from=builder /build/target/release/traits /usr/local/bin/traits
COPY traits.toml traits.toml

# Ensure binary has proper permissions
RUN chmod +x /usr/local/bin/traits

# Set environment and expose port
ENV TRAITS_PORT=8090
ENV TRAITS_BIND=0.0.0.0
ENV RUST_LOG=info
EXPOSE 8090

ENTRYPOINT []
# Prefer binary on persistent volume (/data/traits) if present — allows fast-deploy
# to update the binary without rebuilding the Docker image
CMD ["sh", "-c", "if [ -x /data/traits ]; then exec /data/traits; else exec traits; fi"]
