# ENVIOS.md — envíos a domicilio

Estado (28 nov 2026): **etapa 1 hecha en el POS**; en xplaya no hay nada todavía.
Decisión del POS: `inventario_papeleria/docs/decisiones/2026-11-27-envios-a-domicilio.md`.

---

## Reglas del negocio

- Los envíos los hace un proveedor de viajes (hoy Didi Entrega, pedido con la cuenta de la tienda).
  `api_paqueteria` está en pausa: primero hay que probar que los clientes lejanos sí pagan el envío.
- El cliente paga **tres renglones separados, nunca sumados**:
  | Renglón | Precio | Notas |
  |---|---|---|
  | Paquetería | Lo que cobra el proveedor | Ingreso trasladado: sin margen ni monedero |
  | Comisión de envío | $5 (precio de catálogo) | Genera monedero |
  | Empaque | $5 sugerido, **precio libre** | Un pedido grande se negocia; genera monedero |
- **El cliente nunca ve el nombre del proveedor.** El proveedor solo vive en `Envios.Proveedor`
  (y `PROVEEDOR_ENVIO` en `Settings`). Ni el esquema ni el código dicen "Didi".
- **Se paga por adelantado** (transferencia). El servicio solo se pide cuando los abonos cubren el
  pedido completo.
- El costo no se redondea. Si el proveedor cobra otro monto al pedirlo, la caja elige quién paga la
  diferencia: el cliente (el renglón cambia y ve su total nuevo) o la tienda (el renglón se queda).
- Referencia: $68 desde la tienda hasta Calle 28 con 30, en el centro (unos 8.6 km).

## Lo que ya hace el POS (etapa 1)

- Tabla `Envios`, una fila por pedido (`PedidoUid` único). Sobrevive al pedido: cuando el pedido se
  borra al entregarse, `PedidoUid` queda en NULL.
- Columnas que interesan a xplaya: `Latitud`, `Longitud`, `Referencias`, `NombreRecibe`,
  `TelefonoRecibe`, `FechaUbicacion`, `FechaCotizado`, `FechaSolicitado`, `FechaSalio`,
  `FechaEntregado`, `FechaCancelado`. Todas las fechas están en hora de la tienda.
- **El estado sale de las fechas** (`Envio.GetEstado`):
  `SinUbicacion → PorCotizar → Cotizado → ListoParaSolicitar (abonos cubren) → Solicitado → Salio → Entregado`, o `Cancelado`.
- Todo se hace en el diálogo "🚚 Envío" de Pedidos/Mes. La caja pega la ubicación que le manda el
  cliente por WhatsApp (coordenadas o enlace de Maps), cotiza, solicita y marca cuándo salió y cuándo
  se entregó.
- Al cotizar, el POS escribe los tres renglones en `PedidoItems`, con `Descripcion` = "Paquetería" y
  "Empaque". La comisión sale con el nombre del producto ("COMISIÓN DE ENVÍO").
- Los abonos (`PedidoAbonos`) son solo una nota: no entran a la caja.
- Un pedido con envío pendiente no pregunta "¿ya se lo llevó?" ni manda "ya puedes recogerlo".

**Lo que el POS dejó pendiente para xplaya:** la ubicación desde la cotización, mostrar los
renglones sin el proveedor, el avance del pedido, el aviso en la caja cuando llega la ubicación
("se hace junto con xplaya, que define cómo la guarda") y la etapa 2 (enlace de seguimiento).

## Lo que ya existe en xplaya y sirve

- `xplaya.com/cotizacion/{uid}` lee los renglones del pedido en vivo
  (`COALESCE(pi.descripcion, p.nombre)`). **Ya muestra los tres renglones sin decir el proveedor**:
  no hay que cambiar nada para eso.
- En la app, el carrito (`POST /tienda/pedido`) crea el pedido y su tarjeta de cotización aparece
  en el chat. La caja también liga pedidos al chat (tarjetas `cotizacion` con `EncargoId`).
- Las notificaciones Web Push le llegan al cliente por NATS (`NOTIFICACIONES`, tipo
  `pedido_listo`), y la papelería recibe avisos en su celular (`avisos.pedidos` → ntfy).
- `/cotizacion/{uid}` se sirve por https (xplaya.com), así que puede pedir la ubicación del
  celular (`navigator.geolocation`). La caja no la necesita: solo lee la fila.

---

## Cómo puede participar xplaya (propuesta por etapas)

### Etapa A — el cliente manda su ubicación (quita el paso de WhatsApp)

1. En `/cotizacion/{uid}` (y por lo tanto en la tarjeta del chat de la app), un botón
   **"Quiero que me lo envíen"** con:
   - "Usar mi ubicación" (geolocalización; si la niega, que pueda pegar un enlace de Maps)
   - Referencias (portón, depto…), quién recibe y su teléfono, con los datos del cliente ya
     escritos.
2. xplaya guarda eso en `Envios`: crea la fila si no existe, con `Proveedor` = `PROVEEDOR_ENVIO` y
   `FechaUbicacion`. Tiene que respetar las mismas reglas que `Envio.SetUbicacion`: no cambia nada
   si el envío ya salió, se entregó o se canceló, y las referencias no pasan de 500 caracteres.
3. Le avisa a la caja (ver la decisión 2).
4. La cotización muestra "Ubicación recibida, te mandamos el costo del envío".

### Etapa B — el cliente ve en qué va su envío

En la cotización (y en su tarjeta del chat), una línea de avance que sale de las mismas fechas:

