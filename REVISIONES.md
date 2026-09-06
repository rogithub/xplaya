# REVISIONES.md — xplaya

Bitácora de cambios paso a paso. Las entradas más recientes van arriba.

---

## Página `/foto-credencial` — anuncio del kiosko carnet self-service

Landing pública que promociona tomarse la foto de credencial en casa con la herramienta
`imagina.xplaya.com/kiosko/carnet` y recogerla impresa. Enfoque honesto: credencial escolar,
gafete, título y medida libre — **no** pasaporte ni visa (callout explícito + FAQ, porque una
foto de celular sin fondo/luz controlados se rechaza en esos trámites).

**Archivos a mirar:**
- `templates/pages/foto-credencial.html` — **nuevo**. Extiende `base.html`. SEO completo:
  `{% block title %}` / `meta_description` + `keywords` / `canonical` / `meta_social` (OG + Twitter,
  reusa `og_xplaya.jpeg`). Cuatro bloques JSON-LD en `{% block head %}`: `Service`
  (serviceType, areaServed Playa del Carmen, provider Store con dirección, offers sin precio
  fijo), `HowTo` (4 pasos = misma narrativa que la sección visible), `FAQPage` (6 Q&A, la
  primera es el disclaimer de pasaporte/visa), `BreadcrumbList` (Inicio → Fotos → esta página).
  CTAs a `{{ imagina_url }}/kiosko/carnet`. Estilos propios con prefijo `.fc-` en un `<style>`
  del head (mismo patrón que `fotos.html` / `imagina.html`). Secciones: hero, cómo funciona,
  para qué credenciales sirve (+ callout amarillo "pasaporte/visa no"), bloque de la herramienta,
  tips de fondo/luz, tabla de tamaños, FAQ visible (espejo del `FAQPage`), CTA final con WhatsApp,
  enlaces cruzados a `/fotos` e `/imagina`.
- `src/routes/pages.rs` — handler `foto_credencial` (idéntico a `fotos`, sin contexto extra:
  `imagina_url` y `site_url` ya son globales de minijinja). Añadido a la lista de páginas
  estáticas de `sitemap_xml` (prioridad 0.8) y a `llms_txt` bajo "Servicios".
- `src/main.rs` — ruta `GET /foto-credencial`.
- `templates/pages/fotos.html` — 2 enlaces internos nuevos a `/foto-credencial` (en la tarjeta
  "Fotos carnet e infantil" y bajo la sección del kiosko polaroid).
- `templates/base.html` — enlace en la lista de Servicios del footer (aparece en todo el sitio) +
  el item "Fotos" del navbar se convirtió en desplegable Bulma (`navbar-item has-dropdown
  is-hoverable`): "Impresión de fotos" (`/fotos`) y "Foto para credencial en casa"
  (`/foto-credencial`). El link padre sigue yendo a `/fotos`; sin JS (Bulma abre en hover en
  desktop y muestra los sub-items indentados en el menú móvil). No agrega slots al navbar.

**Verificado (local, `./target/debug/xplaya` contra la BD dev):** `GET /foto-credencial` → 200;
`<title>`, `description`, `canonical` y `og:url` correctos; los 6 bloques JSON-LD de la página
(4 propios + Store/WebSite heredados) parsean como JSON válido; `/sitemap.xml` y `/llms.txt`
incluyen la ruta; `/fotos` renderiza los enlaces cruzados. `cargo clippy` limpio.

---

## Embeddings Fase 4 — búsqueda semántica como fallback en /productos y /kiosko

Cuando la búsqueda exacta (tsvector/ILIKE/trigram) devuelve 0 resultados, la consulta se embebe vía bge-m3 y se buscan los 12 productos más cercanos por distancia coseno contra `Productos.embedding` (umbral de similitud > 0.5, mismos filtros de visibilidad que el catálogo). "plumón rojo" ahora encuentra "MARCADOR…" aunque no compartan ni una letra. Fail-open en todos los caminos: si `BGE_EMBEDDINGS_URL` no está definida, bge-m3 no responde en 2 s, o la query vectorial falla, el usuario ve el "sin resultados" de siempre — nunca un error.

**Archivos a revisar:**
- `src/embeddings.rs` — nuevo: `embed_query()`, POST `/embed` con reqwest (ya era dependencia), timeout 2 s, devuelve `Option` y loguea `warn` en fallas.
- `src/db/productos.rs` — `busqueda_semantica()`: el vector se serializa como literal pgvector (`[0.1,…]`) y se bindea como texto con cast `::vector` (sqlx no conoce el tipo); `ORDER BY embedding <=>` + `JOIN v_inventario` con los filtros de `v_galeria_principal`. La carga batch de presentaciones se extrajo a `cargar_presentaciones()` (la usan `busqueda` y `busqueda_semantica`).
- `src/routes/productos.rs` y `src/routes/kiosko.rs` (`lista`) — el bloque de fallback con let-chains; `Paginacion::new(n, 1, 1)` = una sola página sin nav. En kiosko solo aplica en el landing — dentro de una categoría un fallback global sería confuso.
- `templates/productos/partials/grid.html` y `templates/kiosko/partials/grid.html` — aviso `{% if semantica %}` "esto es lo más parecido".
- `src/config.rs` / `.env.example` — `BGE_EMBEDDINGS_URL: Option<String>` (vacía o ausente → apagado).
- `k3s-manifests/workloads/papeleria/xplaya-deployment.yaml` — env apuntando a `http://bge-embeddings.ai.svc.cluster.local:8000`; quitar la variable es el botón de pánico.

**Verificado (local, BD dev con 2,262/2,262 embeddings + mock de bge-m3 con un embedding real de la BD):** query sin hits léxicos → aviso + marcadores ordenados por similitud en `/productos` y `/kiosko`; búsqueda exacta intacta (sin aviso, paginación normal); con bge caído → 200 y "No se encontraron productos" normal; `cargo clippy` limpio.

---

## Kiosko Fase 3 — envío de pedido al POS, sin capturar datos del cliente

Cierra el flujo del kiosko: el pedido llega al POS con `Origen=0` (Tienda) para atenderse como mostrador. Dos rediseños sobre el plan original del ROADMAP, ambos documentados en la sección [Kiosko Fase 3] con su razonamiento completo:

