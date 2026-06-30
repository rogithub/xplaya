# Plan de desarrollo — Kiosko de papelería

Pantalla Elo 2270L + Raspberry Pi corriendo Chromium apuntando a `https://xplaya.com/kiosko`.

El kiosko es una vista alternativa del mismo servidor Axum: reutiliza las queries de productos y el flujo de pedidos, pero con un layout fullscreen sin nav/footer, fuente y targets táctiles grandes, y un endpoint propio `/kiosko/pedidos` protegido con un token embebido en el template.

---

## Objetivos estratégicos

### 1. Rotar inventario de baja venta

El kiosko muestra primero los productos que menos se venden. El cliente los ve al llegar sin buscarlo, los descubre, y el inventario rota.

Query propia `db::kiosko::kiosko_lista()` ordenada por ventas recientes ASC. La búsqueda libre sigue disponible para quien sabe lo que busca.

### 2. El kiosko es el laboratorio, xplaya.com es la consecuencia

**El kiosko no imita a xplaya.com — xplaya.com se modela sobre lo que funcione en el kiosko.**

El kiosko tiene acceso directo al cliente físico en su ambiente real. Lo que se observa ahí (qué buscan, qué tocan, dónde se pierden, qué compran sin ayuda) es el insumo para decidir cómo evoluciona la web, no al revés. El diseño del kiosko se optimiza para el cliente de la tienda, sin importar si parece o no parece a xplaya.com.

Esto tiene una consecuencia directa en el plan: **la analítica del kiosko es obligatoria desde el principio**, no un extra de pulido. Sin datos, el laboratorio no sirve.

---

## Resumen de fases

| # | Fase | Estado |
|---|------|--------|
| 1 | Layout base + catálogo táctil | ⬜ |
| 2 | Carrito en modo kiosko | ⬜ |
| 3 | Envío de pedido al POS | ⬜ |
| 4 | Analítica de comportamiento | ⬜ |
| 5 | Consulta de monedero | ⬜ |
| 6 | Configuración Raspberry Pi | ⬜ |
| 7 | Pulido y despliegue en producción | ⬜ |

---

## Fase 1 — Layout base + catálogo táctil

**Objetivo:** `/kiosko` sirve el catálogo ordenado por baja venta, con UI optimizada para touch, sin nav/footer, con branding xplaya.com visible.

### Query de baja venta — `db::productos::kiosko_lista()`

Query nueva (no reemplaza `busqueda()`). Ordena productos activos por ventas de los últimos 30 días, de menor a mayor:

```sql
SELECT
    p.id, p.nombre, p.precio, p.unidadmedida,
    COALESCE(SUM(ap.cantidad), 0) AS ventas_recientes
FROM productos p
LEFT JOIN ajustesproductos ap ON ap.productoid = p.id
LEFT JOIN ajustes a ON a.id = ap.ajusteid
    AND a.tipoajuste = 0              -- solo ventas, no mermas ni ingresos
    AND a.fechacreado >= NOW() - INTERVAL '30 days'
WHERE p.activo = true
GROUP BY p.id, p.nombre, p.precio, p.unidadmedida
ORDER BY ventas_recientes ASC, p.nombre ASC
```

Este JOIN es seguro: se suma `ap.cantidad` (campo de `AjustesProductos`), no `a.pago` (campo de `Ajustes`). La trampa documentada en CLAUDE.md aplica solo al sumar campos monetarios de `Ajustes` cuando se une con sus líneas.

Cuando el usuario busca texto, la búsqueda libre ignora el orden por ventas y filtra por nombre (comportamiento esperado: quien sabe lo que busca lo encuentra directamente).

### Archivos nuevos

- `templates/kiosko/base.html` — layout fullscreen: `<body>` con Bulma + HTMX + Alpine, sin navbar ni footer, fuente base +2pt, sin FAB de WhatsApp; el logo `xplaya.com` fijo arriba (pequeño, no intrusivo) para que el cliente asocie la marca
- `templates/kiosko/lista.html` — extiende `kiosko/base.html`; grid 3 columnas, imágenes grandes, precio prominente, buscador grande (target mínimo 48px); texto de bienvenida tipo "Toca un producto para ver el precio o agregarlo al carrito"
- `templates/kiosko/partials/grid.html` — fragmento HTMX; misma mecánica de reemplazo que `productos/partials/grid.html`
- `src/db/kiosko.rs` — `kiosko_lista(pool, busqueda, pagina, content_base_url)` con la query de baja venta
- `src/routes/kiosko.rs` — handler `GET /kiosko` que llama `db::kiosko::kiosko_lista()`; detecta `hx-request` para responder parcial o completo

