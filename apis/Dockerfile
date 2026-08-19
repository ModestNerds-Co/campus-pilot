# syntax=docker/dockerfile:1
FROM rust:1.91-bookworm AS builder
WORKDIR /app
ENV SQLX_OFFLINE=false
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && printf "" > src/lib.rs
RUN cargo build --release && rm -rf src
COPY src ./src
COPY migrations ./migrations
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