1. **Fricción cero.** Se eliminó el formulario nombre/teléfono (y con él la dependencia del teclado en pantalla en el carrito). El vendedor pide o verifica el teléfono al cobrar — la pantalla de venta del POS ya permite cambiar el cliente al cargar un pedido, y el cashback se genera sobre la venta, no el pedido. El pedido llega a nombre del cliente de sistema **"Kiosko en tienda"** (`Settings.ID_CLIENTE_KIOSKO`); no se usó `ClienteId NULL` porque `PedidosController` del POS truena con cliente nulo. Cero cambios de código en el POS.
2. **Cookie HttpOnly en vez de campo oculto.** El token en el HTML de una página pública habría sido visible para cualquiera. `GET /kiosko/activar?t=<token>` (la URL de autostart de la Pi) siembra la cookie `HttpOnly; SameSite=Lax; Path=/kiosko`; `POST /kiosko/pedidos` la valida. Fail-closed si `KIOSKO_TOKEN` no está configurado.

**Archivos a revisar:**
- `inventario_papeleria/dbchanges/2026-07-02_cliente_kiosko.sql` — nuevo, idempotente: cliente "Kiosko en tienda" + setting `ID_CLIENTE_KIOSKO` (también en `dbscripts/inserts.sql`). Aplicado en dev y en producción (2026-07-02).
- `k3s-manifests/workloads/papeleria/xplaya-deployment.yaml` — env `KIOSKO_TOKEN` desde el SealedSecret nuevo `papeleria-kiosko-secret` (key `KIOSKO_TOKEN`, creado por el dueño con kubeseal). Sin `optional`: si falta la key el pod no arranca — sealed y deployment se commitean juntos.
- `src/db/pedidos.rs` — refactor: `insertar_pedido()` común con parámetro `origen`; `crear()` público (origen=1) y `crear_kiosko()` (origen=0, cliente desde Settings, devuelve `None` si falta la setting → 500 con log claro).
- `src/routes/kiosko.rs` — `activar` (valida `?t=`, Set-Cookie, redirect) y `crear_pedido` (valida cookie → 403; carrito vacío → 400); `token_de_cookie()` parsea el header Cookie a mano — sin dependencia nueva.
- `src/models/pedido.rs` — `KioskoPedidoRequest { items }` (sin token en el body — viaja en la cookie) y `KioskoPedidoResponse`.
- `templates/kiosko/carrito.html` — sin formulario ni teclado; botón único "Enviar pedido al mostrador" con nota "Pagas al recogerlo en el mostrador"; el `fetch` ya no manda datos del cliente.
- `src/main.rs` — rutas nuevas; `.env.example` — comentario actualizado del token.

**Verificado (Playwright 8/8 + curl + BD dev):** visitante sin activar → 403 con mensaje claro; activar con token malo/ausente → 403; con token bueno → cookie HttpOnly + redirect; pedido creado con `Origen=0` a nombre de "Kiosko en tienda"; token ausente del HTML de todas las páginas `/kiosko*` e ilegible desde JS; `POST /pedidos` público sigue con `Origen=1`; `cargo clippy` limpio. Pendiente de humo: ver el pedido en el POS real.

---

## Kiosko Fase 2 — detalle táctil y carrito con stepper

El flujo de compra completo del kiosko: tocar una card abre el detalle, "Agregar" alimenta el carrito (mismo store de Alpine/localStorage que la web pública), y el carrito tiene steppers +/− en vez de `<input type="number">`. El envío del pedido usa **temporalmente** el `POST /pedidos` público (`Origen=EnLinea`) — en Fase 3 cambia a `POST /kiosko/pedidos` con `KIOSKO_TOKEN` y `Origen=0`; el `fetch` está marcado con un comentario `TEMPORAL Fase 2`.

**Archivos a revisar:**
- `templates/kiosko/detalle.html` — nuevo. Misma estructura que `productos/detalle.html` pero sin SEO/OG/compartir/videos (el kiosko no debe abrir sitios externos): galería Alpine con miniaturas de 96px, presentaciones con botones `is-medium`, precio `is-size-1`, botón "Agregar" `is-large`. Tras agregar aparece un atajo "Ver carrito". "Volver" usa `history.back()` para conservar búsqueda/categoría/página del grid, con fallback a `/kiosko`.
- `templates/kiosko/carrito.html` — nuevo. Una box por línea con stepper: `−` se deshabilita en cantidad 1 (quitar es siempre acción explícita con el botón rojo "Quitar"), `+` sin tope. Formulario nombre/teléfono con `inputmode="none"` y `onfocus` que abre el teclado en pantalla — texto para el nombre, pad numérico para el teléfono (`maxlength=10`).
- `templates/kiosko/partials/teclado.html` — generalizado (era exclusivo del buscador): el evento `teclado-abrir` acepta `detail { input, modo }` con `modo: 'texto' | 'numerico'`; sin detail se comporta igual que antes (búsqueda). Nuevo layout numérico tipo pad telefónico. `tecla()` respeta el `maxlength` del input destino. **Fix de UX encontrado con el test end-to-end:** el teclado fijo tapaba el campo de teléfono — al abrir agrega `body.teclado-abierto` (padding inferior de 420px) y hace `scrollIntoView` del input activo; al cerrar lo quita.
- `templates/kiosko/base.html` — botón flotante de carrito (88px, esquina inferior derecha, z-index bajo el teclado) con badge alimentado por `$store.carrito.total`; CSS nuevo (`.kiosko-card-link`, `.carrito-flotante`, `.carrito-badge`, `.teclado-numero`, `body.teclado-abierto`).
- `templates/kiosko/partials/grid.html` — cada card envuelta en `<a href="/kiosko/productos/{nid}">`.
- `src/routes/kiosko.rs` — handlers `detalle` (reutiliza `db::productos::detalle()`, 404 si no existe) y `carrito` (solo renderiza; el estado vive en localStorage).
- `src/main.rs` — rutas `/kiosko/productos/{nid}` y `/kiosko/carrito`.

