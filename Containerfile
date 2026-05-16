# ── builder (amd64 nativo, cross-compila para arm64) ───────────────────────
FROM --platform=linux/amd64 rust:latest AS builder

RUN apt-get update && \
    apt-get install -y gcc-aarch64-linux-gnu && \
    rustup target add aarch64-unknown-linux-gnu && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN mkdir -p .cargo && \
    printf '[target.aarch64-unknown-linux-gnu]\nlinker = "aarch64-linux-gnu-gcc"\n' \
    > .cargo/config.toml

# Cachear dependencias antes de copiar el código real
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release --target aarch64-unknown-linux-gnu && \
    rm -rf src

# Build real
COPY src ./src
RUN touch src/main.rs && \
    cargo build --release --target aarch64-unknown-linux-gnu

# ── runtime (arm64) ────────────────────────────────────────────────────────
FROM --platform=linux/arm64 debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/aarch64-unknown-linux-gnu/release/xplaya .
COPY templates ./templates
COPY static ./static
EXPOSE 3000
CMD ["./xplaya"]
