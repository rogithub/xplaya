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

- ⬜ **EMBEDDINGS Fase 0** — BD: `CREATE EXTENSION vector`, columnas `embedding`/`EmbeddingGeneratedAt`/`FamiliaSemanticaId`, tabla `FamiliasSemanticas`, trigger de invalidación → ver sección [Embeddings](#embeddings)
- ⬜ **EMBEDDINGS Fase 1** — repo `inventario-embeddings-job`, `ingest.py`, CronJob k3s → ver [Embeddings Fase 1](#embeddings-fase-1)
- ⬜ **KIOSKO Fase 1** — ruta `/kiosko`, layout táctil, query baja-venta, branding → ver [Kiosko Fase 1](#kiosko-fase-1)
- ⬜ **KIOSKO Fase 2** — detalle táctil, carrito con botones +/− → ver [Kiosko Fase 2](#kiosko-fase-2)
- ⬜ **KIOSKO Fase 3** — `POST /kiosko/pedidos` con token, `Origen=0`, confirmación → ver [Kiosko Fase 3](#kiosko-fase-3)
- ⬜ **KIOSKO Fase 4** — eventos Umami → ver [Kiosko Fase 4](#kiosko-fase-4)

### Semana 2 — Categorías antes de que llegue el hardware

- ⬜ **EMBEDDINGS Fase 2** — clustering k=40, reporte de familias → ver [Embeddings Fase 2](#embeddings-fase-2)
- ⬜ **CATEGORIAS Fase 1** — revisar reporte: definir 6-8 MacroCategorias con nombre e ícono _(requiere decisión del dueño)_ → ver [Categorías Fase 1](#categorias-fase-1)
- ⬜ **CATEGORIAS Fase 2** — tabla `MacroCategorias` en BD, mapeo familias → ver [Categorías Fase 2](#categorias-fase-2)
- ⬜ **KIOSKO Fase 5** — consulta de monedero desde el kiosko → ver [Kiosko Fase 5](#kiosko-fase-5)

### Cuando llegue el hardware

- ⬜ **CATEGORIAS Fase 3** — tiles de categorías en el landing del kiosko → ver [Categorías Fase 3](#categorias-fase-3)
- ⬜ **KIOSKO Fase 6** — configurar Raspberry Pi: Chromium kiosk mode, autostart, touch → ver [Kiosko Fase 6](#kiosko-fase-6)
- ⬜ **KIOSKO Fase 7** — deploy en k3s, SealedSecret `KIOSKO_TOKEN`, prueba end-to-end → ver [Kiosko Fase 7](#kiosko-fase-7)

### Pendiente sin fecha

- ⬜ **DEPLOY** — GitHub Actions build ARM64, manifiestos k3s, SealedSecret `DATABASE_URL`, ArgoCD
- ⬜ **ANALYTICS** — middleware que inserta en `Visitas`, gestión `SessionId` en cookie, excluir `/static/*`
- ⬜ **EMBEDDINGS Fase 4** — búsqueda semántica en xplaya + kiosko (fallback tsvector→vector) → ver [Embeddings Fase 4](#embeddings-fase-4)
- ⬜ **CATEGORIAS Fase 4** — filtros por categoría en xplaya.com _(hacerlo solo si los datos del kiosko muestran que los tiles se usan)_ → ver [Categorías Fase 4](#categorias-fase-4)

---

## Notas de sesiones

_Entradas más recientes arriba._

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
| Servicio bge-m3 en itzamna (Oracle VM ARM64, Tailscale) | ✅ Desplegado, endpoints `/embed` y `/embed-batch` funcionando |
| PostgreSQL 18.3 | ✅ Disponible |
| pgvector | ✅ Instalado en cluster — solo falta `CREATE EXTENSION` |
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
4. `BEGIN; TRUNCATE FamiliasSemanticas CASCADE; INSERT ...; UPDATE Productos SET FamiliaSemanticaId = ...; COMMIT;`
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

Variable de entorno: `BGE_EMBEDDINGS_URL` (ej: `http://bge-embeddings.papeleria.svc.cluster.local`). Si no está → skip silencioso.

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
- [ ] `GET /kiosko` devuelve catálogo completo
- [ ] Productos con menos ventas recientes aparecen primero
- [ ] Búsqueda por texto funciona (reemplaza `#catalogo` vía HTMX)
- [ ] Cards visualmente más grandes que en `/`
- [ ] No hay navbar ni footer; logo `xplaya.com` visible
- [ ] `cargo clippy` sin warnings

### Fase 2 — Carrito en modo kiosko {#kiosko-fase-2}

Reutiliza `$store.carrito` de Alpine.js (mismo localStorage). Sin conflicto — el kiosko corre en un browser dedicado con sesión propia.

**Archivos nuevos:**
- `templates/kiosko/detalle.html` — imagen grande, precio, botón "Agregar al carrito" (48px mínimo)
- `templates/kiosko/carrito.html` — stepper +/− (mejor que `<input type="number">` para táctil), total, formulario nombre/teléfono

**Cambios en existentes:**
- `src/routes/kiosko.rs` — handlers `GET /kiosko/productos/{nid}` y `GET /kiosko/carrito`
- `src/main.rs` — registrar rutas nuevas

**UX táctil:** botones `−`/`+` separados ≥48×48px; botón "Quitar" rojo explícito; el formulario abre teclado virtual automáticamente (Chromium + Elo).

**Verificación:**
- [ ] Tocar card navega al detalle
- [ ] "Agregar" incrementa badge
- [ ] `GET /kiosko/carrito` muestra lista correcta
- [ ] Botones +/− actualizan cantidad y total en tiempo real
- [ ] `cargo clippy` sin warnings

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

Transforma las 273 "categorías" caóticas del POS en 6-8 `MacroCategorias` navegables para el cliente.
Depende de `FamiliasSemanticas` generadas por el pipeline de embeddings.

**Situación actual:** categorías del POS son etiquetas de SKU escritas a mano — duplicados, typos, granularidad de producto no de navegación. No se tocan (siguen existiendo para el POS interno); este plan crea una capa paralela.

**Camino crítico:**
```
Embeddings Fase 0+1 → Embeddings Fase 2 (FamiliasSemanticas)
                              ↓
                    Categorías Fase 1 (revisión humana → definir MacroCategorias)
                              ↓
                    Categorías Fase 2 (BD: tabla + mapeo)
                              ↓
                    Categorías Fase 3 (tiles en kiosko)    → Categorías Fase 4 (xplaya, después)
```

**Mientras tanto, en paralelo:** Kiosko Fases 1-3 no necesitan categorías.

### Fase 1 — Revisión del clustering y diseño de macro-categorías {#categorias-fase-1}

Tras correr **Embeddings Fase 2**, el reporte tiene esta forma:
```
Familia 1 (87 productos): CUADERNO PROFESIONAL 100H | LIBRETA PASTA DURA | BLOCK PROFESIONAL ...
Familia 2 (64 productos): PLUMA PUNTO FINO AZUL | BOLIGRAFO BIC | LAPICERO GEL ...
...
```

Pregunta por familia: **¿un cliente del kiosko tocaría este tile buscando estos productos?**

Propuesta inicial (las ~40 familias colapsan en 6-8):

| Macro-categoría | Familias semánticas esperadas |
|---|---|
| ✏️ Escritura | plumas, lápices, marcadores, plumones, colores, gomas, sacapuntas |
| 📄 Papel | cuadernos, libretas, hojas, cartulinas, blocks, monografías |
| 📐 Escolar | juegos geométricos, mapas, calculadoras, reglas, compases |
| 📁 Oficina | folders, clips, engrapadoras, perforadoras, cinta, pegamento, tijeras |
| 🎨 Arte | foamy, pintura, pinceles, plastilina, diamantina, washi tape |
| 🧵 Mercería | hilos, cierres, listones, broches, agujas, bisutería |
| 🎈 Fiestas | globos, moños, bolsas de regalo, confetti |
| 🖨️ Servicios | copias, impresiones, trámites, engargolado, enmicado |

Los nombres e íconos finales los define la revisión del clustering real.

**Verificación:**
- [ ] Reporte de clustering generado y revisado
- [ ] 6-8 macro-categorías definidas con nombre e ícono (Font Awesome)
- [ ] Cada `FamiliaSemantica` asignada a una `MacroCategoria`

### Fase 2 — Esquema en BD + mapeo {#categorias-fase-2}

```sql
CREATE TABLE IF NOT EXISTS MacroCategorias (
    Id      UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    Nombre  VARCHAR(100) NOT NULL,
    Icono   VARCHAR(50)  NOT NULL,   -- clase Font Awesome: "fa-pen", "fa-file"...
    Orden   SMALLINT     NOT NULL DEFAULT 0
);

ALTER TABLE FamiliasSemanticas
    ADD COLUMN IF NOT EXISTS MacroCategoriaId UUID
    REFERENCES MacroCategorias(Id) ON DELETE SET NULL;
-- Cadena: Productos.FamiliaSemanticaId → FamiliasSemanticas.MacroCategoriaId → MacroCategorias
```

Inserts con UUIDs fijos (ajustar nombres e íconos tras Fase 1):
```sql
INSERT INTO MacroCategorias (Id, Nombre, Icono, Orden) VALUES
    (gen_random_uuid(), 'Escritura', 'fa-pen',      1),
    (gen_random_uuid(), 'Papel',     'fa-file',     2),
    (gen_random_uuid(), 'Escolar',   'fa-ruler',    3),
    (gen_random_uuid(), 'Oficina',   'fa-folder',   4),
    (gen_random_uuid(), 'Arte',      'fa-palette',  5),
    (gen_random_uuid(), 'Mercería',  'fa-scissors', 6),
    (gen_random_uuid(), 'Fiestas',   'fa-gift',     7),
    (gen_random_uuid(), 'Servicios', 'fa-print',    8);
```

Reflejar en `Ro.Inventario.Core/dbscripts/postgresql_inventario.sql`.

**Verificación:**
- [ ] `MacroCategorias` creada con 6-8 filas
- [ ] Columna `MacroCategoriaId` en `FamiliasSemanticas`
- [ ] Todas las familias mapean a una macro-categoría (ningún NULL)
- [ ] Script de init actualizado

### Fase 3 — Tiles de navegación en kiosko {#categorias-fase-3}

**Query:**
```sql
SELECT mc.id, mc.nombre, mc.icono, mc.orden,
       COUNT(DISTINCT p.nid) AS total_productos
FROM macrocategorias mc
JOIN familiassemanticas fs ON fs.macrocategoriaid = mc.id
JOIN productos p ON p.familiaSemanticaid = fs.id
WHERE p.activo = true
GROUP BY mc.id, mc.nombre, mc.icono, mc.orden
ORDER BY mc.orden;
```

**Archivos nuevos en xplaya:**
- `src/db/kiosko.rs` — agregar `macro_categorias(pool)` y `kiosko_lista_por_categoria(pool, macro_categoria_id, busqueda, pagina, content_base_url)`
- `src/routes/kiosko.rs` — handler `GET /kiosko/categoria/{id}`
- `templates/kiosko/lista.html` — franja de tiles táctiles entre buscador y low-sellers
- `templates/kiosko/partials/tiles_categorias.html` — grilla de tiles (ícono grande + nombre + cuenta)

UX: tap en tile → `/kiosko/categoria/{id}` → grid filtrado. Buscador permanece visible. Botón "← Todas" para volver.

**Verificación:**
- [ ] Tiles aparecen en el landing del kiosko
- [ ] Tap en tile filtra correctamente
- [ ] Número de productos por categoría es coherente
- [ ] Buscador sigue funcionando dentro de una categoría
- [ ] `cargo clippy` sin warnings

### Fase 4 — Filtros por categoría en xplaya.com {#categorias-fase-4}

_(Hacer solo después de observar el uso real de tiles en el kiosko)_

- Barra horizontal de chips/tags sobre el grid: "Todos | Escritura | Papel | ..."
- Tap/click → HTMX recarga `#catalogo` con `?categoria={id}`
- `src/db/productos.rs` — extender `busqueda()` para aceptar `categoria_id: Option<Uuid>`
- `src/routes/productos.rs` — leer `?categoria=` del query string

Si los clientes no usan los tiles y solo buscan texto → esta fase se reconsidera.