**Verificado con Chromium headless (Playwright), 16/16 checks:** card → detalle → agregar (badge 1) → volver → segundo producto (badge 2) → carrito con 2 líneas → `+` actualiza cantidad y total en tiempo real ($95→$130) → `−` deshabilitado en 1 → "Quitar" elimina línea → teclado texto escribe el nombre → teléfono abre pad numérico (sin ESPACIO) → 10 dígitos completos → `maxlength` respetado → `x-model` sincronizado. Capturas visuales revisadas. `cargo clippy` limpio.

---

## Kiosko — teclado en pantalla + tiles reubicados

Dos ajustes de UX del kiosko. Primero: Chromium en Linux **no** trae teclado virtual (eso es de ChromeOS, no de Chromium a secas), así que sin esto la búsqueda por texto sería inusable en la pantalla táctil. Se dibuja un teclado propio en la página en vez de instalar uno a nivel de OS en la Pi (`onboard`/`matchbox`): teclas del tamaño que queremos, se prueba en local sin hardware, y cero configuración en la Raspberry. Segundo: los tiles de categorías se movieron debajo del grid de productos y su paginación, con encabezado "Explora por categoría".

**Archivos a revisar:**
- `templates/kiosko/partials/teclado.html` — nuevo. Componente Alpine autocontenido (markup + script en el mismo archivo). Solo mayúsculas y números — la búsqueda es case-insensitive y `unaccent` resuelve los acentos, así que no hacen falta shift ni tildes. Las filas se generan con `{% for c in "QWERTYUIOP" %}` (Minijinja itera strings por carácter). Cada tecla escribe en el input y dispara `new Event('input', {bubbles: true})` — HTMX lo ve idéntico a un teclado físico, sin tocar la mecánica de búsqueda. Abre con el evento window `teclado-abrir`; cierra con su botón.
- `templates/kiosko/lista.html` — el input gana `id="busqueda-kiosko"`, `inputmode="none"` (suprime cualquier teclado del sistema) y `onfocus` que dispara `teclado-abrir`. Tiles movidos después de `#catalogo`. Incluye el teclado en ambas vistas (landing y categoría).
- `templates/kiosko/base.html` — estilos `.teclado-kiosko` (fijo al fondo, z-index sobre el contenido, teclas 64×60px).

**Verificado con Chromium headless (Playwright):** teclado oculto al inicio → tocar el buscador lo abre → teclear L-A-P-I-Z escribe en el input → HTMX dispara `?busqueda=LAPIZ` y el grid se filtra → backspace borra → Cerrar lo oculta. Captura visual revisada.

**Pendiente relacionado (Fase 2/3):** reutilizar este teclado para el formulario nombre/teléfono del carrito, con layout numérico — el ROADMAP asumía teclado automático del navegador y se corrigió.

---

## Categorías Fase 3 — tiles de las 37 familias en el kiosko (adelantada)

Navegación por categoría en el kiosko usando las `FamiliasSemanticas` curadas a mano. Se adelantó respecto al plan ("cuando llegue el hardware") porque no tenía dependencia real del hardware: los datos ya estaban en producción y todo es código de este repo.

**Archivos a revisar:**
- `src/db/kiosko.rs` — `kiosko_lista()` gana el parámetro `familia_id: Option<Uuid>` (filtro con `EXISTS` sobre `productos.familiasemanticaid`; una sola función en vez de duplicar la query). Nuevas `familias_semanticas()` (tiles con conteo — aplica los mismos filtros de visibilidad que el grid para que el número del tile coincida con lo que se ve al filtrar) y `familia_nombre()` (para el encabezado y el 404).
- `src/models/kiosko.rs` — nuevo, struct `FamiliaTile { id, nombre, total_productos }`.
- `src/routes/kiosko.rs` — handler `categoria` (`GET /kiosko/categoria/{id}`); ambos handlers pasan `base_url` al template (`/kiosko` o `/kiosko/categoria/{id}`) para que búsqueda y paginación HTMX respeten el contexto. El landing solo consulta las familias en la carga completa, no en cada partial HTMX.
- `templates/kiosko/partials/tiles_categorias.html` — nuevo, botones flex-wrap (37 tiles, grid denso con scroll — no una franja de una fila) con nombre + tag de conteo.
- `templates/kiosko/lista.html` — tiles entre buscador y grid solo en el landing; dentro de una categoría muestra encabezado con el nombre y botón "← Todas". El `hx-get` del buscador usa `{{ base_url | safe }}`.
- `templates/kiosko/partials/grid.html` — paginación parametrizada con `base_url`.
- `templates/kiosko/base.html` — `.button` entra a la regla de targets táctiles ≥48px.
- `src/main.rs`, `src/models/mod.rs` — registro de ruta y módulo.

**Verificado local:** 37 tiles en el landing; tile "Varios de Papelería" = 487 = 40 páginas × 12 + 7 del grid filtrado; búsqueda "papel" dentro de la categoría filtra bien; la paginación del grid filtrado apunta a `/kiosko/categoria/{id}?pagina=...`. `cargo clippy` limpio.

---

## Kiosko Fase 1 — layout táctil y catálogo de baja venta

Nueva ruta `GET /kiosko` para la pantalla táctil de la tienda (Elo 2270L + Raspberry Pi). Catálogo con orden invertido al de la web pública: los productos con **menos** ventas en los últimos 30 días aparecen primero — el objetivo es rotar inventario estancado.

