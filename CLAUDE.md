# CLAUDE.md — xplaya

Tienda en línea pública de la papelería, en **https://xplaya.com**.

Reemplaza `papeleria-ecomerce-web` (Angular 21 SSR) + `papeleria-ecomerce-api` (ASP.NET Core). El nuevo proyecto es Rust puro: accede directamente a PostgreSQL con sqlx, sin la capa intermedia del API .NET. El frontend está en discusión — ver sección Stack.

---

## Repos relacionados

| Repo | Ruta local | Rol |
|---|---|---|
| **xplaya** | `/mnt/storage/data/code/xplaya` | Este proyecto — reemplaza web + api |
| **papeleria-ecomerce-web** | `/mnt/storage/data/code/papeleria-ecomerce-web` | Predecesor — Angular 21 SSR (referencia) |
| **papeleria-ecomerce-api** | `/mnt/storage/data/code/papeleria-ecomerce-api` | Predecesor — ASP.NET Core API (referencia) |
| **Ro.Inventario.Core** | `/mnt/storage/data/code/Ro.Inventario.Core` | Esquema y entidades — leer para entender el dominio y la BD |
| **inventario_papeleria** | `/mnt/storage/data/code/inventario_papeleria` | POS interno — comparte la misma BD PostgreSQL |
| **Ro.Inventario.Charts** | `/mnt/storage/data/code/Ro.Inventario.Charts` | Referencia directa de arquitectura Rust para este proyecto |
| **k3s-manifests** | `/mnt/storage/data/code/k3s-manifests` | GitOps — ArgoCD, deployments del cluster |

---

## Arquitectura general

```mermaid
graph TD
    Browser["Navegador"]
    Axum["xplaya\nRust + Axum"]
    DB[("PostgreSQL\ninventario_papeleria")]
    Minio["MinIO\ncntnt.xplaya.com"]
    Superset["Superset\nAnalytics"]

    Browser -->|HTTP| Axum
    Axum -->|sqlx queries| DB
    Browser -->|imágenes| Minio
    DB -->|solo lectura| Superset

    subgraph "Cluster k3s — namespace papeleria"
        Axum
    end
```

---

## Stack

**Backend**
- Rust + Axum — servidor HTTP
- sqlx — queries SQL crudas a PostgreSQL (sin ORM)
- Minijinja — templates HTML SSR

**Frontend**
- HTMX — interacciones servidor: paginación, búsqueda incremental, envío de pedidos, consulta de saldo
- Alpine.js — estado cliente: carrito en localStorage, badge del header, toggles de UI
- Bulma — CSS puro sin JS propio, cargado desde CDN
- Sin build step — los tres desde CDN en el template base

**Infra**
- PostgreSQL compartida — base `inventario_papeleria`
- Despliegue: contenedor OCI ARM64 en k3s vía ArgoCD
- Namespace: `papeleria` (mismo que el predecessor)

---

## Estructura del proyecto

```
xplaya/
├── src/
│   ├── main.rs              # Startup: config, pool, router, middleware
│   ├── config.rs            # Variables de entorno (DATABASE_URL, PORT, etc.)
│   ├── db/
│   │   ├── mod.rs           # Pool de conexiones sqlx
│   │   ├── productos.rs     # Queries de catálogo
│   │   ├── pedidos.rs       # Crear cliente + pedido
│   │   ├── monedero.rs      # Balance y historial de cashback
│   │   └── visitas.rs       # INSERT en tabla Visitas
│   ├── routes/
│   │   ├── mod.rs           # Registro de todas las rutas
│   │   ├── productos.rs     # GET /productos, GET /productos/:id
│   │   ├── carrito.rs       # GET /carrito, POST /pedidos
│   │   ├── monedero.rs      # GET /app/:guid, GET /saldo, GET /recibo/:guid
│   │   └── pages.rs         # GET /resena, GET /terminos (páginas estáticas)
│   ├── middleware/
│   │   ├── mod.rs
│   │   ├── session.rs       # Gestión de SessionId en cookie
│   │   └── analytics.rs     # Insertar visita en cada request
│   └── models/
│       ├── mod.rs
│       ├── producto.rs      # Structs de producto
│       ├── pedido.rs        # Structs de pedido / request
│       └── monedero.rs      # Structs de cashback
├── templates/
│   ├── base.html            # Layout: Bulma + HTMX + Alpine desde CDN
│   ├── productos/
│   │   ├── lista.html
│   │   ├── detalle.html
│   │   └── partials/
│   │       ├── card.html        # Fragmento HTMX — tarjeta de producto
│   │       └── paginacion.html  # Fragmento HTMX — controles de paginación
│   ├── carrito/
│   │   └── index.html
│   ├── monedero/
│   │   ├── app.html         # Monedero del cliente
│   │   ├── saldo.html       # Consulta de saldo
│   │   └── recibo.html      # Ticket de venta
│   └── pages/
│       ├── resena.html
│       └── terminos.html
├── static/
│   └── css/
│       └── main.css         # Estilos propios mínimos (todo lo demás es Bulma)
├── Containerfile            # Multi-stage, ARM64
├── .env.example
└── CLAUDE.md
```

