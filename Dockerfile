# syntax=docker/dockerfile:1.7

FROM rust:1.75-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY README.md ./

RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/taskkit /usr/local/bin/taskkit
COPY .env.example /app/.env.example

ENV TASKEN_DATA_DIR=/data/taskkit
ENV TASKEN_STORE_FILE=store.json
ENV TASKEN_CACHE_DIR=/data/taskkit/cache
ENV TASKEN_CACHE_FILE=cache.json
ENV TASKEN_LOG_FORMAT=json

VOLUME ["/data/taskkit"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["taskkit", "health"]

ENTRYPOINT ["taskkit"]
CMD ["health"]