**Archivos a revisar:**
- `src/db/kiosko.rs` — `kiosko_lista()`: CTE `ventas` agrupa `ajustesproductos` por producto (suma `cantidad`, sin la trampa del JOIN multiplicador) y ordena `COALESCE(ventas, 0) ASC, nombre ASC`. Mismos filtros de visibilidad que `v_galeria_principal` (sin servicios, con stock o kit, precio > 0). Búsqueda simple con `unaccent + ILIKE` sobre nombre y categoría. Paginación con `COUNT(*) OVER ()`, 12 items por página (grid de 3). Nota: la columna real es `Ajustes.FechaAjuste`, no `FechaCreado` como decía el borrador del ROADMAP.
- `src/routes/kiosko.rs` — handler `lista` con detección `hx-request` (partial vs página completa), mismo patrón que `routes/productos.rs`. Reutiliza `CatalogoParams` y `ProductoCard`.
- `templates/kiosko/base.html` — layout fullscreen sin navbar/footer, logo pequeño centrado, `font-size: 20px`, targets táctiles ≥48px, `noindex`. Carga cart.js/Alpine/HTMX (el carrito se usa desde Fase 2) y Umami (Fase 4).
- `templates/kiosko/lista.html` — buscador `is-large` + `#catalogo`.
- `templates/kiosko/partials/grid.html` — grid fijo de 3 columnas (pantalla siempre 1080p horizontal), precio prominente `is-size-3`, paginación `is-large` sin `hx-push-url` (el kiosko no tiene barra de URL). Las cards aún no navegan — el detalle llega en Fase 2.
- `src/config.rs` / `.env.example` — `KIOSKO_TOKEN` (se usa hasta Fase 3, default vacío).
- `src/main.rs`, `src/db/mod.rs`, `src/routes/mod.rs` — registro de ruta y módulos.

**Verificado local:** página completa 200, partial HTMX sin `<html>`, búsqueda "lapiz" filtra bien, última página (169) trae los más vendidos (hojas sueltas, listón por metro, clips a menudeo). `cargo clippy` limpio.

---

## QR de validación en recibos y cotizaciones impresas

Las vistas `/recibo/{id}/print` y `/cotizacion/{uid}/print` ahora incluyen un código QR que apunta a la versión en vivo en xplaya.com (`{SITE_URL}/recibo/{id}` o `/cotizacion/{uid}`). Como el PDF se genera desde la vista `/print`, el QR aparece tanto en papel como en PDF sin cambio adicional. El cliente escanea y compara contra los datos vivos de la BD — un papel falsificado no puede hacer que xplaya.com muestre datos que no existen.

**Archivos a revisar:**
- `Cargo.toml` — nuevo crate `qrcode` (solo feature `svg`, sin `image`)
- `src/routes/monedero.rs` — helper `qr_svg()` que genera el SVG inline; `recibo_print` y `cotizacion_print` lo pasan al template como `qr_svg`
- `templates/monedero/recibo_print.html` y `cotizacion_print.html` — bloque `{% if qr_svg %}` con el SVG (`| safe`) y la leyenda "Escanea para validar..."

**Pendiente relacionado:** cuando el POS empiece a insertar en `ShortUrls`, el QR puede apuntar a `xplaya.com/r/{code}` para un código más pequeño (mejor para térmica de 58mm).

---

## Productos kit (compuestos) — recibo, catálogo y detalle

Soporte para los kits que agregó `inventario_papeleria` (`Productos.EsCompuesto`, tabla `ProductoComponentes`, `AjustesProductos.KitProductoId`).

**Archivos a revisar:**
- `src/db/monedero.rs` → `recibo()` — la query de líneas ahora filtra `AND ap.kitproductoid IS NULL`. Al vender un kit, el POS inserta una línea por componente con precio $0 (solo para deducir stock); sin el filtro aparecían en el ticket como productos de $0.00. Mismo comportamiento que el recibo del POS.
- `src/models/producto.rs` — nuevo struct `ComponenteKit`; campo `componentes: Vec<ComponenteKit>` en `ProductoDetalle`
- `src/db/productos.rs` → `detalle()` — lee `escompuesto` de `v_inventario` y, si es kit, consulta `productocomponentes` para listar qué incluye
- `src/db/productos.rs` → `sitemap_productos()` — `WHERE vi.stock > 0 OR vi.escompuesto = true` (los kits no tienen compras, su stock es 0)
- `templates/productos/detalle.html` — bloque "📦 Este kit incluye:" con los componentes
- `inventario_papeleria/dbscripts/reportes.sql` y `dbchanges/updates.sql` — `v_galeria_principal` ahora incluye kits: `(Stock > 0 OR EsCompuesto = true)`; sin esto los kits nunca aparecen en el catálogo público

**Sin cambios:** cotización y carrito. Los kits no se expanden en `PedidoItems` (la expansión ocurre en el POS al convertir a venta) y su precio sale de `PreciosProductos`, así que la cotización ya los muestra bien; el carrito los trata como cualquier producto.

**Requiere BD:** correr `dbscripts/reportes.sql` (recrea las vistas con `EsCompuesto`). El código de `detalle()` falla si `v_inventario` no tiene la columna `escompuesto`.

---

## Cotización: soporte de presentaciones en PedidoItems

**Archivo:** `src/db/monedero.rs` → función `cotizacion()`

La consulta de `PedidoItems` ahora hace `LEFT JOIN productopresentaciones pres ON pres.id = pi.presentacionid`. Cuando existe presentación se usa `pres.precioventa` como precio y se muestra `"Producto — Caja de 12"` como nombre. La cantidad en `PedidoItems` ya viene en unidades de presentación (el usuario seleccionó "2 cajas"), por lo que no hay factor que ajustar — a diferencia del recibo que sí divide por `factor` porque `AjustesProductos.cantidad` está en unidades base.

---

## Presentaciones de producto (unidad / caja / paquete)

Soporte para la tabla `ProductoPresentaciones` que agregó `inventario_papeleria`.

**Archivos a revisar:**
- `inventario_papeleria/dbchanges/updates.sql` — agrega `PresentacionId` a `PedidoItems` (ya estaba en `AjustesProductos`)
- `src/models/producto.rs` — nuevo struct `Presentacion`; campo `presentaciones: Vec<Presentacion>` en `ProductoDetalle`
- `src/db/productos.rs` — query a `productopresentaciones` en `detalle()`, ordenado por precio
- `src/models/pedido.rs` — campo `presentacion_id: Option<Uuid>` en `PedidoItemRequest`
- `src/db/pedidos.rs` — INSERT en `pedidoitems` ahora incluye `presentacionid`
- `static/js/cart.js` — clave de item cambiada de `nid` a `_key` (`nid` o `nid_presentacionId`); migración automática de localStorage antiguo
- `templates/productos/detalle.html` — botones de presentación con Alpine: actualiza precio y etiqueta al seleccionar; pasa `presentacion_id` al carrito
- `templates/carrito/index.html` — usa `item._key` en `quitar`/`cambiarCantidad`; envía `presentacion_id` al API

