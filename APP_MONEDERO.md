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
recibo**. (Una tercera, el alta desde el celular a distancia, quedó pospuesta; ver abajo.) En ambas, aceptar los términos cuenta como el "quiero participar" que hoy se pide en la
tienda; a partir de ahí puede redimir el monedero, que ya se genera de forma retroactiva en todas
las ventas.

**Un solo celular por cliente.** Si liga un celular nuevo, ese reemplaza al anterior, que queda
desligado solo. El POS también puede desligar a mano.

Después de la primera vez, entra con **huella o cara** (passkey). La passkey siempre se crea en el
celular del cliente, nunca en el kiosko.

#### Kiosko en la tienda

Una Raspberry con pantalla Elo, del lado de los clientes, que pasa slides con instrucciones muy
sencillas y muestra un **QR que cambia cada 30 segundos**. Escanearlo prueba que la persona está
frente a la pantalla. Cada enlace vale unos **3 minutos** (para quien copia el enlace y lo pega en
el navegador); lo que de verdad comprueba el teléfono es el código por WhatsApp.

- El cliente lo escanea con su celular y captura **nombre y teléfono**.
- En todas las pantallas del POS aparece un aviso con su nombre, su teléfono y un **código de 4
  dígitos**, con el botón **Enviar por WhatsApp** (abre el chat con ese número, aunque no esté en
  contactos, con el mensaje ya escrito).
