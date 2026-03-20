# Build stage - Rust stable on Debian Bookworm
FROM rust:1-bookworm AS builder

# Install cargo-leptos and add WASM target
RUN cargo install cargo-leptos
RUN rustup target add wasm32-unknown-unknown

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./

# Copy source and assets
COPY src ./src
COPY style ./style
COPY assets ./assets

# Build release binary (SSR server + WASM client)
ENV LEPTOS_OUTPUT_NAME="we_hodl_btc"
RUN cargo leptos build --release -vv

# Runtime stage - same Debian version as builder
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update -y \
  && apt-get install -y --no-install-recommends openssl ca-certificates \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

# Copy the server binary
COPY --from=builder /app/target/release/we_hodl_btc /app/

# Copy the site assets (JS/WASM/CSS)
COPY --from=builder /app/target/site /app/site

# Copy source (needed for server functions that read FAQ files)
COPY --from=builder /app/src /app/src

# Copy Cargo.toml (leptos reads config from it at runtime)
COPY --from=builder /app/Cargo.toml /app/

ENV RUST_LOG="info"
ENV LEPTOS_SITE_ROOT="site"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
EXPOSE 8080

CMD ["/app/we_hodl_btc"]
