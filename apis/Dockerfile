# syntax=docker/dockerfile:1
FROM rust:1.91-bookworm AS builder
WORKDIR /app
ENV SQLX_OFFLINE=true

# Cargo workspace (common + app + one crate per ERP module).
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
COPY .sqlx ./.sqlx
RUN --mount=type=cache,id=campus-pilot-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=campus-pilot-target,target=/app/target \
    touch crates/app/src/db/mod.rs && \
    cargo build --release --bin campus-pilot --bin agent-worker && \
    cp target/release/campus-pilot /tmp/campus-pilot && \
    cp target/release/agent-worker /tmp/agent-worker

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 curl wget && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /tmp/campus-pilot ./campus-pilot
COPY --from=builder /tmp/agent-worker ./agent-worker
COPY migrations ./migrations
RUN useradd -r -s /bin/false appuser && chown -R appuser:appuser /app
USER appuser
EXPOSE 8000
CMD ["./campus-pilot"]
