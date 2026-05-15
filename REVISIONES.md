# REVISIONES.md — xplaya

Bitácora de cambios paso a paso. Las entradas más recientes van arriba.

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
