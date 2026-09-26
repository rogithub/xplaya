# xplaya

Tienda en línea de Papelería xplaya.com — `https://xplaya.com`

Stack: Rust + Axum · Minijinja (SSR) · HTMX + Alpine.js · Bulma  
Desplegado en k3s (cluster x86) vía ArgoCD; imagen amd64.

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

Push a `main` → GitHub Actions compila la imagen amd64 → ArgoCD despliega en k3s. La imagen es `latest`: tras el build, `k rollout restart deployment/xplaya -n papeleria` si el pod no la toma solo.
