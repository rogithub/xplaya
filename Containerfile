# ── build ──────────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder
WORKDIR /app

RUN apt-get update && \
    apt-get install -y gcc-aarch64-linux-gnu && \
    rm -rf /var/lib/apt/lists/* && \
    rustup target add aarch64-unknown-linux-gnu

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

# Cache de dependencias: solo se recompilan si cambia Cargo.toml o Cargo.lock
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release --target aarch64-unknown-linux-gnu && \
    rm -rf src

# Build real
COPY src ./src
RUN touch src/main.rs && \
    cargo build --release --target aarch64-unknown-linux-gnu

# ── runtime ────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/aarch64-unknown-linux-gnu/release/xplaya .
COPY templates ./templates
COPY static ./static
EXPOSE 3000
CMD ["./xplaya"]
