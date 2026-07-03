# ── builder (nativo en el runner, cross-compila según TARGETARCH) ──────────
FROM --platform=$BUILDPLATFORM rust:latest AS builder
# buildx la inyecta según la plataforma destino: amd64 | arm64
ARG TARGETARCH

RUN apt-get update && \
    apt-get install -y gcc-aarch64-linux-gnu && \
    rustup target add aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# TARGETARCH (amd64|arm64) → triple de Rust
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-gnu > /rust_target ;; \
      arm64) echo aarch64-unknown-linux-gnu > /rust_target ;; \
      *) echo "TARGETARCH no soportado: $TARGETARCH" && exit 1 ;; \
    esac

RUN mkdir -p .cargo && \
    printf '[target.aarch64-unknown-linux-gnu]\nlinker = "aarch64-linux-gnu-gcc"\n' \
    > .cargo/config.toml

# Cachear dependencias antes de copiar el código real
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release --target $(cat /rust_target) && \
    rm -rf src

# Build real
COPY src ./src
RUN touch src/main.rs && \
    cargo build --release --target $(cat /rust_target) && \
    cp target/$(cat /rust_target)/release/xplaya /app/xplaya-bin

# ── runtime (arquitectura destino, la fija buildx) ─────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/xplaya-bin ./xplaya
COPY templates ./templates
COPY static ./static
EXPOSE 3000
CMD ["./xplaya"]
