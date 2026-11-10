# App de clientes — Papelería xplaya

Idea general, definida el 9 de noviembre de 2026. Se movió aquí desde el repo `pwa-monedero`, que
solo guardaba esta documentación y se retira.

## Propósito

Una app para el celular de los clientes de la papelería. El gancho es el programa de lealtad
(monedero). Más adelante incluirá un chat con los clientes, para depender menos de WhatsApp.

## Dónde vive

- Se construye **dentro de este repo** (Rust + Axum) y se sirve en **`app.xplaya.com`**. El mismo
  servidor atiende ambos dominios y separa las rutas por el `Host` de la petición.
- Es una PWA: se instala desde el navegador, sin tiendas de apps (`manifest.json` + service worker).
- La imagen de la app (ícono, tarjeta, colores) usa el logo oficial: archivos en `static/img/logo/`,
  guía en `templates/pages/logo-oficial.html` (publicada en https://xplaya.com/logo-oficial).
- Requiere en `k3s-manifests`: DNS, ruta en el túnel y TLS para `app.xplaya.com`. Las passkeys
  exigen HTTPS (en desarrollo, `localhost` también sirve).

## Etapas

1. **Tarjeta del monedero** (esta etapa).
2. **Carrito de compras**: lo que hace hoy xplaya.com, pero pensado para el celular.
3. **Chat** con los clientes. Fuera del alcance de la etapa 1.

## Primera versión: la tarjeta

### Mis tarjetas

Solo aparece la tarjeta de la papelería. Muestra:

- Nombre del cliente y "Miembro desde".
- Saldo, vencimiento e historial (lo mismo que hoy muestra xplaya, pero ordenado, limpio y bonito).
- Un QR para escanear en caja.

### Alta y acceso

Hay dos formas de ligar un celular a un monedero: el **kiosko** de la tienda y el **enlace en el
recibo**. En ambas, aceptar los términos cuenta como el "quiero participar" que hoy se pide en la
tienda; a partir de ahí puede redimir el monedero, que ya se genera de forma retroactiva en todas
las ventas.

**Un solo celular por cliente.** Si liga un celular nuevo, ese reemplaza al anterior, que queda
desligado solo. El POS también puede desligar a mano.

Después de la primera vez, entra con **huella o cara** (passkey). La passkey siempre se crea en el
celular del cliente, nunca en el kiosko.

#### Kiosko en la tienda

Una Raspberry con pantalla Elo, del lado de los clientes, que pasa slides con instrucciones muy
sencillas y muestra un **QR que cambia cada 30 segundos**. Escanearlo prueba que la persona está
frente a la pantalla.

- El cliente lo escanea con su celular y captura **nombre y teléfono**.
- **Teléfono nuevo:** se da de alta en el momento, acepta términos y crea su passkey.
- **Teléfono que ya tiene monedero:** llega una notificación de **ntfy** a la caja (tema
  `papeleria-altas`), por ejemplo *"Juan Pérez (\*\*\*\*1234) está en el kiosko, ¿es él?"*, con
  botones **Aprobar** / **Rechazar**. El botón llama a xplaya con un token de un solo uso. El
  celular del cliente muestra "Esperando confirmación…" y continúa al aprobarse.
- La pantalla puede confirmar "¡Listo, Juan!" al terminar, pero **nunca** muestra saldo, teléfono
  completo ni historial: es una pantalla pública.
- No es un programa aparte: es una página de xplaya (`app.xplaya.com/kiosko`) abierta en Chromium
  en modo kiosko. Se identifica con un token de dispositivo, para que nadie genere QR válidos desde
  fuera. El kiosko retirado en 2026-09 (`git checkout kiosko-v1`) ya resolvía ese token con una
  cookie HttpOnly.
- Entre las slides de instrucciones puede mostrar promociones de la semana.

#### Enlace en el recibo

Los tickets no se imprimen: el recibo se comparte por WhatsApp como `/recibo/{guid}`. Ese recibo
muestra un enlace de alta de **un solo uso** y válido por **24 horas**, para quien compró y se fue
sin pasar por el kiosko. Si el cliente de la venta ya tiene un celular ligado, el enlace dice
"¿Cambiaste de celular? Liga el nuevo" (es la forma de recuperar el acceso sin ir a la tienda); si
quien lo abre ya tiene sesión con ese mismo cliente, la app lo manda a su monedero sin gastar el
enlace. Los 4 dígitos admiten **5 intentos**; al quinto fallo el enlace queda bloqueado.

| Caso | Qué pasa |
|---|---|
| La venta tiene cliente cargado | Escribe los **últimos 4 dígitos** de su teléfono y liga su celular a ese monedero |
| Venta sin cliente, teléfono nuevo | Captura nombre y teléfono; se crea su monedero y se abona esa compra |
| Venta sin cliente, teléfono que ya tiene monedero | **No** se liga el celular (desde fuera de la tienda no se puede comprobar el número), pero **sí** se abona esa compra a ese monedero. La app le pide que en su próxima visita escanee el kiosko |
| Cliente que ya tiene la app abre el enlace de una venta sin cliente (olvidó mostrar su tarjeta) | La compra se abona a su monedero; no se crea otro cliente |

Si el celular de un cliente nuevo no puede crear passkeys, "Solo abona mi compra" crea su monedero
y abona la compra sin ligar el celular; lo ve en la web, como hasta hoy.

#### Si cambia o pierde el celular

- La passkey se sincroniza sola con su cuenta de Apple o Google.
- Si eso no funciona, vuelve a ligar con el kiosko o con el enlace de un recibo; el celular nuevo
  reemplaza al anterior.

#### iPhone

Si la abre sin instalarla, la app le muestra paso a paso cómo agregarla a la pantalla de inicio. Sin
instalarla no recibe notificaciones (Web Push en iOS requiere 16.4+ y la app instalada).

### En la caja

- El QR de la tarjeta lleva el **teléfono** del cliente, igual que las tarjetas impresas; el
  escáner de caja lo teclea en el buscador de clientes.
- **Se descartó (por ahora) un QR que caduca cada minuto.** Solo protegería si el POS lo exigiera
  para gastar el monedero, y hoy hay pocos clientes con monedero; casi todos escriben por WhatsApp
  desde el mismo número de su monedero, y así se verifica que son ellos (decidido el 10 de noviembre
  de 2026). Se retoma si crecen los clientes: el QR llevaría el ID del cliente más un código de un
  minuto calculado en la PWA con un secreto que el servidor da al ligar el celular.
- En la ficha del cliente, el POS muestra su celular ligado y permite **desligarlo**.

### Notificaciones

- Cuando se abona al monedero.
- 7 días antes de que venza su saldo.

Se envían con Web Push (llaves VAPID). Como los abonos los crea el POS, xplaya se entera con una
**tarea que cada minuto busca abonos nuevos** en `MonederoGenerados`; la misma tarea revisa una vez
al día los saldos que vencen en 7 días. Si más adelante el POS publica eventos en NATS, se puede
cambiar a escucharlos.

### Términos y privacidad

Se usan los mismos términos del programa que ya están en xplaya.com (incluyen privacidad conforme a
la LFPDPPP y el derecho a pedir el borrado de datos), actualizados para mencionar la app, el celular
ligado, el kiosko y las notificaciones.

## Cambios en xplaya.com

- La consulta de saldo por teléfono (`/saldo`) y el monedero público (`/monedero/{guid}`) **se
  quedan por un tiempo**, para no dejar fuera a quien no quiera usar la app. Más adelante se revisa
  si se quitan.
- **Se quedan públicos** como están: recibos, cotizaciones y enlaces cortos (`/recibo`,
  `/cotizacion`, `/r/:code`). El recibo suma el enlace de alta descrito arriba.

## Piezas nuevas

| Pieza | Dónde | Notas |
|---|---|---|
| Tabla de celulares ligados (passkey, secreto del QR, suscripción push, fechas) | BD compartida | Una fila activa por cliente |
| Fecha de aceptación de términos | `Clientes` | El "quiero participar" |
| Tokens de alta (recibo, 24 h, un solo uso) | BD compartida | Parecida a `ShortUrls` |
| Aprobaciones pendientes del kiosko | xplaya, en memoria | Token de un solo uso para el botón de ntfy; si el pod se reinicia, el cliente vuelve a escanear (como el alta del recibo) |
| Passkeys | xplaya | Crate `webauthn-rs` |
| Web Push | xplaya | Crate `web-push`, llaves VAPID en SealedSecret |
| Publicar en ntfy | xplaya | Token de publicador en SealedSecret; tema `papeleria-altas` |
| Ver y desligar el celular | POS | Ficha del cliente |

## Orden propuesto

1. Tarjeta de solo lectura (con el diseño nuevo) sobre la URL de hoy.
2. `app.xplaya.com` + PWA instalable (manifest, service worker, guía para iPhone).
3. Tablas nuevas, términos y alta con passkey desde el enlace del recibo.
4. Ver y desligar el celular en la ficha del cliente del POS (el QR que caduca se descartó por ahora).
5. Kiosko: página, QR de 30 s y aprobación por ntfy.
6. Notificaciones: abonos y vencimiento.