| Estado del POS | Lo que ve el cliente |
|---|---|
| PorCotizar | Estamos cotizando tu envío |
| Cotizado | Envío cotizado: transfiere $X para que lo pidamos |
| ListoParaSolicitar | Pago recibido, en breve pedimos tu envío |
| Solicitado | Estamos preparando tu paquete |
| Salio | Tu pedido va en camino |
| Entregado | Entregado |

- Para "transfiere $X" y "Pago recibido" hace falta leer `PedidoAbonos` (cubierto / falta).
- Avisos push: "tu envío ya tiene costo" y "va en camino". Hoy solo existe `pedido_listo`.

### Etapa C — seguimiento en vivo (la etapa 2 del POS)

El enlace de seguimiento del proveedor, visible en la cotización mientras el envío va en camino.
Necesita una columna nueva en `Envios`, que se agrega en el repo del POS.

### Fuera de alcance por ahora

- Cotizar el costo automáticamente (lo escribe la caja a mano; no hay API).
- Cobro en línea (ver `cobro-en-linea-descartado`: los anticipos son por transferencia).
- Una pantalla "Mis pedidos" en la app (el chat ya hace ese papel).

---

## Decisiones (las toma el dueño)

1. ✅ **Dónde se pide el envío (decidido el 28 nov 2026): solo en la cotización**
   (`/cotizacion/{uid}`), después de armar el pedido. Funciona igual si la cotización llegó por
   WhatsApp, por el chat de la app o la armó la caja. El carrito de la app no cambia.
2. ✅ **Cómo se entera la caja (decidido el 28 nov 2026): por las dos vías.** Un mensaje
   automático en el chat del cliente ("Mandó su ubicación para el envío"), que queda en su
   historial, y un aviso al celular de la papelería por `avisos.pedidos` (ntfy). Además, el título
   del pedido en el POS ya lo muestra (🚚 | por cotizar).
3. ✅ **Quién escribe en `Envios` (decidido el 28 nov 2026): xplaya directo**, con sqlx, usando
   `ID_XPLAYA.COM_ANONYMOUS_USER` en `UserUpdatedId`, como ya hace con `Pedidos` y
   `ClienteCelulares`. Las reglas de `Envio.SetUbicacion` / `SetRecibe` se copian en Rust (no se
   cambia si ya salió, se entregó o se canceló; límites de 500/100/20 caracteres). Así se guarda al
   instante aunque el POS esté apagado. Descartado: publicar por NATS y que lo guarde un consumer
   del POS.
4. ✅ **Avisos push del envío (decidido el 28 nov 2026): dos, cotizado y en camino.** Los manda el
   POS al marcar el paso, por `NOTIFICACIONES`, como `pedido_listo`. Tipos nuevos:
   `envio_cotizado` ("Tu envío cuesta $X, transfiere para pedirlo") y `envio_salio` ("Tu pedido va
   en camino"), con `referencia` = `Pedidos.Uid`. Al tocarlos se abre la cotización. Requiere
   cambios en el POS (publicar) y en xplaya (`push.rs`: textos y destino).

## Riesgos a revisar al implementar

Revisado en el código del POS (`EnviosRepo`, `EnviosService`, `envioPedidoVM.ts`) el 28 nov 2026:

- **Descartado como riesgo:** un `UserUpdatedId` NULL no truena el POS, porque `EnviosRepo.Read` lo
  lee como `Guid.Empty`. Igual se usa `ID_XPLAYA.COM_ANONYMOUS_USER`.
- **Sí se puede romper: el diálogo 🚚 abierto pisa lo que mandó el cliente.** El diálogo carga
  referencias, quién recibe y teléfono una vez (`envioPedidoVM.ts`, al abrir) y los manda
  completos al guardar la ubicación. Si el cliente manda los suyos mientras el diálogo está
  abierto, al guardar se reemplazan por lo viejo; el punto no, porque un texto vacío lo deja como
  estaba. Arreglo en el POS: refrescar el diálogo cuando llegue el aviso, o que "guardar
  ubicación" mande solo lo que la caja cambió.
- **Crear la fila dos veces.** `Start` del POS hace "si no existe, inserta". Si la caja y el
  cliente lo hacen al mismo tiempo, uno choca con el `UNIQUE (PedidoUid)`. En xplaya:
  `INSERT … ON CONFLICT (PedidoUid) DO UPDATE` solo con los campos del cliente. En el POS, el
  choque es un error 500 raro que se resuelve reintentando.
- **Mismas condiciones que el POS para abrir un envío:** solo pedidos con `Estatus = 0` (Nuevo;
  `Start` rechaza uno cobrado). Un envío cancelado y ya solicitado no se reabre; uno cancelado sin
  solicitar se reabre como `Envio.Reopen` (borra la cotización).
- **Un envío creado cambia la caja:** el pedido deja de ofrecer "Entregar" y la pregunta de "¿ya se
  lo llevó?". Si al final el cliente pasa a recogerlo, la caja tiene que cancelar el envío en 🚚.
- **Formato de los datos:** las fechas van en hora de la tienda (`America/Cancun`, `TIMESTAMP` sin
  zona), y las coordenadas se redondean a 6 decimales (`NUMERIC(9,6)`).
- El resto de las acciones del POS (cotizar, solicitar, salió, entregado, cancelar) releen la fila
  antes de escribir, así que no pisan lo del cliente.
- Si el cliente cambia la ubicación después de cotizar, el costo ya no corresponde. ¿Se bloquea en
  la cotización, o la caja recibe un aviso para volver a cotizar?
- Que la cotización no muestre nunca `Proveedor` ni `CostoEnvio`; solo los renglones.
