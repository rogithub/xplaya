# REVISIONES.md — xplaya

Bitácora de cambios paso a paso. Las entradas más recientes van arriba.

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