### Cambios en archivos existentes

- `src/db/mod.rs` — agregar `pub mod kiosko;`
- `src/routes/mod.rs` — agregar `pub mod kiosko;`
- `src/main.rs` — registrar `.route("/kiosko", get(routes::kiosko::lista))`
- `src/config.rs` — agregar campo `kiosko_token: String` (se usará en Fase 3)
- `.env.example` — agregar `KIOSKO_TOKEN=cambiar_en_produccion`

### Verificación

- [ ] `GET /kiosko` devuelve el catálogo completo
- [ ] Los productos con menos ventas recientes aparecen primero
- [ ] La búsqueda por texto funciona (reemplaza `#catalogo` vía HTMX)
- [ ] Las cards son visualmente más grandes que en `/`
- [ ] El logo/marca `xplaya.com` es visible en la pantalla
- [ ] No hay navbar ni footer
- [ ] `clippy` sin warnings

---

## Fase 2 — Carrito en modo kiosko

**Objetivo:** el cliente puede agregar/quitar productos y ver el resumen antes de confirmar.

El carrito reutiliza el `$store.carrito` de Alpine.js (mismo localStorage que la web). No hay conflicto porque el kiosko corre en un browser dedicado con sesión propia.

### Archivos nuevos

- `templates/kiosko/detalle.html` — vista de producto al tocar una card: imagen grande, precio, unidad de medida, botón "Agregar al carrito" (grande, verde)
- `templates/kiosko/carrito.html` — lista de items con cantidad editable (stepper +/–, mejor que input de texto para pantalla táctil), total, y formulario de nombre/teléfono

### Cambios en archivos existentes

- `src/routes/kiosko.rs` — agregar handlers:
  - `GET /kiosko/productos/{nid}` → renderiza `kiosko/detalle.html`
  - `GET /kiosko/carrito` → renderiza `kiosko/carrito.html` con `kiosko_token` en el contexto
- `src/main.rs` — registrar las rutas nuevas

### Notas de UX táctil

- Botones de cantidad: `−` y `+` separados, al menos 48×48px, en lugar de `<input type="number">` (difícil de usar sin teclado físico)
- "Quitar" producto: botón rojo explícito, no solo la ×
- El formulario de nombre/teléfono abre el teclado virtual del sistema (Chromium + Elo lo maneja automáticamente)

### Verificación

- [ ] Tocar una card navega a `/kiosko/productos/{nid}`
- [ ] "Agregar al carrito" incrementa el badge
- [ ] `GET /kiosko/carrito` muestra la lista correcta
- [ ] Los botones +/– actualizan la cantidad
- [ ] "Quitar" elimina el item
- [ ] El total se recalcula en tiempo real
- [ ] `clippy` sin warnings

---

## Fase 3 — Envío de pedido al POS

**Objetivo:** el formulario del carrito kiosko crea un `Pedido` en la BD con `Origen=0` (Tienda), validando un token secreto que solo conoce el servidor.

### Por qué `Origen=0` (Tienda) y no un valor nuevo

El kiosko está físicamente en la tienda y el staff lo atiende igual que un pedido de mostrador. No requiere migración de BD. Si en el futuro el POS necesita filtrar kiosko vs. mostrador, se agrega `Origen=2` entonces.

### Modelo del token

- `KIOSKO_TOKEN` vive en `config.rs` / variable de entorno
- El handler `GET /kiosko/carrito` pasa el token al template como variable de contexto (igual que `site_url`)
- El template lo embebe en un campo oculto: `<input type="hidden" name="kiosko_token" :value="$store.kioskoToken">` (Alpine lo toma del contexto inicial)
- El handler `POST /kiosko/pedidos` recibe el token en el body JSON, lo compara con `state.config.kiosko_token` con comparación de tiempo constante (`==` en Rust ya es seguro para strings, pero se puede usar `subtle::ConstantTimeEq` si se quiere ser riguroso)
- Si no coincide → 403

### Archivos nuevos

- `templates/kiosko/confirmacion.html` — pantalla de "¡Pedido recibido! El personal te atenderá en un momento." + sección de puente a xplaya.com (ver abajo) + botón "Nueva consulta" que limpia el carrito y vuelve a `/kiosko`
- `src/models/pedido.rs` — agregar `KioskoPedidoRequest { nombre, telefono, items, kiosko_token }`

