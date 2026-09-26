# ROADMAP — xplaya

> **Cómo usar este archivo:**
> Al iniciar una sesión nueva, lee este archivo primero. Encuentra el primer ⬜ de la lista de trabajo, lee la sección de detalle correspondiente más abajo, y arranca. Al terminar un paso, cámbialo a ✅ y anota en "Notas de sesiones" qué quedó hecho.

---

## Contexto

Papelería física en Playa del Carmen. Rust + Axum + sqlx + Minijinja + HTMX + Alpine.js + Bulma.
La web pública (`xplaya.com`) está en producción: catálogo, carrito/pedidos, monedero, recibos y cotizaciones.

**Proyectos involucrados:**
- `/mnt/storage/data/code/xplaya` — este proyecto (Rust+Axum)
- `/mnt/storage/data/code/inventario_papeleria` — POS, BD PostgreSQL compartida

---

## Lista de trabajo

### Base completada ✅

- ✅ Servidor base (Axum + Minijinja + Bulma/HTMX/Alpine)
- ✅ Catálogo `/productos` con paginación y búsqueda HTMX
- ✅ Detalle `/productos/:id` con galería Alpine
- ✅ Carrito Alpine + `POST /pedidos`
- ✅ Monedero, recibos, cotizaciones, URL cortas (`/app`, `/recibo`, `/cotizacion`, `/r/:code`)
- ✅ Página `/resena`
- ✅ SEO y Open Graph (meta tags, JSON-LD, OG dinámico por página)
- ✅ Soporte kits (compuestos) en catálogo, detalle y recibo
- ✅ Presentaciones de producto (unidad/caja/paquete)
- ✅ Página `/cortinas`
- ✅ Cashback potencial en cotizaciones
- ✅ QR de validación en impresiones de recibos y cotizaciones

### Pendiente sin fecha

- ⬜ **ANALYTICS** — middleware que inserta en `Visitas`, gestión `SessionId` en cookie, excluir `/static/*`

---

## Retirado (2026-09-26)

Se retiraron el **kiosko táctil** (`/kiosko/*`), la **búsqueda semántica** (fallback con embeddings bge-m3) y las **FamiliasSemanticas** (tiles de categorías del kiosko):

- El kiosko era un piloto sin fecha: depende de mobiliario para los kioskos físicos. Las pantallas Elo se usan hoy en la venta touch del POS (`inventario_papeleria`).
- Los embeddings (`vector(1024)` por producto) triplicaban el tamaño de la BD (~10 → ~34 MB) y crecían ~4 KB por producto nuevo, para un fallback de búsqueda con muy poco tráfico.
- En la BD se quitaron `Productos.embedding`, `EmbeddingGeneratedAt`, `FamiliaSemanticaId`, la tabla `FamiliasSemanticas`, la extensión `vector` y el cliente de sistema "Kiosko en tienda" (`ID_CLIENTE_KIOSKO`). En k3s: `bge-embeddings`, el CronJob `embeddings-ingest` y el secret `papeleria-kiosko-secret`.

**Para retomarlo:** `git checkout kiosko-v1` tiene el código completo del kiosko y del fallback semántico, y este ROADMAP con el diseño detallado de cada fase (teclado en pantalla, token por cookie HttpOnly, pedidos con `Origen=0`, curación de las 37 familias). Los scripts de BD retirados están en el historial de `inventario_papeleria` (`dbchanges/`, `EMBEDDINGS_PLAN.md`).

**Lección que conviene no perder:** nunca `TRUNCATE ... CASCADE` sobre una tabla referenciada por `Productos` — en 2026-07-01 vació `Productos` y 13 tablas más en producción (se recuperó del backup de R2).
