# xplaya

Tienda en línea de Papelería xplaya.com — `https://xplaya.com`

Stack: Rust + Axum · Minijinja (SSR) · HTMX + Alpine.js · Bulma  
Desplegado en k3s ARM64 vía ArgoCD.

---

## Desarrollo local

```bash
cp .env.example .env   # ajusta DATABASE_URL
cargo watch -x run     # http://localhost:3000
```

---

## Antes de hacer commit

```bash
cargo clippy
```

---

## Despliegue

Push a `main` → GitHub Actions compila la imagen ARM64 → ArgoCD despliega en k3s.

---

## Liberar espacio en disco

`/mnt/cargo-target` es un **tmpfs de 8 GB** (disco en RAM) usado como cache de compilación Rust.
Se llena con el tiempo al compilar varios proyectos. Síntoma:

```
error: No space left on device (os error 28)
```

**Solución rápida** — borra solo los incrementals (se regeneran solos, no afecta otros proyectos):

```bash
rm -rf /mnt/cargo-target/ubuntu/debug/incremental
```

Libera ~1.7 GB. Si sigue sin espacio, también puedes borrar deps y build:

```bash
rm -rf /mnt/cargo-target/ubuntu/debug/deps/
rm -rf /mnt/cargo-target/ubuntu/debug/build/
```

El siguiente `cargo build` tardará más pero funciona con normalidad.
