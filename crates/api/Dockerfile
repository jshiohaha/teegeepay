FROM rust:1.88-slim-bookworm AS builder

WORKDIR /workspace

# Install dependencies needed for the build
RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq-dev libssl-dev pkg-config build-essential g++ && \
    rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates crates

# Create dummy source file for 'solana-api' for dependency caching
RUN mkdir -p ./crates/api/src
RUN echo "fn main() { println!(\"This is the dummy function.\") }" > ./crates/api/src/main.rs

# Build with debug symbols and more verbose output
ENV RUST_LOG=debug \
    RUST_BACKTRACE=full

# Build the application
RUN cargo build --release --bin solana-api

# Remove the dummy main.rs
RUN rm ./crates/api/src/main.rs

# Copy the actual source code
COPY crates/api crates/api

RUN touch crates/api/src/main.rs

RUN cargo build --release --bin solana-api

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install only the necessary runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libpq5 \
    libssl3 \
    zlib1g && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /workspace/target/release/solana-api /app/solana-api

RUN chmod +x /app/solana-api

# Set environment variables
ENV RUST_LOG=info \
    DATABASE_URL=${DATABASE_URL}

EXPOSE 3000

CMD ["/app/solana-api"]