### Cambios en archivos existentes

- `src/db/pedidos.rs` — agregar parámetro `origen: i16` a `crear()`; actualizar el INSERT para usar `$4` en lugar de literal `1`; actualizar el llamado existente en `routes/carrito.rs` para pasar `1`
- `src/routes/kiosko.rs` — agregar handler `POST /kiosko/pedidos`:
  1. Validar `kiosko_token`
  2. Normalizar teléfono
  3. Llamar `db::pedidos::crear(..., 0)`
  4. Devolver JSON `{ pedido_uid, cliente_id }` (igual que `/pedidos`)
- `src/main.rs` — registrar `.route("/kiosko/pedidos", post(routes::kiosko::crear_pedido))`

### Pantalla de confirmación

`templates/kiosko/confirmacion.html` muestra:
- "¡Pedido recibido! El personal te atenderá en un momento."
- Resumen breve (número de productos, total estimado)
- Botón grande "Nueva consulta" → limpia carrito, vuelve a `/kiosko`

Sin QR ni referencias a xplaya.com por ahora. Primero se observa qué hace el cliente en el kiosko; si hay señales de que quiere seguir comprando desde casa, se agrega ese elemento cuando haya datos que lo justifiquen.

### Verificación

- [ ] `POST /kiosko/pedidos` con token correcto → crea pedido con `Origen=0` en la BD
- [ ] `POST /kiosko/pedidos` con token incorrecto → 403
- [ ] El POS muestra el pedido nuevo
- [ ] La pantalla de confirmación NO menciona xplaya.com
- [ ] `POST /pedidos` (ruta pública) sigue funcionando con `Origen=1`
- [ ] `clippy` sin warnings

---

## Fase 4 — Analítica de comportamiento

**Objetivo:** registrar qué hace el cliente en el kiosko para tomar decisiones informadas sobre UX — tanto del kiosko mismo como, eventualmente, de xplaya.com.

### Qué medir

| Evento | Cuándo | Dato útil |
|--------|--------|-----------|
| `kiosko_producto_visto` | Al abrir el detalle de un producto | `producto_id`, `nombre` |
| `kiosko_busqueda` | Al ejecutar una búsqueda | `termino` (texto que escribió) |
| `kiosko_carrito_agregado` | Al tocar "Agregar al carrito" | `producto_id`, `nombre` |
| `kiosko_pedido_iniciado` | Al abrir `/kiosko/carrito` | — |
| `kiosko_pedido_completado` | Al recibir respuesta 200 de `/kiosko/pedidos` | `total_items` |
| `kiosko_reset` | Al volver a `/kiosko` desde confirmación o por inactividad | — |

El abandono (abrió carrito pero no completó el pedido) se deriva de `kiosko_pedido_iniciado` vs `kiosko_pedido_completado` — no necesita evento propio.

### Mecanismo: Umami custom events

El proyecto ya tiene Umami en `analytics.xplaya.com`. Umami soporta eventos custom con `window.umami.track(nombre, propiedades)`. Los eventos del kiosko se diferencian del tráfico web normal porque la URL del kiosko empieza con `/kiosko` — Umami ya los separa por path.

En `templates/kiosko/base.html` se llama a Umami igual que en `base.html`, pero además se instrumenta cada acción táctil relevante desde Alpine.js o atributos `onclick`:

```js
// al ver un producto (en el handler de navegación al detalle)
window.umami?.track('kiosko_producto_visto', { id: productoId, nombre: productoNombre });

// al buscar (en el hx-trigger del input, con debounce)
window.umami?.track('kiosko_busqueda', { termino: event.target.value });
```

El operador `?.` evita errores si Umami no carga (Pi sin conexión momentánea).

### Lo que NO se mide aquí

- Cuánto tiempo pasa el cliente mirando sin tocar — fuera de scope por ahora
- Qué productos *no* se tocan (se infiere por ausencia de `kiosko_producto_visto`)

### Verificación

- [ ] Los eventos aparecen en el dashboard de Umami bajo `/kiosko`
- [ ] Una búsqueda registra `kiosko_busqueda` con el término correcto
- [ ] Agregar al carrito registra `kiosko_carrito_agregado`
- [ ] Completar un pedido registra `kiosko_pedido_completado`
- [ ] Si Umami no carga, el kiosko sigue funcionando sin errores JS

---

## Fase 5 — Consulta de monedero

**Objetivo:** el cliente puede consultar su saldo desde el kiosko sin salir de la pantalla.

