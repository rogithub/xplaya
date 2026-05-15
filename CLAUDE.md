# CLAUDE.md — xplaya

Tienda en línea pública de **Papelería y Mercería El Gordo**, en **https://xplaya.com**.

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

| Ruta | Descripción | Origen en POS |
|---|---|---|
| `/recibo/{guid}` | Ticket de venta | `inventario_papeleria /Recibo/{guid}` |
| `/app/{guid}` | Monedero del cliente (cashback) | `inventario_papeleria /App/{guid}` |
| `/saldo` | Consulta de saldo | `inventario_papeleria /saldo` |
| `/terminos` | Términos y condiciones | `inventario_papeleria /Terminos` |

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

## Desarrollo local

```bash
DATABASE_URL=postgres://user:pass@host/inventario_papeleria cargo run
# http://localhost:3000 (por definir)
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
