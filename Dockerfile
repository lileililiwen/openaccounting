FROM rust:1.82-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock* ./
COPY migrations ./migrations
COPY src ./src
COPY templates ./templates
COPY static ./static
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/openaccounting /usr/local/bin/openaccounting
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/static ./static
COPY --from=builder /app/migrations ./migrations
RUN mkdir -p /var/lib/openaccounting/documents
ENV DOCUMENTS_DIR=/var/lib/openaccounting/documents
EXPOSE 3000
CMD ["openaccounting"]