**Opción A (simple):** botón en el layout del kiosko que navega a `/saldo` (ya existe). El cliente escribe su teléfono, ve su saldo, y tiene un botón "Volver al catálogo" → `/kiosko`.

**Opción B (integrada):** panel deslizable (Alpine.js) sobre el catálogo con el formulario de saldo embebido vía HTMX. Más suave, sin salir de la pantalla del kiosko.

Se recomienda **Opción A** por ahora: reutiliza código existente sin duplicar lógica, y el flujo es claro para el cliente.

### Cambios en archivos existentes

- `templates/kiosko/base.html` — agregar botón flotante "Monedero" (esquina inferior izquierda, opuesto al carrito) → `href="/saldo?volver=/kiosko"`
- `templates/monedero/saldo.html` — si llega `?volver=`, mostrar botón "← Volver" al inicio de la página (parámetro leído vía Alpine o pasado por el handler)

### Verificación

- [ ] Botón "Monedero" visible en el kiosko
- [ ] Navega a `/saldo`, el cliente puede buscar por teléfono
- [ ] Hay forma clara de volver a `/kiosko`
- [ ] `clippy` sin warnings

---

## Fase 6 — Configuración Raspberry Pi

**Objetivo:** la Pi arranca directamente al kiosko, pantalla encendida permanentemente.

### Sistema operativo recomendado

Raspberry Pi OS Lite (64-bit) + Chromium instalado. Sin entorno de escritorio completo — solo openbox o similar para correr Chromium.

### Archivo de autostart (openbox)

`~/.config/openbox/autostart`:
```bash
xset s off
xset s noblank
xset -dpms
chromium-browser \
  --kiosk \
  --noerrdialogs \
  --disable-infobars \
  --disable-pinch \
  --overscroll-history-navigation=0 \
  --touch-events=enabled \
  --no-first-run \
  --disable-session-crashed-bubble \
  "https://xplaya.com/kiosko" &
```

### Pantalla Elo 2270L

El driver táctil ELO funciona como HID estándar en Linux — no requiere drivers especiales en Pi OS. Conectar por USB. Verificar orientación: si la imagen sale girada, agregar en `/boot/config.txt`:
```
display_rotate=0  # 0=normal, 1=90°, 2=180°, 3=270°
```

### Verificación

- [ ] Pi arranca en menos de 60s al kiosko
- [ ] Pantalla no se apaga sola
- [ ] El touch responde correctamente (calibrar si hay offset)
- [ ] El browser no muestra barra de dirección ni controles de navegación
- [ ] Si Chromium crashea, se reinicia solo (watchdog simple en autostart o systemd)

---

## Fase 7 — Pulido y despliegue en producción

**Objetivo:** experiencia sin fricciones, token seguro en el cluster.

### Auto-reset por inactividad

En `templates/kiosko/base.html`, con Alpine.js + `setTimeout`:
- Si no hay evento táctil por N minutos (sugerido: 3 min), hacer `window.location.href = '/kiosko'`
- Limpiar el carrito antes del redirect: `$store.carrito.vaciar()`

```js
// En el x-data del <body> o en un Alpine component global
let idleTimer;
function resetIdle() {
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
        Alpine.store('carrito').vaciar();
        window.location.href = '/kiosko';
    }, 3 * 60 * 1000);
}
document.addEventListener('touchstart', resetIdle);
document.addEventListener('mousemove', resetIdle);
resetIdle();
```

### Secret en producción

- `KIOSKO_TOKEN` va en un `SealedSecret` en `k3s-manifests/workloads/papeleria/`
- Mismo secret que `DATABASE_URL` del deployment de xplaya, o secret separado `xplaya-kiosko`
- **Nunca commitear el valor en texto plano**

### Verificación final

- [ ] Auto-reset funciona después de inactividad
- [ ] El carrito se limpia al hacer reset
- [ ] `KIOSKO_TOKEN` está en SealedSecret, no en el Deployment yaml en texto plano
- [ ] Deploy en k3s funciona sin cambios de NodePort (mismo deployment)
- [ ] Prueba end-to-end: cliente agrega producto → confirma pedido → el POS ve el pedido
- [ ] `clippy` sin warnings
- [ ] `cargo test` sin regresiones

---

## Dependencias entre fases

```
Fase 1 → Fase 2 → Fase 3
                         ↘
Fase 4 (independiente)    → Fase 6
Fase 5 (independiente)   ↗
```

Fases 4 y 5 pueden hacerse en paralelo con Fase 2 o 3.
