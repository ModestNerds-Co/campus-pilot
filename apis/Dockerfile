# syntax=docker/dockerfile:1
FROM rust:1.91-bookworm AS builder
WORKDIR /app
ENV SQLX_OFFLINE=true

# Cargo workspace (common + app + one crate per ERP module) — copied as a
# whole rather than manifests-first, since with 14 member crates the
# per-crate dependency-caching trick trades most of its benefit for a lot
# of Dockerfile noise. Deps still cache across builds that don't touch
# Cargo.toml/Cargo.lock, just not as granularly as a single-crate app.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
COPY .sqlx ./.sqlx
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 curl wget && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/campus-pilot ./campus-pilot
COPY migrations ./migrations
RUN useradd -r -s /bin/false appuser && chown -R appuser:appuser /app
USER appuser
EXPOSE 8000
CMD ["./campus-pilot"]
