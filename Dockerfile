# Multi-stage build: compile in a full Rust image, run in a minimal one.
#
#   docker build -t api-starter-axum .
#   docker run --env-file .env -p 3000:3000 -p 9091:9091 api-starter-axum
#
# Migrations are embedded into the binary at compile time (see `sqlx::migrate!` in
# `infra/database.rs`), so the runtime image needs nothing from `migrations/` or `seeds/`.

FROM rust:1-slim-bookworm AS builder
WORKDIR /build

# webauthn-rs (passkeys) needs OpenSSL headers at build time; see the README's Known limits.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --release --bin api-starter-axum

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home --shell /usr/sbin/nologin api-starter-axum

COPY --from=builder /build/target/release/api-starter-axum /usr/local/bin/api-starter-axum

USER api-starter-axum
# API traffic and the Prometheus /metrics listener (see METRICS_BIND_ADDR).
EXPOSE 3000 9091
ENTRYPOINT ["api-starter-axum"]
