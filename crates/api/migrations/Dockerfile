FROM rust:1.88-slim-bookworm AS development

WORKDIR /usr/src/app/crates/api

RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq-dev \
    pkg-config \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install sqlx-cli --no-default-features --features postgres

CMD ["sqlx"]
