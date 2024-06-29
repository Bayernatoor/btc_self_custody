# Builder stage 
# Build env using Rust nightly
FROM rustlang/rust:nightly-bullseye AS chef 

RUN cargo install cargo-chef

# install cargo-binstall, makes it easier to install other cargo extensions
RUN wget https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz
RUN tar -xvf cargo-binstall-x86_64-unknown-linux-musl.tgz
RUN cp cargo-binstall /usr/local/cargo/bin

# Install cargo-leptos
RUN cargo binstall cargo-leptos -y

# Add the WASM target
RUN rustup target add wasm32-unknown-unknown

WORKDIR /app

FROM chef AS planner

# Copy all files from our working environment to our Docker image 
COPY . .

# Compute a lock-like file for our project
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build project dependencies, not application.
RUN cargo chef cook --release --recipe-path recipe.json

# at this point, if our dependency tree stays the same,
# all layers should be cached.
COPY . . 
# SQLX should use offline saved queries at compile-time
ENV SQLX_OFFLINE=true
# Build release binary
RUN cargo leptos build --release -vv

# Runtime stage
FROM debian:bookworm-slim AS runtime

WORKDIR /app
RUN apt-get update -y \
  && apt-get install -y --no-install-recommends openssl ca-certificates \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

# -- NB: update binary name from "leptos_start" to match your app name in Cargo.toml --
# Copy the server binary to the /app directory
COPY --from=builder /app/target/release/we_hodl_btc /app/

# /target/site contains our JS/WASM/CSS, etc.
COPY --from=builder /app/target/site /app/site

# Copy src - need to reach server functions
COPY --from=builder /app/src /app/src

# Copy the configuration directory to the /app directory
COPY --from=builder /app/configuration /app/configuration

# Copy Cargo.toml if it’s needed at runtime
COPY --from=builder /app/Cargo.toml /app/

# Set any required env variables and
ENV APP_ENVIRONMENT=production
ENV RUST_LOG="info"
#ENV LEPTOS_SITE_ADDR="0.0.0.0:8000"
ENV LEPTOS_SITE_ROOT="site"
#EXPOSE 8000

# When `docker run` is executed, launch the binary!
CMD ["/app/we_hodl_btc"]