**Comportamiento:**
- Producto sin presentaciones: UI idéntica a antes.
- Producto con presentaciones: aparecen botones "Pieza — $X" / "Caja de 12 — $Y". Seleccionar uno actualiza el precio visible y lo que se agrega al carrito.
- El mismo producto puede estar en el carrito como unidad y como caja al mismo tiempo (claves distintas).

---

## Página `/cortinas` — anuncio elaboración de cortinas

**Archivos a revisar:**
- `templates/pages/cortinas.html` — página nueva con hero, galería, sección "todo incluido", paleta de colores, pasos y CTA a WhatsApp
- `src/routes/pages.rs` — handler `cortinas()` agregado al final
- `src/main.rs` — ruta `GET /cortinas` registrada
- `templates/base.html` — ítem "Cortinas" en navbar y en footer bajo Servicios
- Sitemap actualizado con `/cortinas` (priority 0.8, monthly)

**Qué hace:** Landing estática para el servicio de cortinas a medida. Muestra galería con las 6 imágenes de `static/img/cortinas/`. Destaca los 4 componentes incluidos (confección, bases, tornillería, bastón), flujo en 3 pasos, envíos a toda la república y CTAs a WhatsApp. SEO con meta description, OG tags, canonical y schema `Service`.

---

## Cotización: cashback potencial + ícono descarga

**Archivos a revisar:**
- `src/db/settings.rs` — nueva función `tipo_cambio_monedero()` que lee solo `TIPO_CAMBIO_MONEDERO` de `Settings`
- `src/models/monedero.rs` — `Cotizacion` ahora incluye `monedero_potencial` y `porcentaje_monedero`
- `src/db/monedero.rs` — `cotizacion()` calcula `total × tasa` y lo pasa al modelo
- `templates/monedero/cotizacion.html` — caja verde con cashback estimado; mensaje diferente si ya tiene o no monedero; ícono descarga `fa-file-pdf` → `fa-download`
- `templates/monedero/cotizacion_print.html` — misma caja de cashback en el PDF

**Qué hace el cambio:**

1. En `/cotizacion/:uid` se muestra para todos (tengan o no monedero) cuánto ganarían de cashback si realizan la compra, calculado con la tasa `TIPO_CAMBIO_MONEDERO` de la BD (default 2%).
2. El mensaje es contextual: clientes con monedero ven "se agregarán automáticamente al pagar"; los demás reciben la invitación a unirse en la papelería.
3. El cashback estimado también aparece en el PDF de la cotización.
4. El ícono del botón Descargar PDF cambió de `fa-file-pdf` a `fa-download` para ser consistente con el recibo.

---

## UI: OG imagen homepage, botón reseña mobile, ícono PDF en recibo

**Archivos a revisar:**
- `templates/productos/lista.html` — og:image cambiado de `og_catalogo.jpeg` a `og_xplaya.jpeg`; twitter:card a `summary_large_image`
- `templates/pages/resena.html` — botón Google: texto "Dejar mi reseña en Google" → "Dejar mi reseña" + ícono `fa-google`
- `templates/monedero/recibo.html` — ícono del botón Descargar PDF: `fa-file-pdf` → `fa-download`

**Qué hace el cambio:**
1. La imagen OG de la homepage (`/`) ahora muestra `og_xplaya.jpeg` (representativa de la papelería) en lugar de `og_catalogo.jpeg` que contenía una foto de producto.
2. El botón de reseña en `/resena` ya no se sale del fondo verde en mobile — texto más corto + ícono de Google a la izquierda.
3. El botón de descarga en `/recibo/:id` muestra una flecha de descarga limpia, sin la etiqueta "PDF" incrustada en el ícono.

---

## Monedero — filtro por AceptoPrograma

**Archivos a revisar:**
- `src/db/monedero.rs` — único archivo modificado

**Qué hace el cambio:**

La participación en el monedero electrónico es voluntaria. Los clientes con `Clientes.AceptoPrograma = false` (el default) quedan excluidos silenciosamente de todas las vistas públicas:

| Ruta | Comportamiento si AceptoPrograma = false |
|---|---|
| `/saldo` (búsqueda por teléfono) | "Número no registrado" — como si no existieran |
| `/monedero/{guid}` | 404 — el GUID no resuelve a ningún monedero |
| `/recibo/{id}` | El recibo se muestra normalmente, pero la sección de monedero desaparece (`tiene_monedero = false`, todos los montos en cero) |

**Tres cambios en `db/monedero.rs`:**
1. `buscar_cliente` — añade `AND aceptoprograma = true` al WHERE
2. `monedero` — añade `AND aceptoprograma = true` al WHERE del cliente
3. `recibo` — consulta `aceptoprograma` antes de calcular saldo/monedero; si no aceptó, todos los campos de monedero son cero y `tiene_monedero = false`

---

## Fix — Metas SEO duplicadas en templates

**Archivos a revisar:**
- `templates/base.html` — `<meta name="description">` ahora es `{% block meta_description %}`; el bloque de tags `og:image`, `og:image:width/height`, `twitter:card`, `twitter:image` ahora es `{% block meta_social %}`
- `templates/productos/detalle.html` — sobreescribe `{% block meta_description %}` y `{% block meta_social %}` en lugar de añadir duplicados en `{% block head %}`; `{% block head %}` queda solo para el JSON-LD de Product
- `templates/productos/lista.html` — sobreescribe `{% block meta_social %}` con el bloque completo (incluye `og:site_name`, `og:locale`, `twitter:card` = `summary`)
- `templates/monedero/recibo.html` — ídem, sobreescribe `meta_description` y `meta_social`
- `templates/monedero/saldo.html` — ídem

**Qué hace cada parte:**

El problema era que `base.html` declaraba tags OG y Twitter hardcodeados, y los templates de producto/recibo/saldo los volvían a declarar dentro de `{% block head %}`. El resultado era el doble de tags en el HTML final, con valores contradictorios (ej. dos `twitter:card`, dos `og:image`).