Los fragmentos que HTMX reemplaza viven en `templates/*/partials/`.  
Un archivo por tema en `db/` y un archivo por grupo de rutas en `routes/`.

---

## Páginas a implementar

### Heredadas de `papeleria-ecomerce-web`

| Ruta | Descripción |
|---|---|
| `/` → `/productos` | Redirect |
| `/productos` | Catálogo con paginación y búsqueda |
| `/productos/:id` | Detalle de producto |
| `/carrito` | Carrito de compras — envía pedido sin auth |
| `/resena` | Página de reseñas/testimonios |

### Nuevas — vistas públicas del POS (migradas fuera del POS)

| Ruta | GUID es... | Descripción | Origen en POS |
|---|---|---|---|
| `/recibo/{guid}` | `Ajustes.Id` | Ticket de venta | `/Recibo/{id:guid}` |
| `/cotizacion/{guid}` | `Pedidos.Id` | Cotización/pedido | `/Cotizacion/{id:guid}` |
| `/app/{guid}` | `Clientes.Id` | Monedero del cliente (cashback) | `/App/{clienteId:guid}` |
| `/saldo` | — | Busca cliente por teléfono → redirige a `/app/{guid}` | `/saldo` |
| `/terminos` | — | Lee `Settings`: `DIAS_VIGENCIA_MONEDERO`, `TIPO_CAMBIO_MONEDERO` | `/Terminos` |

**Ninguna ruta requiere autenticación.** El acceso al monedero se protege solo con el GUID del cliente.

---

## Endpoints de API actuales (referencia del predecesor)

| Método | Path | Descripción |
|---|---|---|
| `GET` | `/api/productos` | Lista paginada (`pagina`, `rows`, `search?`) |
| `GET` | `/api/productos/{id}` | Detalle de producto |
| `POST` | `/api/pedidos` | Crea pedido; crea `Clientes` si el teléfono no existe |

`POST /api/pedidos` recibe `{ nombre, telefono, items[] }` → devuelve `{ pedidoId, pedidoUid, clienteId }`.  
Si el teléfono ya existe en `Clientes`, reutiliza ese registro.

---

## Base de datos

El esquema canónico está en `Ro.Inventario.Core/dbscripts/postgresql_inventario.sql`.

### Tablas clave para este proyecto

| Tabla | Descripción |
|---|---|
| `Clientes` | Clientes. `Telefono` único y requerido. `Email` único y nullable. Sin auth. |
| `Pedidos` | Cotizaciones/pedidos online. `Estatus`: 0=Nuevo, 1=Pagado, 2=Entregado. `Origen`: 0=Tienda, 1=EnLinea. |
| `Ajustes` | Ventas del POS. `TipoAjuste`: 0=Venta, 1=Merma, 2=IngresoSinCompra. |
| `AjustesProductos` | Líneas de cada venta. |
| `MonederoGenerados` | Cashback generado por venta. Tiene `FechaExpiracion`. |
| `MonederoRedimidos` | Cashback usado. |
| `v_ajuste_producto_monedero` | Vista: `DineroDigitalDisponible` y `DineroDigitalGastado` por entrada. |
| `v_ventas_monedero` | Vista: cashback agrupado por venta (`AjusteId`). |

### Enums (columnas INT)

| Columna | Valores |
|---|---|
| `Ajustes.TipoAjuste` | `0`=Venta, `1`=Merma, `2`=IngresoSinCompra |
| `Pedidos.Estatus` | `0`=Nuevo, `1`=Pagado, `2`=Entregado |
| `Pedidos.Origen` | `0`=Tienda, `1`=EnLinea |
| `Contactos.Tipo` | `0`=Cliente, `1`=Proveedor |

### Tabla ShortUrls — acortador de URLs

