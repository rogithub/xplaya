# ROADMAP — xplaya

> **Cómo usar este archivo:**
> Al iniciar una sesión nueva, lee este archivo primero. Encuentra el primer ⬜ de la lista de trabajo, lee la sección de detalle correspondiente más abajo, y arranca. Al terminar un paso, cámbialo a ✅ y anota en "Notas de sesiones" qué quedó hecho.

---

## Contexto

Papelería física en Playa del Carmen. Rust + Axum + sqlx + Minijinja + HTMX + Alpine.js + Bulma.
Raspberry Pi + Elo 2270L (22" touch 1080p) en camino (~2 semanas desde 2026-06-30).
La web pública (`xplaya.com`) ya está en producción. El foco ahora es el kiosko táctil + embeddings.

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
- ✅ Definir plan del kiosko
- ✅ Analizar categorías actuales (273 entradas caóticas)
- ✅ Definir plan de categorías

### En progreso / siguiente

- ✅ **EMBEDDINGS Fase 0** — BD: `CREATE EXTENSION vector`, columnas `embedding`/`EmbeddingGeneratedAt`/`FamiliaSemanticaId`, tabla `FamiliasSemanticas`, trigger de invalidación → ver sección [Embeddings](#embeddings)
- ✅ **EMBEDDINGS Fase 1** — repo `inventario-embeddings-job`, `ingest.py`, CronJob k3s → ver [Embeddings Fase 1](#embeddings-fase-1)
- ✅ **KIOSKO Fase 1** — ruta `/kiosko`, layout táctil, query baja-venta, branding → ver [Kiosko Fase 1](#kiosko-fase-1)
- ✅ **KIOSKO Fase 2** — detalle táctil, carrito con botones +/− → ver [Kiosko Fase 2](#kiosko-fase-2)
- ⬜ **KIOSKO Fase 3** — `POST /kiosko/pedidos` con token, `Origen=0`, confirmación → ver [Kiosko Fase 3](#kiosko-fase-3)
- ⬜ **KIOSKO Fase 4** — eventos Umami → ver [Kiosko Fase 4](#kiosko-fase-4)

### Semana 2 — Categorías antes de que llegue el hardware

- ✅ **EMBEDDINGS Fase 2 COMPLETA** — clustering k=40 (bootstrap), vista de costos/margen por familia (Superset) → ver [Embeddings Fase 2](#embeddings-fase-2)
- ✅ **CATEGORIAS Fase 1 COMPLETA** — curación manual de las 2,262 productos, 37 `FamiliasSemanticas` finales, sin capa `MacroCategorias` → ver [Categorías Fase 1](#categorias-fase-1)
- ⬜ **KIOSKO Fase 5** — consulta de monedero desde el kiosko → ver [Kiosko Fase 5](#kiosko-fase-5)

### Cuando llegue el hardware

- ✅ **CATEGORIAS Fase 3** — tiles de las 37 familias en el landing del kiosko → ver [Categorías Fase 3](#categorias-fase-3) _(adelantada — era solo código, no dependía del hardware)_
- ⬜ **KIOSKO Fase 6** — configurar Raspberry Pi: Chromium kiosk mode, autostart, touch → ver [Kiosko Fase 6](#kiosko-fase-6)
- ⬜ **KIOSKO Fase 7** — deploy en k3s, SealedSecret `KIOSKO_TOKEN`, prueba end-to-end → ver [Kiosko Fase 7](#kiosko-fase-7)

### Pendiente sin fecha

- ⬜ **DEPLOY** — GitHub Actions build ARM64, manifiestos k3s, SealedSecret `DATABASE_URL`, ArgoCD
- ⬜ **ANALYTICS** — middleware que inserta en `Visitas`, gestión `SessionId` en cookie, excluir `/static/*`
- ⬜ **EMBEDDINGS Fase 4** — búsqueda semántica en xplaya + kiosko (fallback tsvector→vector) → ver [Embeddings Fase 4](#embeddings-fase-4)
- ⬜ **CATEGORIAS Fase 4** — filtros por categoría en xplaya.com _(hacerlo solo si los datos del kiosko muestran que los tiles se usan)_ → ver [Categorías Fase 4](#categorias-fase-4)
- ⬜ **DATOS** — revisar la familia "Servicios de Copiado / Impresión" en el POS: aparece tercera en los tiles del kiosko con muchos productos, o sea que gran parte de sus artículos NO están marcados `EsServicio` y son visibles/vendibles desde kiosko y web. Confirmar si es intencional o falta marcar servicios.

---

## Notas de sesiones

- **2026-07-02** — **KIOSKO Fase 2 completada**: detalle táctil (`templates/kiosko/detalle.html`, reutiliza `db::productos::detalle()`; sin SEO/compartir/videos — el kiosko no abre sitios externos) y carrito con stepper (`templates/kiosko/carrito.html`: `−` deshabilitado en cantidad 1, quitar siempre explícito con botón rojo). Teclado en pantalla generalizado: `teclado-abrir` acepta `detail { input, modo }` con layout numérico tipo pad para el teléfono; fix encontrado en pruebas — el teclado fijo tapaba el campo teléfono, ahora agrega `body.teclado-abierto` (padding inferior) y hace `scrollIntoView` del input activo. Botón flotante de carrito con badge en `kiosko/base.html`; cards del grid navegan a `/kiosko/productos/{nid}`. **Ojo:** el envío del pedido usa temporalmente `POST /pedidos` público (`Origen=EnLinea`) — cambiarlo en Fase 3 a `/kiosko/pedidos` con token y `Origen=0` (el `fetch` en `carrito.html` tiene comentario `TEMPORAL Fase 2`). Verificado end-to-end con Playwright headless (16/16 checks). Siguiente: **KIOSKO Fase 3**.
- **2026-07-01** — **Teclado en pantalla del kiosko** (`templates/kiosko/partials/teclado.html`): Chromium/Linux no trae teclado virtual — se dibujó uno propio (Alpine, mayúsculas+números, dispara eventos `input` que HTMX ve como tipeo normal, `inputmode="none"` en el buscador). Verificado end-to-end con Playwright headless. Para Fase 2/3: reutilizarlo con layout numérico en el formulario nombre/teléfono (la nota vieja de "teclado automático" era falsa, ya corregida). También: tiles de categorías movidos debajo del grid y su paginación, bajo el título "Explora por categoría".
- **2026-07-01** — **CATEGORIAS Fase 3 completada (adelantada)**: los tiles no dependían del hardware — las 37 `FamiliasSemanticas` ya estaban curadas y era puro código. `kiosko_lista()` ahora acepta `familia_id: Option<Uuid>` (una sola función, sin duplicar la query); nuevas `familias_semanticas()` y `familia_nombre()` en `src/db/kiosko.rs`; handler `GET /kiosko/categoria/{id}`; tiles como botones flex-wrap entre el buscador y el grid (solo en el landing); dentro de una categoría hay encabezado con nombre + botón "← Todas". Los templates usan `base_url` (`/kiosko` o `/kiosko/categoria/{id}`) para que búsqueda y paginación respeten el filtro. El conteo de cada tile aplica los mismos filtros de visibilidad que el grid — verificado: tile 487 = 40×12+7 páginas. Siguiente: **KIOSKO Fase 2**.
- **2026-07-01** — **KIOSKO Fase 1 completada**: `GET /kiosko` con layout táctil (sin navbar/footer, fuente 20px, targets ≥48px, `noindex`), query de baja venta en `src/db/kiosko.rs` (CTE de ventas 30 días sobre `AjustesProductos` + filtros de `v_galeria_principal`; ojo — la columna real es `Ajustes.FechaAjuste`, no `FechaCreado` como decía el borrador de la sección Kiosko Fase 1), búsqueda `unaccent+ILIKE`, paginación 12/página. `KIOSKO_TOKEN` ya está en `config.rs`/`.env.example` para Fase 3. Verificado local contra BD real: última página = más vendidos. Siguiente: **KIOSKO Fase 2** (detalle táctil + carrito con stepper).
- **2026-07-01** — **CATEGORIAS Fase 1 completada**: revisión manual del catálogo completo (2,262 productos) contra la BD real, no solo el reporte de clustering. Se descartó la capa `MacroCategorias` (6-8 tiles) de la propuesta original — el dueño prefirió 37 `FamiliasSemanticas` curadas a mano como taxonomía final del kiosko, sin agrupar. 9 familias nuevas (Monografías, Mercería, Bisutería/Joyería infantil, Escolar-Geometría y Cálculo, Trámites/Gestoría, Láminas Educativas, Maquillaje, Juegos Didácticos, Bolsos y Confección), 3 familias "Temas Escolares" disueltas por contaminación (mezclaban trámites de gobierno, monografías y temas ajenos agrupados por casualidad léxica del embedding), catch-all final "Varios de Papelería" (535 productos, antes disperso en 10 familias "mixta"). Script en `inventario_papeleria/dbchanges/2026-07-01_recategorizar_familias_semanticas.sql` (idempotente, sin `TRUNCATE`), probado en dev y aplicado en producción por el dueño. **`embeddings-cluster` CronJob retirado** (manifest borrado de `k3s-manifests/workloads/papeleria/`) — ya no debe volver a correrse, destruiría la curación manual; ver detalle en sección [Categorías Fase 1](#categorias-fase-1). También se corrigió un rollback obsoleto en `EMBEDDINGS_PLAN.md` que todavía usaba `TRUNCATE ... CASCADE` (el mismo comando del incidente de más abajo).
- **2026-07-01** — Incidente de producción en `inventario_papeleria`: `cluster.py` usaba `TRUNCATE FamiliasSemanticas CASCADE`, que en Postgres ignora el `ON DELETE SET NULL` real de la FK y trunca en cascada cualquier tabla que referencie `Productos` — vació `Productos` y 13 tablas más (ventas, compras, fotos, monedero, etc.) en producción. Recuperado restaurando el backup diario de R2 (18:00 del 2026-06-30) sin pérdida de ventas/compras reales. Fix: `cluster.py` ahora usa `DELETE FROM FamiliasSemanticas`. Clustering re-corrido (2,262 productos, cobertura completa) y las 40 familias renombradas de nuevo (11 quedaron `(mixta)` esta vez). **EMBEDDINGS Fase 2 queda completa**: se agregó la vista `v_ventas_por_familia` en `dbscripts/reportes.sql` (grano por línea de venta, costo al momento de la venta) — se consume desde **Superset**, no desde la app. Detalle completo en `inventario_papeleria/EMBEDDINGS_PLAN.md`.

_Entradas más recientes arriba._

- **2026-06-30** — EMBEDDINGS Fase 2 completada: clustering k=40 corrido con `COMMIT=1` (2,262 productos asignados a 40 `FamiliasSemanticas`), nombres legibles asignados vía SQL. 4 familias quedaron `(mixta)` — catch-all sin tema de negocio claro, revisar antes de usarlas en `MacroCategorias` o reportes. Bug corregido en `cluster.py` (identificadores entre comillas dobles no calzaban con las tablas creadas sin comillas → `relation does not exist`). Desbloquea **CATEGORIAS Fase 1**. Pendiente en `inventario_papeleria`: construir el reporte de costos/ventas/margen por familia (ver `EMBEDDINGS_PLAN.md`).
- **2026-06-30** — Fases 0 y 1 completadas. 2,262 productos con embedding al 100%. cluster.py listo, embeddings-cluster CronJob desplegado en ArgoCD. Siguiente: dry run del clustering y revisar reporte de familias semánticas.
- **2026-06-30** — Embeddings Fase 0 completada: pgvector, columnas, trigger aplicados en producción. bge-m3 movido de itzamna a k3s (kukulkan, namespace `ai`), imagen pública `ghcr.io/rogithub/bge-embeddings:latest`. Manifests en `k3s-manifests/workloads/bge-embeddings/`. Siguiente: verificar `/readyz` y arrancar Fase 1.
- **2026-06-30** — Planes de kiosko y categorías definidos. Hardware en camino desde EE.UU. PLAN_DE_DESARROLLO.md, PLAN_KIOSKO.md, CATEGORIAS_PLAN.md eliminados; todo consolidado aquí. EMBEDDINGS_PLAN.md de inventario_papeleria incorporado.

---

---

## Embeddings {#embeddings}

**Modelo:** BAAI/bge-m3 (1024 dim, multilingüe)
**Repo del job:** `inventario-embeddings-job` (nuevo, separado de `bge-embeddings`)
**Objetivo principal:** análisis de costos/ventas/márgenes por familias semánticas de productos.

**Infraestructura existente:**

| Componente | Estado |
|---|---|
| Servicio bge-m3 | ✅ Desplegado en k3s (kukulkan, namespace `ai`) — `http://bge-embeddings.ai.svc.cluster.local:8000` |
| PostgreSQL 18.3 | ✅ Disponible |
| pgvector | ✅ Extension creada en producción (Fase 0 completa) |
| ArgoCD + k3s para CronJobs | ✅ Disponible |

**Escala:** 2,256 productos. Vectores: ~9 MB. Ingest inicial: ~1 min. Búsqueda HNSW: <5ms.

### Cambios en BD (Fase 0) {#embeddings-fase-0}

Aditivos, sin downtime. Reflejar también en `Ro.Inventario.Core/dbscripts/postgresql_inventario.sql`.

```sql
CREATE EXTENSION IF NOT EXISTS vector;

ALTER TABLE Productos
    ADD COLUMN IF NOT EXISTS embedding vector(1024),
    ADD COLUMN IF NOT EXISTS EmbeddingGeneratedAt TIMESTAMP NULL;

CREATE INDEX IF NOT EXISTS idx_productos_embedding
    ON Productos USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

CREATE TABLE IF NOT EXISTS FamiliasSemanticas (
    Id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    Nombre      VARCHAR(300) NOT NULL,
    Descripcion TEXT,
    FechaCreado TIMESTAMP NOT NULL DEFAULT NOW()
);

ALTER TABLE Productos
    ADD COLUMN IF NOT EXISTS FamiliaSemanticaId UUID
    REFERENCES FamiliasSemanticas(Id) ON DELETE SET NULL;

-- Invalida embedding cuando cambia texto fuente (incluye cambios via pgAdmin/scripts)
CREATE OR REPLACE FUNCTION fn_invalidar_embedding()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.Nombre IS DISTINCT FROM NEW.Nombre OR
       OLD.UnidadMedidaId IS DISTINCT FROM NEW.UnidadMedidaId OR
       OLD.Marca IS DISTINCT FROM NEW.Marca THEN
        NEW.embedding := NULL;
        NEW.EmbeddingGeneratedAt := NULL;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_invalidar_embedding
    BEFORE UPDATE ON Productos
    FOR EACH ROW
    EXECUTE FUNCTION fn_invalidar_embedding();
```

**Verificación:**
- [ ] `SELECT COUNT(*) FROM FamiliasSemanticas;` — tabla creada (0 filas es correcto)
- [ ] `SELECT * FROM pg_extension WHERE extname = 'vector';` — extension activa
- [ ] Script de init en `Ro.Inventario.Core/dbscripts/postgresql_inventario.sql` actualizado

### Fase 1 — Repo + ingest inicial {#embeddings-fase-1}

Repo nuevo `inventario-embeddings-job` con:
- `ingest.py` — loop: `SELECT Productos WHERE embedding IS NULL LIMIT 500` → batches 64 → `POST /embed-batch` a bge-m3 → `UPDATE Productos SET embedding = $v, EmbeddingGeneratedAt = NOW()`
- `embeddings-ingest-cronjob.yaml` en `k3s-manifests/workloads/` — schedule `0 3 * * *`
- Texto por producto: `p.Nombre || COALESCE(' ' || um.Nombre, '') || COALESCE(' ' || p.Marca, '')`

**Validación antes de continuar:**
```sql
SELECT COUNT(*) FILTER (WHERE embedding IS NOT NULL) AS con_embedding,
       COUNT(*) AS total,
       ROUND(100.0 * COUNT(*) FILTER (WHERE embedding IS NOT NULL) / COUNT(*), 1) AS pct
FROM Productos;
-- Meta: 100%
```

**Prueba semántica** (embeddear "plumón" con curl → pegar vector):
```sql
SELECT Nombre, ROUND((1 - (embedding <=> '[0.12, ...]'::vector))::numeric, 3) AS sim
FROM Productos WHERE embedding IS NOT NULL
ORDER BY embedding <=> '[0.12, ...]'::vector LIMIT 10;
-- Esperar: artículos de escritura relacionados en top 10
```

### Fase 2 — Clustering + familias semánticas {#embeddings-fase-2}

`cluster.py` en el mismo repo. k inicial = 40 (~56 productos/familia con 2,256 total).

Proceso (idempotente):
1. `SELECT Id, Nombre, embedding FROM Productos WHERE embedding IS NOT NULL`
2. k-means con k configurable
3. Para cada cluster: 5 nombres más cercanos al centroide → nombre tentativo
4. `BEGIN; DELETE FROM FamiliasSemanticas; INSERT ...; UPDATE Productos SET FamiliaSemanticaId = ...; COMMIT;` (nunca `TRUNCATE ... CASCADE` — en Postgres eso ignora el `ON DELETE SET NULL` real de la FK y trunca en cascada cualquier tabla que referencie `Productos`; causó un incidente de producción el 2026-07-01, ver notas de sesión)
5. Generar reporte con 10 productos representativos por cluster
6. **Revisión manual** del propietario antes de considerar válido
7. `UPDATE FamiliasSemanticas SET Nombre = '...' WHERE Id = ...` — nombres legibles

`embeddings-cluster-job.yaml` como Job plantilla, disparar manualmente:
```bash
kubectl create job --from=job/embeddings-cluster-job embeddings-cluster-$(date +%Y%m%d)
```

**Coherencia de clusters:**
```sql
SELECT fs.Nombre, COUNT(*) AS productos,
       STRING_AGG(p.Nombre, ' | ' ORDER BY RANDOM()) FILTER (WHERE p.Nombre IS NOT NULL) AS muestra
FROM FamiliasSemanticas fs
JOIN Productos p ON p.FamiliaSemanticaId = fs.Id
GROUP BY fs.Id, fs.Nombre ORDER BY productos DESC LIMIT 20;
```

### Fase 4 — Búsqueda semántica en xplaya {#embeddings-fase-4}

_(Requiere Fase 1 completa con ≥95% cobertura)_

En `xplaya/src/db/productos.rs`:
```rust
pub async fn search_semantic(pool: &PgPool, query_vector: Vec<f32>, limit: i64) -> Result<Vec<ProductoRow>> {
    sqlx::query_as!(ProductoRow, r#"
        SELECT p.Id, p.Nombre, ...
        FROM Productos p
        WHERE p.embedding IS NOT NULL AND EsPublico = true AND EsServicio = false
        ORDER BY p.embedding <=> $1
        LIMIT $2
    "#, query_vector as _, limit).fetch_all(pool).await
}
```

En `src/routes/productos.rs`: si tsvector devuelve 0 resultados y `BGE_EMBEDDINGS_URL` está definida → llamar a bge-m3 con reqwest (timeout 2s) → `search_semantic` → filtrar `similitud > 0.5`.

Variable de entorno: `BGE_EMBEDDINGS_URL` (ej: `http://bge-embeddings.ai.svc.cluster.local:8000`). Si no está → skip silencioso.

### Arquitectura de resiliencia

Para el job Python (`inventario-embeddings-job`): solo psycopg2 + numpy + scikit-learn + httpx. Sin torch — la imagen es ligera.

Para llamadas HTTP a bge-m3 desde Rust/C#: timeout 2s, 1 retry con backoff 200ms, circuit breaker tras 5 fallos en 30s. Fail-open: si falla → búsqueda funciona con solo tsvector, sin propagar el error al frontend.

Feature flags en tabla `Settings`:
- `EMBEDDINGS_FALLBACK_ENABLED` — controla el fallback de búsqueda
- `EMBEDDINGS_SUGGEST_CATEGORIA_ENABLED` — controla auto-categorización

**Botón de pánico:**
```sql
UPDATE Settings SET Value = 'false' WHERE Key = 'EMBEDDINGS_FALLBACK_ENABLED';
```

---

---

## Kiosko {#kiosko}

Pantalla Elo 2270L + Raspberry Pi corriendo Chromium apuntando a `https://xplaya.com/kiosko`.
Reutiliza queries de productos y flujo de pedidos. Layout fullscreen sin nav/footer.
El kiosko es el laboratorio — xplaya.com se modela sobre lo que funcione aquí.

**Objetivo estratégico:** rotar inventario de baja venta. Los productos que menos se venden aparecen primero.

### Fase 1 — Layout base + catálogo táctil {#kiosko-fase-1}

**Query de baja venta** — `db::kiosko::kiosko_lista()` (no reemplaza `busqueda()`):
```sql
SELECT p.id, p.nombre, p.precio, p.unidadmedida,
       COALESCE(SUM(ap.cantidad), 0) AS ventas_recientes
FROM productos p
LEFT JOIN ajustesproductos ap ON ap.productoid = p.id
LEFT JOIN ajustes a ON a.id = ap.ajusteid
    AND a.tipoajuste = 0
    AND a.fechacreado >= NOW() - INTERVAL '30 days'
WHERE p.activo = true
GROUP BY p.id, p.nombre, p.precio, p.unidadmedida
ORDER BY ventas_recientes ASC, p.nombre ASC
```
_(Suma `ap.cantidad`, no `a.pago` — no aplica la trampa del JOIN multiplicador)_

**Archivos nuevos:**
- `templates/kiosko/base.html` — layout fullscreen: Bulma + HTMX + Alpine, sin navbar/footer, fuente base +2pt, logo `xplaya.com` pequeño arriba
- `templates/kiosko/lista.html` — grid 3 columnas, imágenes grandes, precio prominente, buscador con target ≥48px
- `templates/kiosko/partials/grid.html` — fragmento HTMX (misma mecánica que `productos/partials/`)
- `src/db/kiosko.rs` — `kiosko_lista(pool, busqueda, pagina, content_base_url)`
- `src/routes/kiosko.rs` — handler `GET /kiosko` con detección `hx-request`

**Cambios en archivos existentes:**
- `src/db/mod.rs`, `src/routes/mod.rs` — `pub mod kiosko;`
- `src/main.rs` — `.route("/kiosko", get(routes::kiosko::lista))`
- `src/config.rs` + `.env.example` — agregar `kiosko_token` (se usa en Fase 3)

**Verificación:**
- [x] `GET /kiosko` devuelve catálogo completo
- [x] Productos con menos ventas recientes aparecen primero (última página = hojas sueltas, listón, clips — los top sellers)
- [x] Búsqueda por texto funciona (reemplaza `#catalogo` vía HTMX)
- [x] Cards visualmente más grandes que en `/` (3 columnas fijas, precio `is-size-3`, fuente base 20px)
- [x] No hay navbar ni footer; logo `xplaya.com` visible
- [x] `cargo clippy` sin warnings

### Fase 2 — Carrito en modo kiosko {#kiosko-fase-2}

Reutiliza `$store.carrito` de Alpine.js (mismo localStorage). Sin conflicto — el kiosko corre en un browser dedicado con sesión propia.

**Archivos nuevos:**
- `templates/kiosko/detalle.html` — imagen grande, precio, botón "Agregar al carrito" (48px mínimo)
- `templates/kiosko/carrito.html` — stepper +/− (mejor que `<input type="number">` para táctil), total, formulario nombre/teléfono

**Cambios en existentes:**
- `src/routes/kiosko.rs` — handlers `GET /kiosko/productos/{nid}` y `GET /kiosko/carrito`
- `src/main.rs` — registrar rutas nuevas

**UX táctil:** botones `−`/`+` separados ≥48×48px; botón "Quitar" rojo explícito. **Ojo:** Chromium en Linux NO trae teclado virtual (eso es de ChromeOS) — para nombre/teléfono reutilizar `templates/kiosko/partials/teclado.html` (ya existe, hecho para la búsqueda), agregándole un layout numérico para el teléfono.

**Verificación:**
- [x] Tocar card navega al detalle
- [x] "Agregar" incrementa badge
- [x] `GET /kiosko/carrito` muestra lista correcta
- [x] Botones +/− actualizan cantidad y total en tiempo real
- [x] `cargo clippy` sin warnings

### Fase 3 — Envío de pedido al POS {#kiosko-fase-3}

**`Origen=0`** (Tienda) — el kiosko está en la tienda; el POS lo atiende igual que un pedido de mostrador. Sin migración de BD.

**Modelo del token:**
- `KIOSKO_TOKEN` en `config.rs` / variable de entorno
- `GET /kiosko/carrito` pasa el token al template como variable de contexto
- Template lo embebe en campo oculto
- `POST /kiosko/pedidos` valida con `==` en Rust (tiempo constante) — si no coincide → 403

**Archivos nuevos:**
- `templates/kiosko/confirmacion.html` — "¡Pedido recibido! El personal te atenderá en un momento." + botón "Nueva consulta" (limpia carrito, vuelve a `/kiosko`). Sin QR ni referencias a xplaya.com — observar primero, agregar después si los datos lo justifican.
- En `src/models/pedido.rs` — `KioskoPedidoRequest { nombre, telefono, items, kiosko_token }`

**Cambios en existentes:**
- `src/db/pedidos.rs` — agregar parámetro `origen: i16`; actualizar INSERT; actualizar llamado existente en `routes/carrito.rs` para pasar `1`
- `src/routes/kiosko.rs` — handler `POST /kiosko/pedidos`: validar token → normalizar teléfono → `db::pedidos::crear(..., 0)` → devolver JSON
- `src/main.rs` — `.route("/kiosko/pedidos", post(routes::kiosko::crear_pedido))`

**Verificación:**
- [ ] `POST /kiosko/pedidos` con token correcto → crea pedido con `Origen=0`
- [ ] `POST /kiosko/pedidos` con token incorrecto → 403
- [ ] El POS muestra el pedido nuevo
- [ ] Confirmación NO menciona xplaya.com
- [ ] `POST /pedidos` (ruta pública) sigue funcionando con `Origen=1`
- [ ] `cargo clippy` sin warnings

### Fase 4 — Analítica de comportamiento {#kiosko-fase-4}

Umami custom events (`window.umami?.track(nombre, props)`). El `?.` evita errores si la Pi no tiene conexión momentánea.

| Evento | Cuándo |
|--------|--------|
| `kiosko_producto_visto` | Al abrir detalle |
| `kiosko_busqueda` | Al ejecutar búsqueda (debounced) |
| `kiosko_carrito_agregado` | Al tocar "Agregar" |
| `kiosko_pedido_iniciado` | Al abrir `/kiosko/carrito` |
| `kiosko_pedido_completado` | Al recibir 200 de `/kiosko/pedidos` |
| `kiosko_reset` | Al volver a `/kiosko` desde confirmación |

El abandono se deriva de `iniciado` vs `completado` — no necesita evento propio.

**Verificación:**
- [ ] Eventos aparecen en Umami bajo `/kiosko`
- [ ] Búsqueda registra `kiosko_busqueda` con el término
- [ ] Si Umami no carga, el kiosko sigue funcionando sin errores JS

### Fase 5 — Consulta de monedero {#kiosko-fase-5}

**Opción A (recomendada):** botón flotante "Monedero" → navega a `/saldo?volver=/kiosko`. El cliente busca, ve su saldo, tiene botón "← Volver". Reutiliza código existente sin duplicar lógica.

**Cambios:**
- `templates/kiosko/base.html` — botón flotante esquina inferior izquierda → `href="/saldo?volver=/kiosko"`
- `templates/monedero/saldo.html` — si llega `?volver=`, mostrar botón "← Volver" al inicio

**Verificación:**
- [ ] Botón "Monedero" visible en el kiosko
- [ ] Navega a `/saldo`, cliente puede buscar por teléfono
- [ ] Hay forma clara de volver a `/kiosko`

### Fase 6 — Configuración Raspberry Pi {#kiosko-fase-6}

**SO:** Raspberry Pi OS Lite (64-bit) + Chromium. Sin escritorio completo — solo openbox.

`~/.config/openbox/autostart`:
```bash
xset s off; xset s noblank; xset -dpms
chromium-browser \
  --kiosk --noerrdialogs --disable-infobars --disable-pinch \
  --overscroll-history-navigation=0 --touch-events=enabled \
  --no-first-run --disable-session-crashed-bubble \
  "https://xplaya.com/kiosko" &
```

**Pantalla Elo 2270L:** driver táctil HID estándar en Linux, conectar por USB. Si la imagen sale girada, en `/boot/config.txt`: `display_rotate=0` (0=normal, 1=90°, 2=180°, 3=270°).

**Verificación:**
- [ ] Pi arranca en <60s al kiosko
- [ ] Pantalla no se apaga sola
- [ ] Touch responde correctamente
- [ ] Browser sin barra de dirección ni controles de navegación
- [ ] Si Chromium crashea, se reinicia solo (watchdog en autostart o systemd)

### Fase 7 — Pulido y deploy en producción {#kiosko-fase-7}

**Auto-reset por inactividad** (en `templates/kiosko/base.html`):
```js
let idleTimer;
function resetIdle() {
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
        Alpine.store('carrito').vaciar();
        window.location.href = '/kiosko';
    }, 3 * 60 * 1000); // 3 min
}
document.addEventListener('touchstart', resetIdle);
document.addEventListener('mousemove', resetIdle);
resetIdle();
```

**Secret en producción:** `KIOSKO_TOKEN` en SealedSecret en `k3s-manifests/workloads/papeleria/`. Nunca en texto plano.

**Verificación final:**
- [ ] Auto-reset funciona tras inactividad; carrito se limpia
- [ ] `KIOSKO_TOKEN` está en SealedSecret
- [ ] Deploy en k3s sin cambios de NodePort (mismo deployment)
- [ ] Prueba end-to-end: agregar producto → confirmar pedido → POS ve el pedido
- [ ] `cargo clippy` sin warnings

---

---

## Categorías {#categorias}

Transforma las 273 "categorías" caóticas del POS en `FamiliasSemanticas` navegables para el cliente.
Depende del pipeline de embeddings, pero ya **no** pasa por una capa `MacroCategorias` — ver decisión 2026-07-01 más abajo.

**Situación actual:** categorías del POS son etiquetas de SKU escritas a mano — duplicados, typos, granularidad de producto no de navegación. No se tocan (siguen existiendo para el POS interno); `FamiliasSemanticas` es la capa paralela que sí navega el kiosko.

**Camino crítico:**
```
Embeddings Fase 0+1 → Embeddings Fase 2 (clustering k=40, bootstrap)
                              ↓
                    Categorías Fase 1 (revisión y curación manual → 37 FamiliasSemanticas) ✅
                              ↓
                    Categorías Fase 3 (tiles en kiosko, consumen FamiliasSemanticas directo)
                              ↓
                    Categorías Fase 4 (xplaya, después)
```

**Mientras tanto, en paralelo:** Kiosko Fases 1-3 no necesitan categorías.

### Fase 1 — Curación manual de FamiliasSemanticas ✅ COMPLETADA (2026-07-01) {#categorias-fase-1}

**Decisión de diseño:** se descartó la capa `MacroCategorias` (6-8 tiles) de la propuesta original. En vez de eso, `FamiliasSemanticas` **es** la taxonomía de navegación del kiosko — sin agrupar. Motivo: el dueño conoce el catálogo a detalle (líneas de mercería, maquillaje, trámites de gobierno, bolsos hechos a mano por su esposa) y prefirió una revisión producto por producto en vez de forzar 2,262 SKUs en 6-8 cajones gruesos. El resultado son **37 tiles**, no 6-8.

El clustering k=40 (Embeddings Fase 2) sirvió como bootstrap, no como resultado final: 10 de las 40 familias salieron `(mixta)` — catch-alls sin tema de negocio — y varias de las "Temas Escolares" resultaron ser mezclas (trámites gubernamentales agrupados por nombre de estado, monografías dispersas en 4 familias distintas, maquillaje/mercería/bisutería puestos ahí por similitud léxica casual del embedding, no por negocio real).

Trabajo hecho: revisión manual completa de los 2,262 productos, contra la BD real (no solo el reporte de clustering). Resultado — 9 familias nuevas, 3 disueltas por contaminación, 2 renombradas, un catch-all honesto para lo que de verdad no tiene tema:

| Familia | Productos | Nota |
|---|---:|---|
| Varios de Papelería | 535 | Catch-all final (antes disperso en 10 familias "mixta") |
| Monografías 🆕 | 192 | Folletos de un tema escolar, antes dispersos en 4 familias |
| Servicios de Copiado / Impresión | 169 | Renombrada (ya no dice "mixta") |
| Mercería 🆕 | 48 | Agujas, botones, cierres, listones, estambre, velcro |
| Escolar — Geometría y Cálculo 🆕 | 39 | Calculadoras, compases, juegos geométricos |
| Bisutería / Joyería infantil 🆕 | 36 | Aretes, collares, anillos |
| Trámites / Gestoría 🆕 | 30 | Actas de nacimiento por estado, CURP, SAT, IMSS |
| Láminas Educativas 🆕 | 20 | Mapas y sistemas del cuerpo, formato carta |
| Maquillaje 🆕 | 18 | Labiales, rubores, sombras |
| Juegos Didácticos 🆕 | 15 | Dominó, lotería, memorama, rompecabezas |
| Bolsos y Confección 🆕 | 9 | Bolsos/mochilas/alforjas/carrieles hechos a mano |
| *(resto: 25 familias sin cambio o con limpieza menor de contaminación cruzada)* | 951 | — |

Script aplicado (idempotente, sin `TRUNCATE`): `inventario_papeleria/dbchanges/2026-07-01_recategorizar_familias_semanticas.sql`. Corrido en dev para validar y luego en producción por el dueño directamente.

**Consecuencia para el job de clustering:** `embeddings-cluster` CronJob **retirado** (manifest borrado de `k3s-manifests/workloads/papeleria/`). Ya no aplica volver a correrlo — `FamiliasSemanticas` es ahora una taxonomía curada a mano, no un resultado de clustering reproducible; re-clusterizar la destruiría. Productos nuevos quedan con `FamiliaSemanticaId = NULL` hasta que se clasifiquen a mano o se construya **EMBEDDINGS Fase 5** (`sugerir-categoría`, ver `inventario_papeleria/EMBEDDINGS_PLAN.md`) — ese es el mecanismo de crecimiento hacia adelante, no el re-clustering.

**Verificación:**
- [x] Catálogo completo (2,262 productos) revisado contra la BD real
- [x] 37 `FamiliasSemanticas` finales, ninguna `NULL`
- [x] Script de migración escrito, probado en dev, aplicado en producción
- [x] `embeddings-cluster` CronJob retirado

### Fase 3 — Tiles de navegación en kiosko {#categorias-fase-3}

**Query** (directo sobre `FamiliasSemanticas`, sin `MacroCategorias`):
```sql
SELECT fs.id, fs.nombre,
       COUNT(DISTINCT p.nid) AS total_productos
FROM familiassemanticas fs
JOIN productos p ON p.familiasemanticaid = fs.id
WHERE p.activo = true
GROUP BY fs.id, fs.nombre
ORDER BY total_productos DESC;
```
Nota: son 37 tiles, no 6-8 — el layout del kiosko debe soportar scroll/grid denso, no una franja de una sola fila. Sin columna `Icono` (no existe en `FamiliasSemanticas`); si se quiere ícono por tile, agregarlo como columna nueva en `FamiliasSemanticas` vía `dbchanges/updates.sql`, no revivir `MacroCategorias`.

**Archivos nuevos en xplaya:**
- `src/db/kiosko.rs` — agregar `familias_semanticas(pool)` y `kiosko_lista_por_familia(pool, familia_semantica_id, busqueda, pagina, content_base_url)`
- `src/routes/kiosko.rs` — handler `GET /kiosko/categoria/{id}`
- `templates/kiosko/lista.html` — franja/grid de tiles táctiles entre buscador y low-sellers
- `templates/kiosko/partials/tiles_categorias.html` — grilla de tiles (nombre + cuenta; sin ícono hasta que se agregue la columna)

UX: tap en tile → `/kiosko/categoria/{id}` → grid filtrado. Buscador permanece visible. Botón "← Todas" para volver.

**Verificación:**
- [x] Tiles aparecen en el landing del kiosko (37, orden por total de productos DESC)
- [x] Tap en tile filtra correctamente (`/kiosko/categoria/{id}`, 404 si el UUID no existe)
- [x] Número de productos por categoría es coherente (tile "Varios de Papelería" = 487 = 40 páginas × 12 + 7 del grid filtrado; el conteo usa los mismos filtros de visibilidad que el grid)
- [x] Buscador sigue funcionando dentro de una categoría (búsqueda y paginación apuntan a `base_url` de la categoría)
- [x] `cargo clippy` sin warnings

### Fase 4 — Filtros por categoría en xplaya.com {#categorias-fase-4}

_(Hacer solo después de observar el uso real de tiles en el kiosko)_

- Barra horizontal de chips/tags sobre el grid, o un `<select>` dado que son 37 opciones (no 6-8) — evaluar cuál calza mejor en `xplaya.com`
- Tap/click → HTMX recarga `#catalogo` con `?categoria={id}` (id de `FamiliaSemantica`)
- `src/db/productos.rs` — extender `busqueda()` para aceptar `categoria_id: Option<Uuid>`
- `src/routes/productos.rs` — leer `?categoria=` del query string

Si los clientes no usan los tiles y solo buscan texto → esta fase se reconsidera.