La solución es convertir las secciones que varían por página en bloques Minijinja. `{% block meta_description %}` envuelve el `<meta name="description">` del sitio; `{% block meta_social %}` envuelve todo el bloque OG/Twitter. Los templates que necesitan customización sobreescriben esos bloques completamente — el bloque de la base desaparece y no hay duplicados. Las páginas que no los sobreescriben (carrito, app, cotizacion, resena, terminos) siguen usando los defaults del sitio sin ningún cambio.

**Regla de ahora en adelante:** si una página quiere metas sociales propias, sobreescribe `{% block meta_social %}` y/o `{% block meta_description %}`. Nunca agregar tags OG/Twitter dentro de `{% block head %}`.

---

## Fase 6 — SEO y Open Graph

**Archivos a revisar:**
- `src/config.rs` — nueva variable `site_url` (default `https://xplaya.com`)
- `.env.example` — nueva entrada `SITE_URL`
- `src/main.rs` — `tmpl.add_global("site_url", Value::from_safe_string(...))` registra la variable globalmente como safe string; todos los templates la tienen sin cambios en los handlers y sin escape de slashes
- `templates/base.html` — meta tags globales: `keywords`, `author`, `theme-color #E85D04`, `og:locale es_MX`, logo como imagen OG default, Twitter Card, microdata itemprop; `Store` JSON-LD completo (dirección Playa del Carmen, geo, horarios); `WebSite` JSON-LD con `SearchAction` para caja de búsqueda en Google
- `templates/productos/detalle.html` — OG completo: título y descripción con precio + categoría (formato `$38.00 | MARCADOR`), imagen del producto desde MinIO, `og:image:width/height`; Twitter Card específico del producto; `Product` JSON-LD con `Offer`
- `templates/productos/lista.html` — OG básico del catálogo
- `templates/monedero/recibo.html` — OG con título dinámico "Tu ticket $X.XX", descripción con emoji call-to-action, imagen estática `recibo.jpg`; handler actualizado para pasar `id` (UUID) al template y poder poner `og:url` exacta
- `templates/monedero/saldo.html` — OG completo con imagen estática `saldo.jpeg`
- `static/img/` — se copiaron `logocircle.png`, `papeleria.png`, `recibo.jpg` y `saldo.jpeg` de los proyectos predecesores

**Decisiones de implementación:**
- `Value::from_safe_string` en el global de `site_url` — Minijinja escapa `/` a `&#x2f;` en auto-escape; marcarlo como safe evita el doble-escape en JSON-LD sin tocar ningún template
- `| tojson` para strings dentro de bloques `<script>` — maneja sus propias comillas, marca la salida como safe, evita conflicto con el auto-escape HTML
- Imágenes OG del recibo y saldo: estáticas por ahora (imagen ilustrativa de ticket / monedero). **Pendiente**: imagen dinámica vía Gotenberg (`GET /recibo/{id}/og`) cuando se defina el diseño

---

## Fase 4 — Monedero, recibos y URLs cortas

**Archivos a revisar:**
- `Cargo.toml` — se agregaron `chrono` y la feature `chrono` en sqlx
- `src/models/monedero.rs` — structs serializables para todas las vistas de esta fase
- `src/db/settings.rs` — lee `DIAS_VIGENCIA_MONEDERO` y `TIPO_CAMBIO_MONEDERO` de la tabla `Settings`
- `src/db/short_urls.rs` — busca un `code` en la tabla `ShortUrls` y devuelve tipo + UUID destino
- `src/db/monedero.rs` — cuatro queries: `recibo`, `cotizacion`, `monedero` y `buscar_cliente`
- `src/routes/monedero.rs` — handlers para `/terminos`, `/saldo`, `/app/:id`, `/recibo/:id`, `/cotizacion/:uid`, `/r/:code`
- `src/main.rs` — se registraron las 7 rutas nuevas
- `templates/monedero/saldo.html` — formulario de búsqueda por teléfono; errores via query param `?error=...`
- `templates/monedero/app.html` — monedero del cliente: saldo card + historial con badges de estado
- `templates/monedero/recibo.html` — ticket digital con tabla de productos, formas de pago y bloque de monedero
- `templates/monedero/cotizacion.html` — tabla de cotización con total
- `templates/pages/terminos.html` — términos completos del monedero con datos dinámicos de BD

**Qué hace cada parte:**

`chrono` es la crate estándar de Rust para fechas y horas. La agregamos porque sqlx necesita mapear columnas TIMESTAMP de PostgreSQL a un tipo Rust, y `chrono::NaiveDateTime` es el estándar para eso. Sin ella, las fechas llegarían como bytes crudos. La feature `chrono` en sqlx activa ese mapeo automático vía `#[derive(sqlx::FromRow)]`.

`db/settings.rs` lee dos claves de la tabla `Settings` con un solo `fetch_all`. Luego itera las filas y parsea cada valor: `DIAS_VIGENCIA_MONEDERO` a `i32`, `TIPO_CAMBIO_MONEDERO` a `Decimal`. Si alguna clave no existe en la BD, el código usa valores por defecto (90 días, 2%). Esto hace que el handler de `/terminos` nunca falle por datos faltantes.

`db/short_urls.rs` hace un `fetch_optional` en la tabla `ShortUrls`. Si el código no existe, devuelve `None` → el handler retorna 404. Si existe, devuelve `(tipo, targetid)` que el handler usa para construir la URL de destino y hacer el redirect 301.

`db/monedero.rs` tiene las queries más complejas de la fase:

- `recibo()`: 4 queries en paralelo lógico (secuencial en código): (1) la venta de `Ajustes`, (2) los productos de `AjustesProductos` con un `EXISTS` al view `v_ingresos_trasladados` para marcar los "Sin cashback", (3) el cashback generado sumando `MonederoGenerados`, (4) el saldo actual del cliente desde `v_ajuste_producto_monedero`. El `total` se calcula en Rust sumando todos los pagos menos el cambio — mismo algoritmo que el POS en C#.

- `cotizacion()`: usa `LATERAL` para obtener el precio más reciente de cada producto en `PreciosProductos`. `LATERAL` es un JOIN especial de PostgreSQL que permite hacer una subquery correlacionada por fila, útil cuando queremos el último precio de un producto sin duplicar filas con `DISTINCT ON` y GROUP BY.