```sql
CREATE TABLE ShortUrls (
    Code        VARCHAR(10)  PRIMARY KEY,  -- base62 aleatorio, ej: "xK9mPq"
    Tipo        VARCHAR(20)  NOT NULL,     -- 'recibo' | 'cotizacion' | 'monedero'
    TargetId    UUID         NOT NULL,
    FechaCreado TIMESTAMP    NOT NULL DEFAULT NOW()
);
```

- URL pública: `xplaya.com/r/{code}` → redirect 301 al destino completo
- **xplaya** genera el código y hace el redirect (ruta `GET /r/:code`)
- **El POS** inserta en `ShortUrls` al crear una venta o pedido
- El POS muestra la URL completa en su UI; el botón "Copiar para cliente" usa la URL corta
- Documentar también en `Ro.Inventario.Core/dbscripts/postgresql_inventario.sql`

### Trampas críticas del dominio

**JOIN que multiplica pagos — nunca hacer esto:**
```sql
-- MAL: si el ticket tiene 8 productos, SUM(Pago) se multiplica 8 veces
SELECT SUM(a.Pago) FROM Ajustes a JOIN AjustesProductos ap ON a.Id = ap.AjusteId
```
Calcular pagos solo desde `Ajustes`; datos de líneas desde `AjustesProductos` en subqueries separados.

**Stock — nunca reconstruirlo.** Usar siempre `v_stock` o `v_inventario`.

**Ingresos trasladados:** servicios donde `PrecioCompraPromedio = PrecioVenta` no son ganancia real. Ver `v_ingresos_trasladados`.

---

## Despliegue

- Namespace k3s: `papeleria`
- NodePort: por asignar (próximo disponible en `k3s-manifests/CLAUDE.md` es **30517** — verificar antes de asignar)
- Imagen: `ghcr.io/rogithub/...` (ARM64), siempre `latest`
- Variables de entorno: `DATABASE_URL` (connection string PostgreSQL)
- Manifiestos en `k3s-manifests/workloads/papeleria/`
- Secrets vía SealedSecrets — **nunca commitear secrets en texto plano**

---

## Variables de entorno

| Variable | Requerida | Default | Descripción |
|---|---|---|---|
| `DATABASE_URL` | Sí | — | `postgres://user:pass@host/inventario_papeleria` |
| `PORT` | No | `3000` | Puerto HTTP — nunca hardcodeado, k3s asigna el NodePort externamente |
| `CONTENT_BASE_URL` | No | `https://cntnt.xplaya.com` | Base URL de imágenes en MinIO. Sin cliente MinIO — solo construcción de URL: `{CONTENT_BASE_URL}/papeleria-fotos-productos/{filename}` |

Si `CONTENT_BASE_URL` no está definida, las imágenes no se muestran — comportamiento aceptable en desarrollo local.  
Copiar `.env.example` a `.env` para desarrollo. `.env` está en `.gitignore`.

## Desarrollo local

```bash
cp .env.example .env   # completar DATABASE_URL
cargo run
# http://localhost:3000
```

---

## Modo de trabajo con AI

- **Avance real**: construir el proyecto a ritmo normal. Mezclar conceptos está bien — el objetivo es tener algo funcionando, no aislar temas.
- **Explicar al escribir**: al introducir algo nuevo (Rust, Axum, sqlx, HTMX, Alpine), explicar brevemente qué hace y por qué se usa aquí. Sin pausas formales.
- **El usuario pregunta**: no hacer preguntas de comprensión. El usuario revisa el código, pregunta cuando algo no queda claro.
- **REVISIONES.md**: actualizar después de cada commit — qué archivos mirar, qué hace el cambio. Entradas más recientes arriba.
- **PLAN_DE_DESARROLLO.md**: hoja de ruta con pasos. Marcar cada paso como completado al terminarlo.
- **El usuario dirige**: proponer opciones ante decisiones de diseño, no tomarlas solo.
- **Claridad sobre sofisticación**: no abstraer hasta que la repetición lo justifique.

---

## Convenciones

- Código en inglés (variables, funciones, módulos); textos de UI en español.
- Un archivo por "tema" en `db/` (e.g., `db/productos.rs`, `db/pedidos.rs`, `db/monedero.rs`).
- Un handler por ruta en `routes/`.
- Templates en `templates/`; fragmentos HTMX en `templates/partials/`.
- CSS/JS propio (mínimo) en `static/`; librerías externas desde CDN — no bundlear.
- Rust: código explícito sobre abstracciones elegantes — este es un proyecto de práctica, no aprendizaje, pero la claridad sigue siendo prioritaria.
- No añadir capas de abstracción que no aporten funcionalidad real.
- Images siempre `latest` (proyecto propio, un solo consumer).
