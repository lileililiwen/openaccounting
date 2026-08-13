# syntax=docker/dockerfile:1.7

# -------- Build stage with cached dependency layer --------
FROM rust:1.82-slim AS builder

WORKDIR /app

# Install build deps
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create empty source files so cargo can resolve deps before we mount real sources.
# The real sources are then mounted as a separate layer and trigger only a rebuild
# of the app crate, not every dependency.
RUN mkdir -p src migrations \
    && echo "fn main() {}" > src/main.rs \
    && touch migrations/.gitkeep

# Copy manifests first — change here only invalidates the dep cache layer
COPY Cargo.toml Cargo.lock* ./

# Build dependencies only; this populates /usr/local/cargo/registry & target.
# Using release-fast profile (parallel codegen + thin LTO) so dep builds run in parallel.
RUN cargo build --profile release-fast --bin openaccounting \
    && rm -rf src target/release-fast/deps/openaccounting* target/release-fast/openaccounting

# Now copy the real source and build the actual binary
COPY migrations ./migrations
COPY src ./src
COPY templates ./templates
COPY static ./static

# Touch main.rs so cargo knows the binary source has changed
RUN touch src/main.rs

# Incremental build; only the app crate needs to recompile
RUN cargo build --profile release-fast --bin openaccounting \
    && strip target/release-fast/openaccounting

# -------- Runtime stage --------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 tini \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release-fast/openaccounting /usr/local/bin/openaccounting
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