- El mismo código llega también por **ntfy al celular de la papelería** (12 nov 2026), con un botón
  que abre WhatsApp con el mensaje ya escrito: sirve si nadie está viendo el POS. xplaya publica en
  NATS `avisos.papeleria-kiosko` (`src/avisos.rs`) y el puente
  [`nats-ntfy`](https://github.com/rogithub/nats-ntfy) lo pasa a ntfy. Si NATS falla, solo queda el
  aviso del POS. **Verificado en producción el 12 nov 2026**: en la tienda es más rápido mandar el
  código así (dos toques en el celular) que desde WhatsApp Web en la PC del POS.
- El cliente escribe el código en su celular: así se comprueba que el teléfono es suyo (5 intentos;
  la solicitud vive 15 minutos). Después acepta términos y crea su passkey.
- **Teléfono nuevo:** su monedero se crea hasta ese momento, no antes. **Teléfono que ya tiene
  monedero:** se liga a ese cliente. Es el mismo camino para los dos.
- Se descartó (10 nov 2026) avisar por ntfy con botones Aprobar/Rechazar: la caja solo comprobaba
  que la persona "se parecía" al dueño; el código por WhatsApp comprueba que tiene el teléfono, y
  la solicitud queda en la BD (`KioskoSolicitudes`), también la que no se completa.
- La pantalla puede confirmar "¡Listo, Juan!" al terminar, pero **nunca** muestra saldo, teléfono
  completo ni historial: es una pantalla pública.
- No es un programa aparte: es una página de xplaya (`app.xplaya.com/kiosko`) abierta en Chromium
  en modo kiosko. Se identifica con un token de dispositivo, para que nadie genere QR válidos desde
  fuera. El kiosko retirado en 2026-09 (`git checkout kiosko-v1`) ya resolvía ese token con una
  cookie HttpOnly.
- **Inicio**: a la izquierda la **Vitrina**, que alterna el QR del monedero con productos al azar
  (foto, precio y QR para pedirlo en la app; ‹ › para moverse). A la derecha, cuadros grandes (es
  táctil) que abren cada vista en pantalla completa con "← Inicio":
  - **Monedero**: lo de arriba.
  - **WiFi gratis**: QR de la red para clientes, con nombre y contraseña (`KIOSKO_WIFI_*`).
  - **Te agregamos a WhatsApp**: para quien no sabe agregar un contacto. Escribe su teléfono con
    un teclado de números en la pantalla (sin usar su celular). En el POS aparece un aviso con el
    botón para escribirle; cuando contesta (eso comprueba que el teléfono es suyo), la caja escribe
    su nombre y crea el cliente. Queda sin "aceptó el programa": no aceptó los términos. Si el
    teléfono ya era de un cliente, no se crea otro. Es la misma tabla `KioskoSolicitudes`, con
    `Tipo = 'whatsapp'`: las dos solicitudes comprueban el teléfono.
  - Sin tocar nada en un minuto, vuelve al inicio y la vitrina sigue pasando sola.
- Entre las slides de instrucciones puede mostrar promociones de la semana.
- **La Raspberry del kiosko** (`cajita`, Raspberry Pi OS con labwc), por si hay que rehacerla:
  - `~/.config/labwc/autostart` abre `chromium --kiosk ... "$(cat /home/ro/url.txt)"`. `url.txt`
    (chmod 600) tiene `https://app.xplaya.com/kiosko/activar?t=TOKEN`: cada arranque renueva la
    cookie. Sin la barra del escritorio; se administra por SSH.
  - Lee la EDID de la Elo (ET2270L) sin ayuda; no necesita nada en `cmdline.txt`.
- **`chaac`** (Raspberry 4) fue el kiosko hasta el 2026-11-11; ahora es caja alternativa con
  escritorio normal y teclado en pantalla (squeekboard). El autostart del kiosko quedó como
  `~/.config/labwc/autostart.kiosko`.
  - En su puerto la Elo no entrega su EDID: sale `disconnected` y sin medidas físicas.
    `/boot/firmware/cmdline.txt` lleva al final `video=HDMI-A-2:1920x1080@60D` (fuerza la
    salida) y `drm.edid_firmware=HDMI-A-2:edid/elo.bin` (EDID copiada de `cajita` a
    `/lib/firmware/edid/elo.bin`). Sin la EDID, squeekboard cree que es un celular y saca un
    teclado que tapa media pantalla, sin importar su escala. Respaldos: `cmdline.txt.respaldo`
    (sin nada) y `cmdline.txt.respaldo2` (solo `video=`).

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

#### Alta desde el celular — POSPUESTA (12 nov 2026)

**Pospuesta:** por ahora el alta es solo en la tienda (kiosko) o con el enlace del recibo. No hay
código ni cambios de BD de esto; queda el diseño por si se retoma. (Lo que sí se hizo: el aviso por
ntfy del código **del kiosko**, ver "Kiosko en la tienda".) Si se retoma: Origen en
`KioskoSolicitudes` y `HORARIO_ALTA_REMOTA` en `Settings` (repo del POS), límites por teléfono y
global en `src/db/kiosko.rs`, límite por IP en memoria, y publicar en NATS desde xplaya.

Para quien se quiere registrar sin estar frente a la pantalla del kiosko. Es **el mismo flujo del
kiosko**; cambian la entrada y por dónde le llega el código a la papelería. Los dos caminos conviven.

| | Kiosko | Desde el celular |
|---|---|---|
| Entrada | Escanea el QR de 30 s | Abre `app.xplaya.com/alta` desde donde esté |
| Captura | Nombre y teléfono | Igual |
| El código aparece en | Aviso flotante del POS | Notificación ntfy en el **celular de la papelería** |
| Lo manda | La caja, por WhatsApp | Quien porta ese celular, con el botón **Enviar por WhatsApp** de la notificación |
| Después | Código (5 intentos, 15 min), términos, passkey | Igual |

- Lo que comprueba el teléfono sigue siendo el **código por WhatsApp**, no un juicio de quien lo
  recibe (por eso no revive lo descartado el 10 nov: ntfy solo es el canal, no hay Aprobar/Rechazar).
- Misma tabla `KioskoSolicitudes`, con una columna de **origen** (`kiosko` / `remoto`).
- xplaya **no habla con ntfy**: publica en NATS `avisos.papeleria-kiosko` el JSON de ntfy (título, nombre,
  teléfono, código y una acción `view` a `https://wa.me/52…?text=…`), y el puente
  [`nats-ntfy`](https://github.com/rogithub/nats-ntfy) lo reenvía. Si ntfy está caído, NATS lo
  retiene y reintenta. Infra pendiente: `k3s-manifests/CLAUDE.md`, sección "Eventos — NATS".
- Opción abierta: que las solicitudes del kiosko también avisen por ntfy, por si nadie ve el POS.
- Solicitudes falsas (decidido antes de posponer): límites (una viva por teléfono, 3 al día por
  teléfono, ~5/hora por IP, tope global ~20/hora) y solo en horario de la tienda. Turnstile solo
  si aparece abuso.

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

- **Pedido listo**: botón "Avisar que está listo" en el pedido del POS. Si el cliente no tiene la
  app con notificaciones, el POS ofrece el mensaje de WhatsApp de siempre.
- Cuando se abona al monedero.
- 7 días antes de que venza su saldo.

Se envían con Web Push (llaves VAPID) al celular ligado del cliente (`ClienteCelulares.Push*`).
Todo pasa por la tabla **`Notificaciones`** del repo del POS, que funciona como bandeja de salida:

1. Quien quiere avisar (el POS) inserta una fila: cliente, tipo, título, texto y ruta de la app.
2. Al hacer commit, un trigger hace `NOTIFY notificaciones` con el Id; xplaya escucha con
   `LISTEN` (`PgListener` de sqlx) y la manda al instante. Al arrancar, manda las pendientes.
3. xplaya llena las marcas: `enviada` (el servicio de push la aceptó), `sin_app`, `fallida` (si la
   suscripción ya no existe, se borra del celular), y `FechaRecibido` / `FechaAbierto`, que avisa
   el service worker. El POS las muestra en la ficha del cliente y en el pedido.

Abonos y vencimientos entran a la misma tabla (por definir: trigger en `MonederoGenerados` o tarea
diaria en xplaya).

**Por qué LISTEN/NOTIFY y no NATS** (decidido el 11 de noviembre de 2026): el POS y xplaya ya
comparten la BD, el aviso solo sale si la transacción se guardó, y si xplaya está caído no se
pierde nada. NATS (ya en el cluster) entra si aparece un sistema que no deba conectarse a la BD;
como todo pasa por la tabla, bastaría un relevo de la tabla a NATS. El mismo camino sirve para un
futuro **chat** del POS con clientes de la app (tabla `Mensajes` + `NOTIFY`, SSE en la app).

#### En el POS: "Avisar por la app"

Los pedidos se guardan **ya terminados** (llega el encargo por WhatsApp, se agregan las líneas y
el cliente, se imprime y se guarda). Al guardar sale el panel "¡Guardado!" con el mensaje para
WhatsApp y **Copiar todo** (`savedTicket.ts`, `Views/Ventas/_VentasTemplates.cshtml`). Ahí, junto
a **Copiar todo**, va **📲 Avisar por la app**, solo si el cliente tiene celular ligado con avisos
(`ClienteCelulares.FechaPush`). El POS inserta en `Notificaciones` (`pedido_listo`, mismo texto
que el de WhatsApp, `Url` a la cotización) y publica el Id en NATS (`notificaciones.nueva`, usuario
`pos`). El botón muestra Enviado ✓ → Recibido ✓✓ → Abierto 👁, o "No llegó" (y queda WhatsApp).

#### Dónde se pide el permiso

El permiso es del **celular del cliente**, no de cada pedido: una vez que acepta, le llega cualquier
pedido con su `ClienteId`, lo haya hecho él en la app o la caja en el POS a partir de su WhatsApp.
Solo se pide con sesión (huella), y nunca sin contexto: un "no" del navegador es difícil de deshacer.

1. **Al hacer un pedido en la app**: después de "Pedido enviado", "¿Te avisamos cuando esté listo?"
   con el botón **Sí, avísame**.
2. **Aviso fijo en Monedero** (como "Instala la app"): "🔔 Recibe avisos de tus pedidos y tu saldo".
   Desaparece al aceptar. Es el camino principal de quien pide por WhatsApp.
3. **En la cotización** (`/cotizacion/{guid}`, el enlace que la caja manda por WhatsApp): "¿Quieres
   que te avisemos cuando esté listo? Ábrelo en la app", con enlace a la app.

- **iPhone**: solo con la app instalada (iOS 16.4+). Sin instalar, en lugar del botón: "Primero
  instala la app" con los pasos de siempre.
- **Si dijo que no**: no se vuelve a preguntar; el aviso explica cómo activarlo en los ajustes.
- **En el POS**: el pedido tiene que quedar a nombre del cliente. Sin `ClienteId`, el botón dice
  "Este pedido no tiene cliente" en lugar de fallar en silencio.
- Para después: avisar también "Recibimos tu pedido" al capturarlo.

### Tienda dentro de la app

La app tiene una barra abajo: **Monedero · Tienda · Carrito** (`templates/app/nav.html`).

- **Tienda** (`/tienda`, `/tienda/{nid}`): el mismo catálogo y búsqueda de xplaya.com
  (`db::productos`), con el diseño de la app. Búsqueda con formulario normal y páginas
  anterior/siguiente (la app no usa HTMX).
- **Carrito** (`/carrito`): el store de `static/js/cart.js`, en el `localStorage` de
  app.xplaya.com (aparte del de xplaya.com).
- **Pedido** (`POST /tienda/pedido`): con la sesión del celular ligado ya se sabe quién es, así
  que **no pide nombre ni teléfono**: entra a nombre de ese cliente (`Origen = 1`, como los de la
  web) y responde el enlace de su cotización. Sin sesión se puede ver todo; para mandar, "Entrar
  con huella" y regresa al carrito.
- No se unen app.xplaya.com y xplaya.com (2026-11-10). El carrito de xplaya.com tiene "Entrar
  con huella" con la misma passkey y crea su propia sesión (no se comparte la cookie de la app
  para que la llave no viaje a los demás subdominios); con ella no pide datos.

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
| Solicitudes del kiosko (`KioskoSolicitudes`) | BD compartida | Tipo (`monedero` o `whatsapp`), nombre, teléfono, código, intentos, estado y el cliente que quedó |
| Passkeys | xplaya | Crate `webauthn-rs` |
| Web Push | xplaya | Crate `web-push`, llaves VAPID en SealedSecret |
| Aviso del kiosko con el código | POS | Aviso flotante en todas las pantallas, botón a WhatsApp |
| Aviso del alta desde el celular | NATS `avisos.papeleria-kiosko` → `nats-ntfy` → ntfy | Al celular de la papelería, botón a WhatsApp (**pospuesto**) |
| Ver y desligar el celular | POS | Ficha del cliente |

## Orden propuesto

1. Tarjeta de solo lectura (con el diseño nuevo) sobre la URL de hoy.
2. `app.xplaya.com` + PWA instalable (manifest, service worker, guía para iPhone).
3. Tablas nuevas, términos y alta con passkey desde el enlace del recibo.
4. Ver y desligar el celular en la ficha del cliente del POS (el QR que caduca se descartó por ahora).
5. Kiosko: página, QR de 30 s y código por WhatsApp desde el POS.
6. Notificaciones: abonos y vencimiento.
