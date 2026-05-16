# ── build ──────────────────────────────────────────────────────────────────
FROM rust:1.85-slim AS builder
WORKDIR /app

# Cache de dependencias: solo se recompilan si cambia Cargo.toml o Cargo.lock
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Build real
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ── runtime ────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/xplaya .
COPY templates ./templates
COPY static ./static
EXPOSE 3000
CMD ["./xplaya"]