- `monedero()`: replica la query `HistorialMonedero` del POS. Agrupa por venta (`Ajustes.Id`), suma el cashback generado y gastado, y calcula `BOOL_OR(g.devolucionid IS NOT NULL)` para saber si algún ítem de esa venta fue devuelto. También detecta entradas con saldo próximo a vencer (en los próximos 30 días) para mostrar la alerta.

- `buscar_cliente()`: busca por los últimos 10 dígitos del teléfono — mismo patrón que `db/pedidos.rs`.

`saldo_post` usa `Form<SaldoForm>` — extractor de Axum para datos de formulario HTML (`Content-Type: application/x-www-form-urlencoded`). Normaliza el teléfono igual que el POS en C#. Los errores no usan cookies (no hay TempData como en ASP.NET) sino query params: redirect a `/saldo?error=mensaje`.

Los helpers `fecha_es()` y `hora()` en `db/monedero.rs` formatean `NaiveDateTime` a texto en español. Los meses están hardcodeados — Rust no tiene localización de fechas en la stdlib, y una dependencia solo para los nombres de los meses sería excesiva.

---

## Fase 5 — Reseñas

**Archivos a revisar:**
- `src/routes/pages.rs` — handler estático para `GET /resena`
- `src/routes/mod.rs` — se agregó `pub mod pages`
- `src/main.rs` — se registró la ruta `/resena`
- `templates/pages/resena.html` — página con card centrada y link a Google Maps
- `static/img/circleai.jpg` — logo copiado desde `inventario_papeleria`

**Qué hace cada parte:**

`pages.rs` es el módulo para páginas estáticas — rutas que no necesitan consultar la BD. El handler `resena` solo obtiene el template, lo renderiza con un contexto vacío (`context!()`) y devuelve el HTML. `context!()` sin argumentos es válido en Minijinja cuando el template no usa variables de servidor.

El template extiende `base.html` y usa Bulma puro: `box`, `columns is-centered`, `is-narrow` para centrar la card sin escribir CSS propio. El link a Google Maps usa el Place ID de la papelería (`ChIJVw8jFnxDTo8RnF2hE0foXRw`) que venía hardcodeado en el componente Angular predecesor.

`static/img/` es una subcarpeta nueva — `ServeDir::new("static")` en `main.rs` sirve todo el árbol bajo `/static/`, así que `/static/img/circleai.jpg` funciona automáticamente sin cambios en la config.

---

## Fase 3 — Carrito de compras

**Archivos a revisar:**
- `src/models/pedido.rs` — structs de request y response para crear pedidos
- `src/db/pedidos.rs` — transacción: buscar/crear cliente, leer usuario anónimo, crear pedido e items
- `src/routes/carrito.rs` — dos handlers: `pagina` (GET) y `crear_pedido` (POST)
- `src/routes/mod.rs` — se agregó `pub mod carrito`
- `src/models/mod.rs` — se agregó `pub mod pedido`
- `src/main.rs` — se registraron las rutas `GET /carrito` y `POST /pedidos`
- `static/js/cart.js` — Alpine store con persistencia en localStorage
- `templates/base.html` — se cargó `cart.js` antes de Alpine; se agregó el badge reactivo en el navbar
- `templates/productos/detalle.html` — botón "Agregar al carrito" con Alpine `x-data` y `$store.carrito.agregar(p)`
- `templates/carrito/index.html` — página completa del carrito: tabla, formulario, fetch a POST /pedidos

**Qué hace cada parte:**

`models/pedido.rs` define los tipos que se deserializan del JSON que envía el navegador (`CrearPedidoRequest`, `PedidoItemRequest`) y el JSON que devuelve el servidor (`PedidoCreadoResponse`). `Deserialize` es para parsear JSON entrante; `Serialize` es para producir JSON saliente.

`db/pedidos.rs` abre una transacción con `pool.begin()`. Una transacción agrupa varias operaciones de BD: si cualquiera falla, se revierten todas automáticamente. El patrón para el cliente es: primero intentar encontrarlo (`SELECT`), y solo si no existe, insertarlo. Esto evita duplicados sin necesidad de `ON CONFLICT`. `right(telefono, 10)` recorta por la derecha los últimos 10 dígitos — maneja variantes con `+52` o sin prefijo de país. El usuario anónimo se lee de la tabla `Settings` con la clave `ID_XPLAYA.COM_ANONYMOUS_USER`; si no existe o no parsea como UUID, se usa `Uuid::nil()` (UUID de ceros).

`routes/carrito.rs` valida la entrada antes de llamar a la BD: teléfono de 10 dígitos, nombre no vacío, al menos un item. La normalización del teléfono — `chars().filter(|c| c.is_ascii_digit()).collect()` seguido de tomar los últimos 10 — es el mismo algoritmo que el POS en C# (`c.Telefono[^10..]`).

`static/js/cart.js` se carga **antes** de que Alpine inicialice. Escucha el evento `alpine:init` para registrar el store con `Alpine.store('carrito', {...})`. El store vive en `localStorage` bajo la clave `xplaya_carrito` — persiste aunque el usuario cierre la pestaña. `$store.carrito` es accesible desde cualquier componente Alpine en la página sin necesidad de `x-data` en un ancestro común.

`cart.js` se carga sin `defer` porque Alpine sí tiene `defer`. El orden es importante: cart.js define el listener `alpine:init`, luego Alpine carga, dispara el evento y el store queda registrado.

`templates/carrito/index.html` usa `<template x-if>` (no `x-show`) para los tres estados del carrito. `x-if` elimina del DOM los elementos que no aplican — útil cuando el carrito está vacío y no quieres que Alpine intente evaluar `item.nombre` sobre una lista vacía. `paginaCarrito()` es una función JavaScript local que Alpine invoca para el `x-data` de la página; retorna el estado y los métodos del formulario. `enviar()` llama a `fetch('/pedidos', { method: 'POST', body: JSON.stringify(...) })` y al éxito llama a `$store.carrito.vaciar()` para limpiar el localStorage.

