# Build stage
FROM rust:1.91.0 as builder

WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs

# Build dependencies (this will be cached unless Cargo.toml changes)
RUN cargo install sqlx-cli
RUN cargo sqlx prepare
RUN cargo build --release
RUN rm src/main.rs src/lib.rs

# Copy source code
COPY src ./src
COPY migrations ./migrations

# Build the application
RUN touch src/main.rs src/lib.rs
# Use a placeholder DATABASE_URL for sqlx prepare during build
RUN DATABASE_URL=postgresql://user:password@localhost:5432/database cargo sqlx prepare
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install necessary dependencies for the runtime
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/hulu-payments ./hulu-payments

# Copy migrations directory
COPY --from=builder /app/migrations ./migrations

# Create a non-root user
RUN useradd -r -s /bin/false appuser
RUN chown -R appuser:appuser /app
USER appuser

# Expose the port
EXPOSE 9010

# Run the application
CMD ["./hulu-payments"]
