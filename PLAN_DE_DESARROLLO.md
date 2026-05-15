# Plan de Desarrollo — xplaya

Tienda en línea pública de Papelería El Gordo. Rust + Axum + sqlx + Minijinja + HTMX + Alpine.js + Bulma.
Reemplaza `papeleria-ecomerce-web` (Angular) + `papeleria-ecomerce-api` (ASP.NET).

---

## Fase 1 — Servidor base

- [ ] `cargo init`, dependencias iniciales (Axum, Minijinja, tokio)
- [ ] Estructura de carpetas: `src/`, `templates/`, `static/`
- [ ] Template base con Bulma, HTMX y Alpine.js desde CDN
- [ ] Ruta raíz `/` sirviendo HTML estático — confirmar que el servidor responde

## Fase 2 — Catálogo de productos

- [ ] Conectar PostgreSQL con sqlx (`DATABASE_URL`)
- [ ] Query de productos paginados (`db/productos.rs`)
- [ ] Ruta `GET /productos` — catálogo con paginación via HTMX
- [ ] Ruta `GET /productos/:id` — detalle de producto

## Fase 3 — Carrito de compras

- [ ] Estado del carrito en Alpine.js (localStorage): agregar, quitar, badge en header
- [ ] Ruta `GET /carrito` — vista del carrito
- [ ] Query para crear clientes y pedidos (`db/pedidos.rs`)
- [ ] `POST /pedidos` — enviar pedido al servidor

## Fase 4 — Páginas públicas del monedero

- [ ] Ruta `GET /recibo/:guid` — ticket de venta
- [ ] Ruta `GET /app/:guid` — monedero del cliente (cashback)
- [ ] Ruta `GET /saldo` — consulta de saldo por teléfono
- [ ] Ruta `GET /terminos` — términos y condiciones

## Fase 5 — Reseñas

- [ ] Ruta `GET /resena` — página de reseñas

## Fase 6 — Deploy

- [ ] `Containerfile` (ARM64, multi-stage)
- [ ] GitHub Actions: build + push a `ghcr.io`
- [ ] Manifiestos k3s en `k3s-manifests/workloads/papeleria/`
- [ ] SealedSecret para `DATABASE_URL`
- [ ] Verificar que ArgoCD despliega correctamente

---

> Marcar cada item con `[x]` al completarlo.