El botón "Agregar al carrito" en `detalle.html` usa `| tojson` en Minijinja para escapar el nombre del producto dentro del atributo `x-data`. Esto evita que un nombre con comillas o caracteres especiales rompa el JavaScript.

---

## Fase 2 — Base de datos y catálogo

**Archivos a revisar:**
- `Cargo.toml` — se agregaron sqlx, uuid y rust_decimal
- `src/config.rs` — se agregó `database_url`
- `src/main.rs` — se agregó `PgPool` al `AppState` y la conexión a la BD; se registraron las rutas `/productos` y `/productos/{nid}`
- `src/models/producto.rs` — structs que se pasan a los templates: `ProductoCard`, `Paginacion`, `ProductoDetalle`
- `src/db/productos.rs` — las dos queries principales: catálogo paginado y detalle
- `src/routes/productos.rs` — los dos handlers: `lista` y `detalle`
- `templates/productos/lista.html` — página completa del catálogo
- `templates/productos/partials/grid.html` — fragmento HTMX: tarjetas + paginación
- `templates/productos/detalle.html` — página de detalle con galería Alpine

**Qué hace cada parte:**

`sqlx` es el driver de PostgreSQL. Diferente a un ORM: escribes SQL directo y sqlx mapea las filas a structs Rust. El compilador no puede verificar el SQL (usaríamos macros para eso, más adelante), pero sí verifica que los tipos del struct coincidan con lo que declaramos.

`#[derive(sqlx::FromRow)]` le dice a sqlx cómo convertir una fila de la BD en un struct. Mapea por nombre de columna (en minúsculas, como las devuelve PostgreSQL). `GaleriaRow` e `InventarioRow` son structs privados — solo los usa `db/productos.rs` para convertir filas crudas en los modelos limpios de `models/producto.rs`.

`fn_galeria_busqueda_paginada` es una función PostgreSQL ya existente que hace búsqueda por texto completo (FTS), ILIKE y trigramas, y devuelve los resultados paginados junto con la info de paginación en las mismas filas. Por eso cada fila tiene `total_items`, `pagina_actual` y `total_paginas` repetidos — es un patrón de PostgreSQL para devolver metadatos junto con los datos.

`fetch_all` trae todas las filas, `fetch_optional` trae una o `None`. Ambos son `async` — hay que hacerles `.await` para esperar el resultado.

El handler `lista` detecta si el request viene de HTMX revisando el header `HX-Request`. Si viene de HTMX (búsqueda o paginación), devuelve solo el partial `grid.html`. Si es una carga normal del navegador, devuelve la página completa `lista.html`. Así la búsqueda y paginación actualizan solo el área de productos sin recargar el navbar ni el footer.

`{% include "productos/partials/grid.html" %}` en Minijinja inserta el contenido del partial en el template padre. En la carga inicial, `lista.html` incluye el grid. En requests HTMX, el servidor devuelve solo el grid directamente.

`hx-push-url="true"` en los links de paginación actualiza la URL del navegador (`/productos?pagina=2`) aunque la página no se recargue completa. Esto permite que el botón "Atrás" funcione correctamente y que la URL se pueda compartir.

Los precios se formatean como strings en Rust (`format!("{:.2}", decimal)`) antes de pasarlos al template, para garantizar dos decimales sin depender del formateo de Minijinja.

El detalle usa Alpine (`x-data`) para cambiar la foto principal sin hacer un request al servidor. Minijinja renderiza las URLs de las fotos en el HTML, Alpine las lee en el browser y actualiza el `src` de la imagen al hacer click en una miniatura.

---

## Fase 1 — Servidor base

**Archivos a revisar:**
- `Cargo.toml` — dependencias del proyecto
- `src/main.rs` — punto de entrada: config, templates, router, servidor
- `src/config.rs` — variables de entorno (PORT, CONTENT_BASE_URL)
- `src/db/mod.rs`, `src/routes/mod.rs`, `src/middleware/mod.rs`, `src/models/mod.rs` — módulos vacíos, estructura lista para las siguientes fases
- `templates/base.html` — layout HTML con Bulma, HTMX y Alpine desde CDN
- `static/css/main.css` — placeholder para estilos propios
- `.env.example` — variables de entorno documentadas
- `Containerfile` — build multi-stage ARM64 para el cluster k3s

**Qué hace cada parte:**

`Cargo.toml` declara las dependencias. Axum es el framework web. Tokio es el runtime asíncrono (Rust no tiene uno por defecto — hay que elegir uno). Minijinja renderiza los templates HTML. Tower-http sirve los archivos de `static/`. Dotenvy carga el `.env` en desarrollo. Tracing registra los logs.

`AppState` es un struct con `#[derive(Clone)]` que agrupa las cosas que todos los handlers van a necesitar: el motor de templates (`tmpl`) y la config. Axum lo clona por cada request — por eso debe ser barato de clonar (clonar un `Environment` comparte el loader interno).

`main.rs` arranca en este orden: carga `.env` → lee config → inicializa Minijinja con `path_loader("templates")` (carga templates del disco en runtime) → define el router → escucha en `0.0.0.0:PORT`.

La ruta `/` devuelve un redirect 301 permanente a `/productos`. El `nest_service("/static", ServeDir::new("static"))` hace que cualquier request a `/static/*` sirva el archivo correspondiente de la carpeta `static/`.

`base.html` es el layout que todos los templates van a extender con `{% extends "base.html" %}`. El navbar usa Alpine (`x-data`, `:class`, `@click`) para el menú mobile — Alpine lee el HTML existente y le agrega reactividad sin necesidad de un bundle.

El `Containerfile` tiene dos etapas: `builder` compila el binario Rust nativamente en ARM64 (la VM de build ya es ARM64, igual que el cluster); la segunda etapa solo copia el binario compilado + `templates/` + `static/` a una imagen mínima de Debian. El truco del `echo "fn main() {}"` en la primera etapa permite que Docker cachee la compilación de dependencias — si solo cambia código propio, las dependencias no se recompilan.

<!-- Nueva entrada al completar cada paso:

## Paso N — Título
**Archivos tocados:** `src/main.rs`, `templates/base.html`, ...
**Conceptos introducidos:** Axum Router, Minijinja Environment, ...

-->
