# syntax=docker/dockerfile:1.7

# -------- Build stage with BuildKit cache mounts --------
FROM rust:1.82-slim AS builder

WORKDIR /app

# Install build deps in their own layer (cached as long as this line doesn't change)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create dummy source files so cargo can resolve deps before real sources are mounted.
# This lets us cache the dep build separately.
RUN mkdir -p src migrations \
    && echo "fn main() {}" > src/main.rs \
    && touch migrations/.gitkeep

# Copy manifests first — invalidates only when deps change
COPY Cargo.toml Cargo.lock* ./

# Build dependencies only. BuildKit cache mounts share:
#   - /usr/local/cargo/registry  (downloaded crates)
#   - /app/target                (compiled artifacts)
# across builds. Second build with no dep changes: ~5s.
# First build: still ~5 min, but second onward is dramatically faster.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --profile release-fast --bin openaccounting \
    && rm -rf src target/release-fast/deps/openaccounting* target/release-fast/openaccounting

# Now copy the real source and build the actual binary
COPY migrations ./migrations
COPY src ./src
COPY templates ./templates
COPY static ./static

# Touch main.rs so cargo knows the binary source has changed
RUN touch src/main.rs

# Final build — only the app crate needs to recompile.
# The same cache mounts mean if you ran this build before, the registry & dep
# artifacts are already present and the build is incremental.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --profile release-fast --bin openaccounting \
    && strip target/release-fast/openaccounting \
    && cp target/release-fast/openaccounting /openaccounting-bin

# -------- Runtime stage --------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 tini \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /openaccounting-bin /usr/local/bin/openaccounting
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/static ./static
COPY --from=builder /app/migrations ./migrations

RUN mkdir -p /var/lib/openaccounting/documents

ENV DOCUMENTS_DIR=/var/lib/openaccounting/documents \
    RUST_LOG=info,openaccounting=debug,sqlx=warn

EXPOSE 3000

# tini gives us proper signal handling and PID 1 zombie reaping
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["openaccounting"]
