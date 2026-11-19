# REVISIONES.md — xplaya

Bitácora de cambios paso a paso. Las entradas más recientes van arriba.

---

## Chat de la app, etapa 3a, paso 4: respuesta automática fuera de horario (2026-11-19)

- `src/chat/consumer.rs` — `fuera_de_horario()`: al guardar un mensaje del cliente con la tienda
  cerrada, publica en `chat.{cliente}.automatico.mensaje` el aviso con el horario. Antes revisa
  si ese aviso (mismo id en todo el cierre) ya está guardado: uno por cierre. Si NATS falla, solo
  log. La hora que cuenta es la del mensaje, no la de cuando se procesa.
- `src/chat/reglas.rs` — el aviso sale un segundo después del mensaje, para quedar debajo.
- `src/chat/contrato.rs` — `con_zona()`, lo contrario de `hora_tienda()`.
- Sin NATS en desarrollo: se prueba en producción con `probar-chat.sh` fuera de horario.

## Chat de la app, etapa 3a, paso 3: lo que la app lee de la BD (2026-11-19)

`src/db/chat.rs`, con pruebas `#[sqlx::test]` sobre el schema del POS.

- `conversacion()` — los últimos N mensajes, o los anteriores a uno (al subir en el chat), del más
  viejo al más nuevo. Pagina por (fecha, id): mensajes del mismo segundo no se pierden. Trae el
  inicio del texto citado; si el citado se eliminó, sin texto.
- `bloqueado()` — `Clientes.ChatBloqueado`.
- `sin_leer()` — lo que le escribieron al cliente y no ha leído (el globito del menú).
- `ajustes()` — una sola consulta; suma `CHAT_CLIENTES_PRUEBA` (con la fila, aunque sea NULL,
  cuenta como vacía: todos).
- `src/chat/vista.rs` — `autor` y `tipo` como `String` (vienen de la BD).

## Horario: los sábados se abre a las 8:00 (2026-11-19)

- Pie de página y datos de Google (`OpeningHoursSpecification`) en `templates/base.html`,
  `templates/pages/faq.html`, `templates/pages/impresiones.html` y `/llms.txt`
  (`src/routes/pages.rs`). El chat ya lo tenía (`src/chat/reglas.rs`).

## Chat de la app, etapa 3a, paso 2: reglas de la app y horario (2026-11-19)

Todo en `src/chat/reglas.rs`, puro y con pruebas. Lo usan los pasos 4 y 5 (mientras, Clippy avisa
que no se usan).

- `del_cliente()` — arma el mensaje v1 que escribió el cliente: rechaza vacío, más de 4 000
  caracteres o chat bloqueado. La fecha la pone xplaya y el id el celular.
- `chat_visible()` — `CHAT_CLIENTES_PRUEBA`: sin la fila nadie ve el chat, vacía todos, o la lista
  de ids separados por coma.
- `abierto()`, `inicio_del_cierre()` — L–V 6:30–18:00, S 8:00–14:00, domingo cerrado.
- `aviso_fuera_de_horario()` — la respuesta automática; su id (UUID v5) sale del cliente y de la
  hora del cierre, así que es el mismo en todo ese cierre: si ya está guardado, no se repite.
- Editar y eliminar desde la app no tienen reglas nuevas: se reusa `aplicar()`.
- `Cargo.toml` — `uuid` con `v5`.

## Chat de la app, etapa 3a, paso 1: publicar y avisar en vivo (2026-11-19)

Plan de la 3a: 8 pasos (publicar, reglas, BD, fuera de horario, rutas, pantalla, menú, privacidad).
Todavía sin pantallas.

- `src/chat/nats.rs` (nuevo) — `Chat` en `AppState`: publica en el stream `CHAT` esperando el ack
  (el ✓ de la app) con `Nats-Msg-Id` para no guardar dos veces lo que el celular reintente; manda
  el "escribiendo…" del cliente por NATS núcleo y escucha el de la papelería
  (`chat.*.papeleria.escribiendo`). Reparte los cambios con un `broadcast` en memoria: vale porque
  xplaya corre con una réplica.
- `src/chat/vista.rs` (nuevo) — `Vista` (un mensaje como lo dibuja la app) y `EnVivo` (los eventos
  del SSE). No es contrato de NATS: puede cambiar sin subir `v`.
- `src/chat/consumer.rs` — después de guardar, editar, marcar leído o eliminar, avisa en vivo. Así
  la app solo ve lo que ya está en `Mensajes`.
- `src/chat/contrato.rs` — fuera los `allow(dead_code)` de `Tema::armar`.

## Chat de la app, etapa 2: xplaya guarda lo que pasa por el stream CHAT (2026-11-19)

Diseño completo en `CHAT.md` (nuevo). Todavía no hay pantallas: esto es la base.

- `src/chat/contrato.rs` — el formato v1 (mensaje, leído, editado, eliminado) y el tema
  `chat.{cliente}.{lado}.{evento}`. Puro. Sus pruebas leen los ejemplos de `contratos/chat-v1/`,
  que también sirven al POS y al asistente.
- `src/chat/reglas.rs` — `aplicar()` decide qué hacer con cada evento y devuelve un `Cambio` sin
  ejecutarlo: el autor sale del tema, repetidos fuera, editar solo lo propio en 15 min, eliminar
  solo lo propio, archivos solo en la carpeta del cliente. Puro, con pruebas.
- `src/chat/consumer.rs` — lee el consumer `xplaya` del stream `CHAT` (como `fotos.rs`), pregunta a
  las reglas y ejecuta: BD, MinIO (`papeleria-chat`) y quitar del stream lo eliminado.
- `src/db/chat.rs` — consultas de `Mensajes`. Pruebas con `#[sqlx::test]` sobre el schema real del
  POS (`../inventario_papeleria/...` o `POS_SCHEMA`); sin él se saltan.
- `src/main.rs` — arranca `chat::vigilar`. Sin variables nuevas (reusa `NATS_*` y `MINIO_*`).
- La tabla `Mensajes` y `Clientes.ChatBloqueado` están en el repo del POS (`dbchanges/updates.sql`).
- **Probado en producción el 19 nov 2026** con `/home/ro/srv/share/xplaya-chat/probar-chat.sh`: un
  mensaje editado y leído, y otro que la caja no pudo borrar y el cliente sí.

---

## Kiosko: el ícono de la app sí se achica (2026-11-18)

- `static/css/app.css` — `.cuadro-app .app-icono` tenía un `height: auto` viejo después del
  `height` nuevo, así que el ícono salía a 192 px y sacaba la barra de botones por abajo de la
  pantalla (por eso se veía pegada al borde y cortada). Ahora mide 9vh.

---

## Kiosko: espacios parejos en el inicio (2026-11-18)

- `static/css/app.css` — `.kiosko-inicio` con 3vh en los cuatro lados (antes 2.5vh / 2.5cqw / 4vh)
  y 2vh entre cajas: entre la vitrina y la barra, y entre botones (`.kiosko-cuadros`, antes 1.5cqw).
  Afuera un poco más que adentro, todo en `vh`.

---

## Kiosko: la barra de abajo más baja y separada del borde (2026-11-18)

- `static/css/app.css` — `.kiosko-cuadros` de 17vh a 13vh (ícono de la app de 12vh a 9vh) y
  `.kiosko-inicio` con 4vh de espacio abajo, para que los botones no queden pegados al borde.

---

## Kiosko: fuera Reseñas; los botones en una barra abajo (2026-11-18)

Con menos cuadros, la columna de la derecha desperdiciaba pantalla. Ahora la vitrina va a todo lo
ancho y los botones en un solo renglón abajo.

- `templates/app/kiosko.html`, `src/routes/kiosko.rs` — fuera el cuadro "Tu reseña", su vista y el
  QR a Google (la página `/resena` de xplaya.com sigue).
- `static/css/app.css` — `.kiosko-inicio` en dos renglones (vitrina / barra); `.kiosko-cuadros` es
  una barra de 17vh con la app más ancha (1.7fr) y WhatsApp y Buscar con el ícono a un lado.

---

## Fuera xplaya.com/fotos y xplaya.com/foto-credencial (2026-11-18)

Las fotos de credencial y polaroid ya solo se hacen en la app; esas dos páginas mandaban a algo que
ya no existe (el kiosko de imagina) o a la app sin sesión. Se quitan, con su menú.

- `templates/pages/fotos.html`, `templates/pages/foto-credencial.html` — borradas; las rutas
  (`src/main.rs`, `src/routes/pages.rs`) también: ahora dan 404.
- `templates/base.html` — fuera "Fotos" del menú (con su submenú), del pie y del JSON-LD de la
  tienda.
- `src/routes/pages.rs` — fuera del sitemap y de `llms.txt`.
- `templates/pages/faq.html`, `templates/pages/impresiones.html` — sin enlaces a `/fotos`; dicen que
  las polaroid se arman en la app de la Papelería.

---

## Fotos en la app, etapa 4: se quita lo viejo (2026-11-18)

El kiosko de fotos de imagina (`imagina.xplaya.com/kiosko`) ya no existe: credencial y polaroid se
hacen solo en la app.

- `templates/app/kiosko.html`, `src/routes/kiosko.rs` — fuera el cuadro "Fotos credencial" y su
  QR a imagina. `static/css/app.css`: si el último cuadro queda solo en su renglón, va a lo ancho
  ("Buscar por número").
- `static/app/app.js`, `templates/app/fotos.html` — la credencial ya no ofrece **Pasaporte**: el
  consulado casi siempre rechaza una foto de celular (lo dice xplaya.com/foto-credencial). Quedan
  Infantil (por omisión) y Credencial 3.5 × 4.5 ("también título").
- `templates/pages/foto-credencial.html`, `templates/pages/fotos.html` — los botones llevan a la app
  (`{{ app_url }}/fotos/credencial` y `/fotos/polaroid`), y el texto dice que es en la app gratis
  de la Papelería, que se instala en la tienda (QR del kiosko o en caja). Fuera "sin instalar
  nada" y "sin cuenta"; las medidas que la app no tiene (credencial grande, licencia, otra medida)
  dicen "en la papelería". También el JSON-LD (pasos y preguntas).
- `src/routes/pages.rs` — el texto para buscadores e IA (`llms.txt`).

---

## Fotos en la app, etapa 3: fotos polaroid (2026-11-18)

"Fotos polaroid" en la pestaña Fotos. Es el kiosko polaroid de imagina pasado a Alpine, con el mismo
pedido para que la operadora lo abra y lo corrija en el panel de imagina como siempre.

- `static/app/polaroid.js` — nuevo; solo lo carga esa pantalla. Formatos (Tira 5 × 15 con 3 fotos,
  Grande 10 × 15, Clásica 10 × 9, Mini 5 × 7) con su marco; cuántas caben en carta; recorte
  automático al elegir (varias a la vez según la memoria del celular); acomodo del texto (el mayor
  tamaño que cabe, líneas parejas) dibujado igual en la vista y en el PNG que va al PDF; encuadre
  arrastrando y acercando, guardado como `box` en fracciones; borrador en IndexedDB
  (`xplaya-fotos`, 48 h) por si el navegador cierra la página. "Atrás" del celular regresa de
  pantalla en pantalla.
- Al mandar: sube a imagina los recortes (la tira, sus 3 fotos ya juntas) y los textos en PNG
  (`/api/v1/images`), crea el pedido `polaroid-grid` (`/api/v1/orders/print`, con `X-Permiso`) y
  sube el paquete para corregir en tienda (originales, recortes y `proyecto.json`, version 1,
  `tool: kiosko-polaroid`) con su avance en %.
- `templates/app/fotos_polaroid.html` — formato → tus fotos → editar (texto, 15 letras, 8 colores,
  encuadre, cambiar, quitar; en la tira, sus 3 fotos) → mandar. Sin el selector de emojis del
  kiosko: el teclado del celular ya los trae.
- `src/routes/fotos.rs` — `GET /fotos/polaroid`; `templates/app/fotos.html` ya la enlaza.
- `static/css/app.css` — `.pol-*`.

Probado en local (imagina, MinIO y NATS de prueba): Mini con 3 fotos, texto, letra, color y un
encuadre; Tira con 3 fotos y su texto; recargar a medio camino recupera todo. imagina guardó el
PDF y el paquete completo (8 archivos) y el pedido salió en "Tus pedidos".

---

## Fotos en la app, etapa 2: foto de credencial (2026-11-18)

Pestaña nueva **📸 Fotos** abajo (solo con `FOTOS_LLAVE_PRIVADA`). Ahí: "Foto de credencial",
"Fotos polaroid" (pronto) y **Tus pedidos** con su estado (📥 Lo recibimos · ✅ Terminado ·
Cancelado), al día por NATS. Basado en el kiosko de imagina (`kiosko/carnet`, `CameraCapture`).

- `templates/app/fotos.html` — la pestaña; recién mandado, se recarga cada 3 s hasta que el pedido
  aparece (NATS tarda un instante).
- `templates/app/fotos_credencial.html` + `fotoCredencial` en `static/app/app.js`:
  1. **Tamaño**: Infantil 2.5 × 3 (por omisión, "la más pedida"), Credencial y Pasaporte 3.5 × 4.5,
     dibujados a escala. Consejos de fondo y luz.
  2. **Cámara** a pantalla completa (la de atrás; botón para voltear): ventana al tamaño, óvalo de la
     cara y línea de los ojos. Al disparar se toma ese rectángulo del sensor (compensando el
     `object-fit: cover`). Sin cámara o sin permiso: elegir o tomar con la del celular.
     También "Elegir de mis fotos".
  3. **Acomodar**: arrastrar y acercar (1×–3×) en un marco del tamaño; la foto siempre lo cubre.
  4. **Revisar y mandar**: se hornea a 300 dpi (infantil 295 × 354 px) y se manda **directo a
     imagina** (`/api/v1/kiosko/carnet`) con `X-Permiso`; si el permiso venció, pide otro una vez.
- `src/routes/fotos.rs` — `/fotos`, `/fotos/credencial`, `/fotos/permiso`; `src/db/fotos.rs`
  `de_cliente` (últimos 30 días). El aviso al terminar abre `/fotos`.
- `templates/app/nav.html` — la pestaña; `static/css/app.css` — `.fot-*`.

Probado en local con cámara simulada de Chromium, imagina, MinIO y NATS de prueba: tamaño →
cámara → acomodar → mandar → aparece en Tus pedidos como "Lo recibimos". Falta probarlo en
celulares reales (iPhone y Android) después de instalarlo.

---

## Fotos en la app, etapa 1: permiso para imagina y estado de los pedidos (2026-11-18)

Primer paso para hacer la foto de credencial y las polaroid desde la app (plan en el artifact
"Fotos en la app"). El pedido sigue viviendo en **imagina** (la operadora lo corrige, imprime y
marca); la app es la puerta del cliente. Todavía no hay pantallas: eso es la etapa 2.

- `src/fotos.rs` — `Firmador`: firma con Ed25519 (`FOTOS_LLAVE_PRIVADA`) un permiso de 30 min con
  el cliente, su nombre y teléfono. `vigilar`: lee `imagina.pedidos` (stream IMAGINA, consumer
  `xplaya`), lo guarda en `PedidosFotos` y, al terminar (impreso, cancelado o borrado), le manda
  un aviso al cliente una sola vez.
- `src/routes/fotos.rs` — `GET /fotos/permiso`: `{permiso, imagina}` para el cliente con sesión.
- `src/db/fotos.rs` — guardar el estado (devuelve el anterior) y nombre/teléfono del cliente.
- `src/main.rs`, `src/config.rs` — `AppState.fotos`, global `fotos_activo`; la llave pública sale
  en el log al arrancar.
- Del lado de imagina (`imagina/src/permiso.rs`): revisa el permiso con la pública, guarda el
  cliente en el pedido y publica cada cambio.

Probado en local de punta a punta: permiso → imagina (con MinIO y NATS de prueba) → pedido con
nombre y teléfono → NATS → `PedidosFotos` "recibido" → "impreso".

---

## El kiosko se recarga solo después de un deploy (2026-11-18)

Después de subir los anuncios, la pantalla del kiosko siguió con la página vieja hasta reiniciar la
Elo (no tiene teclado para Ctrl+Shift+R). Ahora el servidor calcula al arrancar una **versión de las
páginas** (huellas de CSS/JS + contenido de las plantillas) y la manda con cada QR (`/kiosko/qr`,
cada ~30 s). Si no es la de la página, el kiosko se recarga en cuanto lleva 1 min sin que nadie lo
toque.

- `src/estaticos.rs` — `Huellas::version("templates")`.
- `src/main.rs` — `AppState.version_paginas` y la global `version_paginas` de las plantillas.
- `src/routes/kiosko.rs` — `qr` devuelve también `version`.
- `templates/app/kiosko.html` — `data-version` en `.kiosko`.
- `static/app/app.js` — `nuevoQr` marca `nuevaVersion`; el reloj de "sin tocar 1 min" recarga.

---

## Anuncios del kiosko: se recorren con el dedo (2026-11-18)

La franja de anuncios de clientes ya no solo avanza sola: tiene flechas ‹ › abajo, a los lados de
"3 / 12", y se desliza con el dedo. Si alguien la toca, se queda quieta 30 s para leer con calma;
luego sigue cada 10 s. Así no importa cuántos anuncios haya: quien quiera, los recorre. La vitrina
de la izquierda (productos y la app) sigue igual: esa es la parte de la papelería.

- `static/app/app.js` — `kioskoAnuncios`: `mover(paso)`, `pausaHasta` y el deslizar
  (`tocar`/`soltar`, más de 50 px).
- `templates/app/kiosko.html` — flechas y contador (solo si hay anuncios; en la invitación del final
  no sale el contador).
- `static/css/app.css` — `.kan-flecha`, `.kan-cuenta`; abajo, para no tapar el anuncio.

---

## Anuncios del kiosko: reglas que se aceptan al mandarlo (2026-11-18)

Antes de mandar su anuncio, el cliente ve las reglas (nada de política, alcohol, préstamos ni
contenido para adultos; nada de otras papelerías o centros de copiado; que sea verdad; la
papelería puede no aprobarlo o quitarlo sin explicación) y marca "Acepto las reglas". Sin tope de
anuncios en la pantalla: la papelería lo controlará metiendo anuncios suyos entre los de clientes.

- `templates/app/anuncio.html` — el cuadro de reglas con la casilla `acepto`.
- `static/app/app.js` — `anuncioForm` no envía sin la casilla.
- `src/routes/anuncio.rs` — `guardar` también lo revisa (sin `acepto=si`, no sube nada).
- `static/css/app.css` — `.anu-reglas`, `.anu-acepto`.

---

## Anuncios del kiosko: "Tu anuncio en la tienda" (2026-11-17)

Cada cliente con la app pone un anuncio en la pantalla de la tienda: una **imagen 9:16** (la de
su estado de WhatsApp) o un **texto sobre un color**. Uno por cliente. La caja lo aprueba en el
POS (Contactos → Anuncios del kiosko, rama `feature/anuncios-kiosko` del POS, tabla `Anuncios`)
y sale **7 días** en una franja derecha del kiosko de 608 × 1080 (9:16 a todo lo alto); el resto
del kiosko se acomoda en los 1312 px que quedan. El cliente lo cambia (vuelve a esperar
aprobación), lo quita cuando quiera o, vencido, lo vuelve a mandar igual.

- `src/anuncio.rs` — deja la imagen en 1080 × 1920 JPEG con el crate `image`: respeta la rotación
  de la cámara (EXIF) y, si no es 9:16, no la recorta: la pone completa sobre la misma foto
  borrosa (se emborrona en 54 × 96 y se agranda, mucho más rápido). Los 8 colores del texto y en
  qué va el anuncio (`situacion`). Con pruebas.
- `src/db/anuncios.rs` — poner o reemplazar (devuelve la imagen anterior para borrarla), volver a
  mandar, quitar, lo del kiosko (sin datos del cliente) y la limpieza.
- `src/routes/anuncio.rs` — `/anuncio` (pantalla, guardar, quitar, volver a mandar, su imagen) y
  `/kiosko/anuncios` + `/kiosko/anuncio/{id}` (solo con la cookie del kiosko, solo lo aprobado y
  vigente). Limpieza diaria: rechazados y vencidos de hace más de 30 días, con su imagen.
- `src/minio.rs` — `Minio::en(bucket)`: el mismo usuario en el bucket `papeleria-anuncios`.
- `src/avisos.rs` — `anuncio_nuevo`: aviso al celular de la papelería (ntfy `papeleria-kiosko`)
  cuando hay un anuncio por aprobar.
- `templates/app/anuncio.html`, `anuncioForm` y `ajustarTexto` en `static/app/app.js` — vista
  previa como sale en la pantalla; la letra del texto es la más grande con la que todo cabe sin
  partir palabras, igual en el celular y en el kiosko.
- `templates/monedero/app.html` — renglón "Tu anuncio en la tienda" con en qué va (solo con sesión).
- `templates/app/kiosko.html`, `kioskoAnuncios` — la franja: un anuncio cada 10 s con la etiqueta
  "Anuncio de un cliente" y, al final de cada vuelta, la invitación "¿Tu anuncio aquí?".
- `static/css/app.css` — las medidas del kiosko pasan de `vw` a `cqw` (ancho de `.kiosko`, que es
  contenedor) para caber junto a la franja.

En producción: el script `/home/ro/srv/share/xplaya-minio/anuncios-minio.sh` (crea el bucket y amplía la política de
`xplaya-impresiones`) y la tabla de `dbchanges/updates.sql` del POS.

Probado con el servidor local: a 360 px poner un texto (vista previa, color, "Esperando
aprobación"), aprobarlo en la BD y verlo en el kiosko a 1920 × 1080 junto con la invitación y la
vista de WhatsApp; quitarlo. La subida de imagen no se probó contra MinIO de desarrollo (falta el
bucket); el redimensionado, con las pruebas de `anuncio.rs`.

---

## Monedero: menú de renglones, historial aparte y pagar por transferencia (2026-11-17)

La pantalla del monedero crecía hacia abajo con el historial. Ahora arriba queda lo de la caja
(tarjeta y "Mostrar en caja") y lo demás va en renglones: **Historial** (su propia vista con
"← Monedero", en la misma página), **Pagar por transferencia** (hoja con el beneficiario y la
CLABE, cada uno con su botón "Copiar", el banco y "Mandar comprobante por WhatsApp") y
**Términos del monedero**. La barra de abajo no cambia. Sin Dimo (nadie lo usa).

- `templates/monedero/app.html` — los renglones, la vista del historial y la hoja de pago. Sirve
  igual en la app y en xplaya.com/monedero/{id}.
- `static/app/app.js` — `monederoPantalla` (QR, hoja de pago, vista, `copiar` con respaldo para
  navegadores sin API de portapapeles).
- `static/css/app.css` — `.mon-menu`, `.mon-fila`, `.pago-*`.

Probado con el servidor local a 360 px: copiar el nombre y la CLABE (llegan al portapapeles),
cerrar la hoja, abrir el historial y regresar.

---

## "Imprime tú solito" sin QR: se manda desde donde esté y se ven los pasos (2026-11-17)

El cliente ya no tiene que estar en la tienda: "Mandar a la papelería" pasa sus trabajos a la caja
desde donde esté, como si los mandara por WhatsApp (la caja decide cuándo imprimirlos). Cada
trabajo enseña en qué va: mandado → imprimiendo → listo → recogido. Va junto con el POS
(Impresoras → Impresiones y "Dejar para después"): subir los dos.

- `src/db/impresiones.rs` — `Trabajo::paso` (con pruebas): lo dejado para después o con error se
  ve como "mandado"; listo / recogido sale del pedido (`de_cliente` trae su estatus: Nuevo = listo,
  Pagado = pagado y pasa después, Entregado o borrado por el POS = recogido) y `fecha_impreso`.
- `src/routes/imprimir.rs` — sin `en_tienda`; la lista ordenada (por mandar, listo, en camino,
  recogido) y el botón dice qué archivos manda.
- `templates/app/imprimir.html`, `templates/app/imprimir_renglon.html` (nuevo), `static/css/app.css`
  — la línea de pasos (cabe en 360 px).
- `src/routes/kiosko.rs`, `src/main.rs` — el QR del kiosko ya no marca "en la tienda" (vuelve a
  ser solo el alta); fuera `AppState::en_tienda`. `templates/app/kiosko.html`, `static/app/app.js`
  — sin el cuadro Imprimir.
- `src/autoservicio.rs` — la notificación dice "Pasa por tus hojas a la papelería y págalas en caja."

Probado con el servidor local y la BD de desarrollo (cliente de prueba con sesión, ya borrado):
un trabajo en cada paso a 360 px y "Mandar a la papelería".

---

## Kiosko: cuadro "Imprimir" (2026-11-17)

El QR que prueba que el cliente está en la tienda solo salía en la ficha de la app y en la vitrina:
quien llegaba a imprimir no sabía cuál escanear. Ahora hay un cuadro **Imprimir** a lo ancho,
debajo de la app, con una vista "Imprime tú solito": tres pasos y el mismo QR firmado de 30 s (con
sesión lleva a mandar los trabajos; sin la app, al alta). Solo con el autoservicio encendido
(`imprimir_activo`).

- `templates/app/kiosko.html` — el cuadro y la vista `imprimir`.
- `static/app/app.js` — `nuevoQr` pinta también `qrImprimir`.
- `static/css/app.css` — `.cuadro-imprimir`, `.kiosko-pasos`.

Probado con el servidor local en 1920×1080: el cuadro, la vista con su QR y "← Inicio" desde
Imprimir, WhatsApp, Buscar y la app.

---

## Kiosko: vuelve el botón "← Inicio" (2026-11-17)

Al quitar la vista "Nuestro WhatsApp" (467924a) quedaron tres líneas sueltas con un `</div>` de
más en `templates/app/kiosko.html`: cerraba el contenedor de Alpine antes de tiempo y dejaba fuera
"← Inicio" y la pantalla "¡Listo!" de "Te agregamos a WhatsApp". Se quitaron las líneas sueltas.

---

## Aviso de privacidad: datos de Google (2026-11-14)

- `templates/pages/terminos.html` — "Datos recabados" suma lo que se guarda al entrar con Google;
  la sección lleva `id="privacidad"`, que es la URL del aviso en Google Cloud
  (`xplaya.com/terminos#privacidad`).

---

## Entrar con Google si el celular no puede crear passkey (2026-11-14)

Un Android sin PIN ni bloqueo de pantalla no puede crear passkeys. En el alta, si la huella falla
aparece **Entrar con Google**; el celular queda ligado con la cuenta de Google (su `sub` en
Keycloak) en lugar de la passkey. En la portada, **Entrar con Google** recupera la sesión.

- `src/google.rs` — habla con Keycloak: URL de ida (PKCE, `kc_idp_hint=google`), cambio del `code`
  por el token y `userinfo` para el `sub`.
- `src/routes/google.rs` — `POST /google/alta`, `GET /google/entrar`, `GET /google/regreso`. Lo
  pendiente vive en memoria con el `state` como id (no en cookie: en iPhone la app no comparte
  cookies con Safari).
- `src/db/celulares.rs` — `ligar` recibe `Acceso::Passkey` o `Acceso::Google`; `por_google`.
- `src/routes/alta.rs` — `completar_alta` recibe el `Acceso` (lo usa también Google).
- `templates/app/alta.html`, `static/app/app.js` — el botón aparece solo si la huella falló.
  `templates/app/inicio.html` — "Entrar con Google" en la portada.
- `src/routes/app.rs` — `inicio_con_error`: los avisos de la portada van codificados (con "á" tal
  cual salían como "Ã¡"). Arregla también los de `/entrar`.
- BD (repo del POS): `ClienteCelulares.GoogleSub`; `CredentialId`/`Passkey` opcionales.
- Variables `KEYCLOAK_ISSUER` / `KEYCLOAK_CLIENT_SECRET`.

---

## Kiosko: sin WiFi ni "Nuestro WhatsApp"; "Buscar por número" en una celda (2026-11-14)

Los QR del WiFi y del WhatsApp de la papelería ya están impresos en la pared, así que salen del
menú. Quedan la app (a lo ancho) y cuatro cuadros de 2×2.

- `templates/app/kiosko.html` — sin los cuadros ni las vistas `wifi` y `contacto`; "Buscar por
  número" sin `cuadro-ancho`.
- Lo que solo servía para eso: `texto_qr_wifi` (`src/kiosko.rs`), el QR en
  `src/routes/kiosko.rs`, `KIOSKO_WIFI_SSID` / `KIOSKO_WIFI_PASSWORD` (`src/config.rs`,
  `.env.example`, `CLAUDE.md`), `.kiosko-wifi` y `.ico-whatsapp` del CSS y
  `static/img/kiosko/whatsapp-contacto.png`. En k3s-manifests también salen del deployment y del
  SealedSecret.

---

## "Imprime tú solito": elegir páginas con miniaturas (2026-11-14)

Las páginas del trabajo se eligen como en el diálogo de impresión de Android: una cuadrícula con
la miniatura de cada página y su palomita (todas marcadas al principio). Arriba: "4 de 12",
**Todas**, **Ninguna** y **Escribir rango** (el campo de antes, como atajo para PDFs largos).

- `src/routes/imprimir.rs` — `GET /imprimir/{id}/pdf`: el PDF desde MinIO, solo para el dueño
  (el bucket es privado). Registrada en `src/routes/app.rs`.
- `static/app/app.js` — `opcionesImpresion`: pdf.js (jsDelivr, sin build step) dibuja las
  miniaturas una a la vez y solo las que se ven (IntersectionObserver), para que un PDF de 500
  páginas no trabe el celular. Palomitas y rango escrito se mantienen iguales (`leerPaginas` /
  `escribirPaginas`); sin páginas marcadas, "Elige al menos una página." y Listo desactivado. El
  documento de pdf.js vive fuera del objeto de Alpine (su proxy rompe los campos privados).
- `templates/app/imprimir_trabajo.html`, `static/css/app.css` — la cuadrícula.

El servidor recibe el mismo texto de páginas ("1-3,5"; vacío = todas): cuentas e impresión no
cambian. Probado con un PDF de 12 páginas en Chromium sin cabeza (fuera de la app): Todas,
Ninguna, tocar páginas y escribir el rango.

---

## "Imprime tú solito": imprimir en la Kyocera, paso 4 (2026-11-14)

Lo último del autoservicio: lo que la caja autoriza en el POS sale en la impresora elegida y queda
un pedido para cobrarlo.

- `src/ipp.rs` — IPP mínimo, sin CUPS: Print-Job con el PDF (rangos de páginas, copias
  intercaladas, una cara / doble cara por el lado largo, carta, b/n, ajustar a la hoja),
  Get-Job-Attributes para saber cuándo terminó y Get-Printer-Attributes para explicar un error
  ("Sin papel.", "Papel atorado."…). `ipps://` va por https aceptando el certificado propio de la
  impresora (la MA5500ifx solo acepta https). Pruebas al final; contra las impresoras, solo
  lectura: `cargo test consulta_las_kyocera -- --ignored`.
- `src/autoservicio.rs` — escucha NATS `impresion.autorizada` y revisa la tabla cada 20 s; toma el
  trabajo (`autorizado` → `imprimiendo`, una sola sentencia: nunca se imprime doble), baja el PDF,
  lo manda, espera hasta 15 min, y crea el pedido + `impreso` en una transacción. Avisa al cliente
  ("Tu impresión está lista") y lo registra en Loki (`job="papeleria-pc"`, `tipo="impresion"`).
  Si falla: `error` con el motivo. Los que se quedan `imprimiendo` 20 min (xplaya se reinició a
  medias) pasan a `error` para que la cajera revise: nunca se reimprimen solos.
- `src/db/impresiones.rs` — `impresoras` (de `Settings`), `autorizados`, `tomar`, `terminar`,
  `fallar`, `interrumpidos`. `src/db/pedidos.rs` — `insertar_autoservicio` (Origen 2).
- `src/impresion.rs` — `rangos` (las páginas guardadas para la impresora). Config: `LOKI_URL`.

Probado con hojas de verdad desde desarrollo (sin Loki): K2, 2 páginas a doble cara (1 hoja, la 1 al
frente y la 2 atrás), y K1, solo la página 1 (1 hoja). Las dos salieron bien y cada una dejó su
pedido (`BN REFK2 × 2`, `BN REFK1 × 1`). Datos de prueba borrados.

---

## "Imprime tú solito": estado `autorizado` (2026-11-14)

El POS (paso 3) marca `autorizado` cuando la caja elige impresora; xplaya lo pasará a
`imprimiendo` al tomarlo (paso 4). En la app se ve igual que "Imprimiendo…" y la lista se sigue
recargando sola: `templates/app/imprimir.html`, `en_curso` en `src/routes/imprimir.rs` y el
comentario de estados en `src/db/impresiones.rs`.

---

## "Imprime tú solito": autoservicio de impresión en la app, paso 2 (2026-11-14)

El cliente con la app sube **un PDF** (b/n, carta), elige páginas, copias y una cara / doble cara,
y ve el total: **$1 por cara impresa** (la doble cara solo ahorra papel). El trabajo queda
`preparado`. En la tienda escanea el QR del kiosko y "Mandar a imprimir" lo pasa a la caja
(`por_autorizar`). La caja lo autoriza en el POS e imprime (pasos 3 y 4, pendientes). Plan
completo en `ROADMAP.md`; tabla `TrabajosImpresion` y decisión en el repo del POS.

- `src/impresion.rs` — las cuentas, sin BD ni red: `calcular` (rango "1-3, 5" ordenado y sin
  repetir, caras, hojas, total, topes de 100 hojas y 50 copias), `contar_paginas` (lopdf; rechaza
  lo que no es PDF y los PDF con contraseña), `nombre_archivo`. Pruebas al final.
- `src/minio.rs` — cliente S3 mínimo con la firma SigV4 a mano (como imagina): subir, bajar,
  borrar en `papeleria-impresiones`. Prueba contra el MinIO de desarrollo:
  `cargo test minio -- --ignored`.
- `src/db/impresiones.rs` — precio de la cara (producto de la primera impresora de
  `AUTOSERVICIO_IMPRESORAS`), crear, listar, cambiar opciones (solo `preparado`), quitar, mandar,
  vencidos. Fechas en hora de la tienda.
- `src/routes/imprimir.rs` — `/imprimir` (lista y subir; límite de cuerpo de 20.5 MB solo en esa
  ruta), `/imprimir/{id}` (opciones), `/imprimir/{id}/cuenta` (la cuenta al instante, la misma que
  se guarda), `/imprimir/{id}/quitar`, `/imprimir/mandar` (solo "en la tienda") y `limpiar` (cada
  hora: borra los PDF de 7 días y marca `vencido`).
- `src/routes/kiosko.rs` — `escaneo`: con sesión (y MinIO) el QR marca al cliente "en la tienda"
  30 min (`AppState::en_tienda`) y lo lleva a `/imprimir`; sin sesión, el alta de siempre.
- `templates/app/imprimir.html`, `imprimir_trabajo.html`, pestaña 🖨️ Imprimir en `nav.html` (solo
  con MinIO, global `imprimir_activo`), `subirPdf` y `opcionesImpresion` en `app.js`, `.imp-*` en
  `app.css`. Mientras algo está en la caja, la lista se recarga cada 10 s.
- Config: `MINIO_ENDPOINT`, `MINIO_USUARIO`, `MINIO_PASSWORD`. **Sin usuario todo esto está apagado**,
  y así se queda en producción hasta los pasos 3 y 4.

Probado en Chromium (Pixel 7) contra la BD y el MinIO de desarrollo: archivo que no es PDF,
subir, página inexistente (botón deshabilitado), 3 copias a doble cara, mandar sin QR (rechazado),
QR del kiosko → "Estás en la papelería" → mandar (`por_autorizar` en la BD, PDF en MinIO), quitar
(fila y PDF borrados), sin sesión → portada, y la limpieza de 7 días (estado `vencido`, PDF
borrado). Datos de prueba borrados.

---

## Kiosko: la app destacada, ficha de tienda de apps y en la vitrina (2026-11-14)

"Instala nuestra app" era un cuadro más del menú, y casi nadie toca el kiosko. Tres cambios:

- **Menú** (`templates/app/kiosko.html`, `.cuadro-app` en `app.css`): la app va primero, a lo
  ancho, con el fondo oscuro de la marca, el ícono de la app instalada
  (`web-app-manifest-*.png`, el logo oficial), "Monedero · Pedidos · Gratis" y "Obtener".
- **Ficha de la app** (vista `app`, `.ficha-*`, `.cel-*`): como en una tienda de apps: ícono,
  nombre, chips (Gratis · Sin anuncios · Entras con tu huella) y tres celulares dibujados con
  datos de ejemplo (monedero, cuenta en caja, tienda con fotos reales de la vitrina). El QR del
  alta va donde iría "Obtener". Sin estrellas ni descargas: serían inventadas.
- **Vitrina** (`conLaApp` y `esApp` en `static/app/app.js`): después de cada 5 productos sale la
  diapositiva de la app ("Ve tu cuenta en tu celular mientras te cobramos") con el mismo QR;
  cuando pasa sola se queda ~30 s en lugar de 15. El QR se pinta en los dos lugares
  (`qrApp`, `qrVitrina`) cada vez que cambia.

Probado en Chromium a 1920×1080 con los productos de dev (29 diapositivas: 24 productos + 5 de
la app) y el QR en la vitrina.

---

## Saldo por teléfono: solo lo básico, sin llevar al monedero completo (2026-11-14)

Antes, quien escribiera cualquier teléfono en `xplaya.com/saldo` o en "Entrar" de la app llegaba
al monedero completo (`/monedero/{guid}`): nombre, historial, QR de caja y la liga permanente.
Ahora la consulta por teléfono solo muestra el saldo y lo que vence, con el nombre tapado.

- `src/routes/monedero.rs` — `SaldoPorTelefono`, `saldo_por_telefono` (reusa
  `db::monedero::monedero`) y `nombre_oculto` ("NAYELLI ALVARADO CRUZ" → "N••••• A."; siempre
  cinco puntos para no dar el largo; prueba al final). `saldo_get` (`?p=` de las tarjetas impresas
  del POS) y `saldo_post` pintan el resultado en la misma página, sin redirigir: el teléfono del
  formulario no queda en la URL.
- `templates/monedero/saldo.html` — el resultado arriba, con la invitación a la app, y "¿Otro número?".
- `src/routes/app.rs` — `POST /entrar` pinta el saldo en la portada (`render_inicio`) en lugar de
  redirigir; `templates/app/inicio.html` + `.saldo-tel` en `app.css`. "Entrar con huella" queda
  como botón principal y "Ver mi saldo" como secundario.
- `/monedero/{guid}` no cambia: llega por el recibo ("Ver Monedero") y por el POS (Clientes).

Probado en Chromium con un cliente de dev con saldo: sitio, `?p=` y app (claro y oscuro); en
ninguno aparece el GUID ni el teléfono, y un número sin monedero sigue dando el aviso de siempre.

---

## Recibo: invitación a la app más visible y alta más corta (2026-11-14)

El 99 % de los recibos llega por WhatsApp como liga. Cuatro cambios para que más clientes pasen
de ahí a la app, sin tocar el POS:

1. **Vista previa que invita.** `src/routes/monedero.rs` — `descripcion_recibo`: mientras el enlace
   de alta está vivo (y el celular no está ligado), la vista previa dice "… Toca para ver tu ticket
   y guardar tu monedero en el celular 👆" (sin monedero: "… crear tu monedero gratis"). La
   plantilla usa `{{ descripcion }}` en las tres metas.
2. **Botón de la app junto al saldo.** `templates/monedero/recibo.html` — "📱 Guarda tu monedero en
   el celular", grande, debajo del saldo; "Ver Monedero" (la web) pasa a `is-light`. Si el
   celular ya está ligado, sigue abajo "¿Cambiaste de celular? Liga el nuevo".
3. **Sin casilla de términos para quien ya aceptó el programa.** `Quien::Cliente { id, acepto }`
   lleva `Clientes.AceptoPrograma` (`db::altas::cliente_alta`), y `Quien::pide_terminos` decide en
   la página (`alta.html`, `crearAcceso(ticket, yaAcepto)`) y en `registro_fin`. Quien ya aceptó
   ve solo "Al continuar aceptas los términos". **Ojo:** a quien es cliente pero no ha aceptado
   (en dev, 108 de 219) se le sigue pidiendo la casilla: es su "quiero participar".
4. **Una semana en lugar de 24 horas.** `src/db/altas.rs` — `DIAS_ENLACE = 7`. `fn_asignar_cliente_venta`
   (POS) no tiene límite de tiempo. Los 4 dígitos y los 5 intentos no cambian.

También: `alta::primer_nombre` pone solo la inicial en mayúscula ("NAYELLI ALVARADO CRUZ" →
"Nayelli"); la copia de `app.rs` del commit anterior se quitó y la prueba pasó a `alta.rs`.

Probado en Chromium con una venta de dev del 9 nov (más de 24 h): sin cliente, con cliente que ya
aceptó (sin casilla, botón habilitado) y con cliente que no (casilla, botón deshabilitado); el
botón cabe en un iPhone SE. La venta se dejó como estaba.

---

## Pantalla "¡Listo!" al terminar el alta (2026-11-14)

Al crear la passkey (kiosko o recibo), el cliente ya no cae directo al monedero: ve una pantalla
con lo que falta para tener la app completa, en filas que se ponen ✅ (Acceso creado · Avisos ·
Instalar la app) y "Ir a mi monedero" abajo. Antes, instalar y avisos eran dos avisos chiquitos
arriba del monedero que se cerraban con ✕, y el cliente se quedaba a medias. Pensada para que la
cajera, con el celular del cliente en la mano, vea de un vistazo si quedó completo.

- `src/routes/alta.rs` — `registro_fin` manda a `/listo` en lugar de `/`.
- `src/routes/app.rs` — `GET /listo` (sin sesión, a la portada) y `primer_nombre` ("NAYELLI
  ALVARADO CRUZ" → "Nayelli"; prueba al final).
- `templates/app/listo.html` + `static/css/app.css` (`.listo-*`) — la pantalla. En iPhone sin
  instalar, la fila de avisos dice que primero instale (ahí solo llegan con la app instalada).
- `static/app/app.js` — la lógica de instalar y de avisos sale a funciones (`pedirInstalar`,
  `estadoAvisos`, `activarAvisos`, `suscribirAvisos`…) que usan los avisos de siempre
  (`instalarApp`, `avisosApp`) y la pantalla nueva (`listoApp`).

Probado en Chromium (Pixel 7 e iPhone 14, claro y oscuro) con un celular de prueba en la BD de
desarrollo: sin sesión redirige a `/`; "Activar" (permiso simulado) pone la fila en ✅ y manda la
suscripción; "Instalar" enseña los pasos a mano; los avisos del monedero siguen igual.

---

## Videos de TikTok incrustados: página del producto y kiosko (2026-11-13)

Los videos de `UrlContentProductos` que son ligas completas de TikTok
(`tiktok.com/@cuenta/video/123…`) ya no se abren en otra pestaña: se ven dentro de la página con
el reproductor oficial de TikTok (`tiktok.com/player/v1/{id}`, developers.tiktok.com/doc/embed-player).
Las demás ligas (cortas `vm.tiktok.com/…`, otras páginas) siguen como botón "Ver video".

- `src/models/producto.rs` — `Video` (liga + reproductor), `tiktok_id` (el número del video;
  pruebas al final) y `reproductor_tiktok` (sin recomendados de otras cuentas, descripción ni
  música; en el kiosko arranca solo y se repite). `ProductoDetalle.videos` pasa a `Vec<Video>`.
- `templates/productos/detalle.html` + `static/css/main.css` — reproductor vertical (9:16, máx.
  340 px), `loading="lazy"`, con "Ver en TikTok" abajo.
- `src/routes/kiosko.rs` — `ProductoKiosko.videos`: los reproductores del kiosko.
- `static/app/app.js`, `templates/app/kiosko.html`, `static/css/app.css` — el producto buscado
  tiene "medios": primero los videos, luego las fotos, con las mismas flechas y contador
  (▶️ / 📷). En un video el avance automático se detiene. TikTok no tiene parámetro para arrancar
  sin sonido (solo `force_muted`, que no deja subirlo): al recibir `onPlayerReady` por
  `postMessage` se le manda `mute` y `play`.

Probado en Chromium: kiosko a 1920×1080 con el NID 1077 (llegan `onPlayerReady`, `onMute: true`
y `onStateChange: 1`, y se queda en el video) y la página del producto a 390 px.

---

## Kiosko: buscar un producto por su número (2026-11-12)

Un cuadro nuevo en el inicio del kiosko, "🔎 Buscar por número" (ocupa las dos columnas, abajo),
para enseñarle un producto a un cliente. Abre el teclado de la pantalla; con el NID muestra el
producto en grande: fotos (con puntos para cambiar de foto), nombre, precio, unidad,
presentaciones, si hay en tienda y "Pídelo en caja con el número…". Si está agotado dice
"Agotado por ahora · pregunta en caja" y no invita a pedirlo. Un número que no existe da
"No encontramos el número…". Como las demás vistas, vuelve al inicio tras un minuto sin tocarla.

- `src/routes/kiosko.rs` — `GET /kiosko/producto/{nid}`: solo para la pantalla activada (como la
  vitrina); reusa `db::productos::detalle` (los datos de la página del producto) y manda solo
  "hay" o no, sin la cantidad.
- `templates/app/kiosko.html` — vistas `buscar` y `producto`.
- `static/app/app.js` — `kioskoPantalla`: `nid`, `teclaNid`, `buscarNid`.
- `static/css/app.css` — `.cuadro-ancho` y los estilos del producto buscado.

Probado en Chromium a 1280×800 con el teclado de la pantalla: con varias fotos, sin foto y con
presentaciones, agotado y un número que no existe. Sin el token del kiosko, 403.

Después: con varias fotos solo había unos puntos chicos abajo y no se notaba. Ahora tiene flechas
‹ › como la vitrina, tocar la foto pasa a la siguiente, las fotos pasan solas cada 4 s si nadie
toca la pantalla y un contador "📷 2 / 3". Además precarga las fotos y quita las que no cargan: 5
de las 918 fotos registradas en `FotosProductos` no están en MinIO (`cntnt.xplaya.com` responde
500), en los NID 221, 1230 y 1679; con eso ya no quedan flechas que no cambian nada.

---

## Cuenta de la caja en vivo: aviso por Web Push con la app cerrada (2026-11-12)

Paso 5. Si el cliente no tiene la app en pantalla, le llegan dos notificaciones: al abrirse la
cuenta ("La caja te está atendiendo · Toca para ver tu cuenta en vivo") y al cobrar ("¡Gracias por
tu compra! · Ganaste $0.68 en tu monedero · tu saldo: $0.82", o el total si no hubo monedero).
Una sola vez cada una por venta, aunque la caja quite y vuelva a poner al cliente. Las dos usan el
`tag` `cuenta-caja`: la del cobro reemplaza a la otra y vuelve a sonar. Al tocarlas abre la app.

- `src/caja.rs` — `por_avisar` decide qué aviso toca (pruebas al final); `Cajas::en_pantalla`
  cuenta las conexiones SSE abiertas por cliente; `avisar` arma el texto (el del cobro con
  `db::monedero::recibo`, igual que el recuadro) y lo manda aparte, sin frenar los mensajes.
- `src/push.rs` — `mandar_del_momento`: sin fila en `Notificaciones` ni reintentos, urgencia alta y
  caducidad de 10 min (el TTL del servicio de push: si el celular está apagado más, ya no llega).
- `src/routes/cuenta.rs` — la conexión SSE guarda un `EnPantalla` mientras está abierta.
- `static/app/app.js` — `cuentaCaja` cierra la conexión cuando la app se va a segundo plano y la
  reabre al volver (así "conectada" = "en pantalla", y ahorra batería); al conectarse cierra la
  notificación de la caja si había.
- `static/app/sw.js` — usa `tag` (y `renotify`) cuando el aviso lo trae.

Probado en local con un servicio de push falso y el aviso descifrado: de 6 mensajes (abierta,
abierta, cerrada, abierta, cobrada, cobrada) salen 2 avisos; con la app en pantalla, 0; en segundo
plano (Chromium con `visibilitychange`), 1.

---

## Cuenta de la caja en vivo: el recuadro en la app (2026-11-12)

Paso 4. Con sesión de passkey, el monedero de la app muestra arriba "Tu cuenta en la caja" mientras
la caja le está cobrando: renglones, total y un punto "En vivo". Al cobrar cambia a "¡Gracias por
tu compra!" con lo que ganó y usó de monedero, su saldo nuevo y la liga al recibo. Desaparece si la
caja quita al cliente, a los 5 min de cobrar o a los 15 min sin cambios.

- `templates/monedero/app.html` — el recuadro (`x-data="cuentaCaja()"`), solo con sesión.
- `static/app/app.js` — `cuentaCaja`: abre `EventSource('/cuenta/eventos')`; al llegar una
  `cobrada` pide el premio una sola vez por venta.
- `src/routes/cuenta.rs` — `GET /cuenta/venta/{venta_id}`: ganado, usado y saldo con el mismo
  cálculo del recibo (`db::monedero::recibo`); solo para ventas del cliente de la sesión (si no, 404).
  `sin-cuenta` ahora manda `data: null`: EventSource no entrega eventos sin datos, y con datos
  vacíos el recuadro nunca se quitaba.
- `static/css/app.css` — sección "Cuenta de la caja en vivo", con los colores de la app (claro y oscuro).

Probado en Chromium a 390 px, claro y oscuro, con NATS en podman: abierta → cobrada (premio real de
la BD dev) → cerrada lo quita.

Después: la tarjeta de saldo también se pone al día al cobrar. `cuentaCaja` manda el evento
`saldo-monedero` con el saldo del premio y `.card-amount` lo escucha (`templates/monedero/app.html`).
El historial sigue igual hasta recargar.

---

## Cuenta de la caja en vivo: xplaya la recibe y la reparte por SSE (2026-11-12)

Paso 3 de "la cuenta en vivo en la app". El POS publica en NATS `caja.cuenta` la cuenta completa
en cada cambio (rama `feature/cuenta-en-app` del POS). xplaya la escucha, guarda en memoria la
última de cada cliente y se la empuja a los celulares de ese cliente. Todavía no hay pantalla:
eso es el paso 4.

- `src/caja.rs` — `Cajas`: la última cuenta por cliente y un canal `broadcast` para avisar a
  los celulares conectados. Solo lee el encabezado (`cuentaId`, `secuencia`, `clienteId`,
  `estado`); el JSON se reenvía tal cual. Descarta mensajes viejos de la misma cuenta. Una
  abierta sin cambios vence a los 15 min (venta cancelada) y una cobrada se muestra 5 min.
  `vigilar` abre su propia conexión a NATS (NATS núcleo, sin stream). Pruebas al final del archivo.
- `src/routes/cuenta.rs` — `GET /cuenta/eventos` en app.xplaya.com, solo con sesión de passkey.
  Es SSE (Server-Sent Events): la petición se queda abierta y manda `cuenta` (el JSON) o
  `sin-cuenta`. Al conectarse llega lo que hay en ese momento.
- `src/main.rs` — `AppState.cajas` y arranque de `caja::vigilar`.
- k3s-manifests: permiso de `pos` para publicar y de `xplaya` para escuchar `caja.cuenta`
  (reiniciar NATS).

Probado en local con NATS en podman: sin sesión da 401, no llegan cuentas de otro cliente, se
ignora un mensaje viejo y al reconectarse llega la cobrada.

---

## Avisos: "Agrégame a WhatsApp" del kiosko al celular de la papelería (2026-11-12)

La solicitud de WhatsApp de la pantalla del kiosko también llega por ntfy (topic
`papeleria-kiosko`): "WhatsApp: 984 123 4567", si ya es cliente (con su nombre) o nuevo, y un
botón que abre WhatsApp con el mismo saludo del POS. Sin cambios en NATS: el permiso ya existía.

- `src/avisos.rs` — `whatsapp_kiosko`.
- `src/routes/kiosko.rs` — `whatsapp` lo llama después de guardar la solicitud.

---

## Avisos: pedido nuevo en línea al celular de la papelería (2026-11-12)

Cada pedido en línea (carrito de xplaya.com, con o sin sesión, y tienda de la app) publica en NATS
`avisos.pedidos` → `nats-ntfy` → ntfy, topic `pedidos`: "Pedido en línea: Juan", número,
productos, total, teléfono y por dónde llegó. Tocarlo abre la cotización; botón a WhatsApp.

- `src/avisos.rs` — `pedido_nuevo`: en una tarea aparte lee la cotización
  (`db::monedero::cotizacion`, mismo total) y el cliente, y publica.
- `src/routes/carrito.rs`, `src/routes/tienda.rs` — lo llaman después de guardar el pedido.
- k3s-manifests: permiso de `xplaya` para publicar `avisos.pedidos` (reiniciar NATS).

---

## Aviso del kiosko por ntfy: verificado en producción (2026-11-12)

Solo documentación. Probado de punta a punta: kiosko → xplaya → NATS → `nats-ntfy` → ntfy →
celular → WhatsApp. En la tienda es más rápido que mandar el código desde WhatsApp Web en el POS.

- `APP_MONEDERO.md` ("Kiosko en la tienda") — anotado como verificado.
- `ROADMAP.md` (sin cambios) — los siguientes avisos por revisar están en "Avisos a la papelería".

---

## Avisos: el topic pasa a `papeleria-kiosko` (2026-11-12)

En ntfy, `dayana` solo lee `papeleria-*`; con `papeleria-kiosko` le llega a ella y a `ro` sin
tocar permisos de ntfy.

- `src/avisos.rs` — subject `avisos.papeleria-kiosko`. Docs al día.
- Infra en k3s-manifests: usuario NATS `ntfy`, permiso de `xplaya`, puente desplegado
  (`workloads/nats-ntfy/`).

---

## Kiosko: el código también llega al celular de la papelería por ntfy (2026-11-12)

Al crear una solicitud del kiosko, xplaya publica en NATS `avisos.papeleria` el JSON de ntfy:
nombre, teléfono, código y un botón que abre WhatsApp con el mismo mensaje del POS. El puente
`nats-ntfy` lo reenvía a ntfy. El aviso del POS sigue igual. Si NATS falla, solo va al log.

- `src/avisos.rs` — nuevo: conexión propia a NATS (`retry_on_initial_connect`, no espera a NATS al
  arrancar), arma el aviso y lo publica con acuse de JetStream en una tarea aparte.
- `src/db/kiosko.rs` — `crear` devuelve también el código.
- `src/routes/kiosko.rs` — `datos` manda el aviso; `src/main.rs` — `AppState.avisos`.
- Falta infra (k3s-manifests, "Eventos — NATS"): permiso a `xplaya` de publicar `avisos.papeleria`,
  stream `AVISOS`, consumer y usuario del puente, y desplegarlo. Hasta entonces el publish falla y
  solo queda el log.

---

## Alta desde el celular: pospuesta (2026-11-12)

Solo documentación. Por ahora el alta sigue siendo en la tienda (kiosko) o con el enlace del recibo.

- `APP_MONEDERO.md` — la sección "Alta desde el celular" queda marcada como **pospuesta**, con lo
  que haría falta si se retoma y lo ya decidido sobre solicitudes falsas (límites + horario).
- `nats-ntfy` se queda publicado pero sin desplegar, hasta que haya un caso real.

---

## Alta desde el celular: diseño y puente NATS → ntfy (2026-11-12)

Solo documentación en este repo; el puente vive en su propio repo.

- `APP_MONEDERO.md` — sección nueva "Alta desde el celular (en diseño)": el mismo flujo del kiosko
  para quien no está frente a la pantalla. El código le llega al celular de la papelería por ntfy
  y lo reenvía por WhatsApp; lo que comprueba el teléfono sigue siendo el código. xplaya publicará
  en NATS `avisos.papeleria` y el puente lo pasa a ntfy. Pendiente: cómo frenar solicitudes falsas.
- Repo nuevo [`rogithub/nats-ntfy`](https://github.com/rogithub/nats-ntfy) — puente genérico:
  subject `avisos.<topic>` → topic de ntfy, cuerpo = JSON de ntfy. Imagen ya publicada; falta
  desplegarlo (pendientes en `k3s-manifests/CLAUDE.md`, "Eventos — NATS").

---

## Kiosko: pasa de `chaac` a `cajita` (2026-11-11)

Solo documentación, sin código.

- `APP_MONEDERO.md` — el kiosko ahora corre en `cajita` (mismo autostart y `url.txt`); `chaac`
  queda como caja con touch. Su Elo no entrega la EDID, así que squeekboard sacaba un teclado
  enorme: `cmdline.txt` carga la EDID copiada de `cajita` con `drm.edid_firmware`.

---

## Notificaciones: "pedido listo" con Urgency high (2026-11-11)

Segunda prueba en producción: `enviada (nats)`, y 14.6 s de xplaya al celular (Android junta los
avisos de prioridad normal para ahorrar batería). Los `pedido_listo` ahora llevan el encabezado
Web Push `Urgency: high`, que pide entregarlos de inmediato; los de saldo siguen normales.

- `src/push.rs` — `entregar`: `Urgency: high` si `tipo = 'pedido_listo'`.
- `src/db/notificaciones.rs` — `Pendiente.tipo`.

---

## Notificaciones: fechas en UTC sin importar la sesión; el log dice quién entregó (2026-11-11)

En la primera prueba en producción el aviso llegó, pero `FechaEnviado - FechaCreado` dio
5 h 0 min 15 s: la fila se insertó desde pgAdmin, con la sesión en hora local, y `NOW()` en un
`TIMESTAMP` sin zona toma la zona de la sesión. xplaya veía la fila 5 h vieja y la revisión
periódica podía adelantarse a NATS.

- Repo del POS: `Notificaciones.FechaCreado DEFAULT (NOW() AT TIME ZONE 'UTC')` (schema y
  `updates.sql`). Aplicado en dev; falta producción.
- `src/db/notificaciones.rs` — macro `ahora!()`: todas las fechas y comparaciones con
  `NOW() AT TIME ZONE 'UTC'`, no `NOW()`.
- `src/push.rs` — `entregar(…, origen)`: log `{id} enviada (nats)` / `(revisión)`, `sin_app`,
  `fallida`.

Probado en dev: una fila insertada con `PGTZ=America/Mexico_City` nace con 0 s de antigüedad y la
revisión no la toma antes de un minuto; envejecida 2 min, sale `sin_app (revisión)`.

---

## Notificaciones: el service worker muestra los avisos (2026-11-11)

El celular muestra el aviso aunque la app esté cerrada y le cuenta al servidor cuándo llegó y
cuándo lo tocó el cliente (`FechaRecibido`, `FechaAbierto`).

- `static/app/sw.js` — `push`: muestra `{titulo, texto}` con el ícono de la app (`tag` = Id, para
  no duplicar) y hace POST a `/notificaciones/{id}/recibida`. `notificationclick`: abre la `url`
  (de la app o completa) o trae al frente esa ventana, y POST a `/abierta`.
- `src/routes/notificaciones.rs` — `recibida` y `abierta` (sin sesión: el service worker corre sin
  la app abierta; el UUID solo lo conoce ese celular). Siempre 204.
- `src/db/notificaciones.rs` — `marcar_recibida`, `marcar_abierta` (solo la primera vez cuenta;
  abrirla también la da por recibida).

Las actualizaciones de `sw.js` llegan solas: el navegador lo vuelve a pedir (`no-cache`) al abrir
la app, y con `skipWaiting` + `clients.claim` el nuevo toma el control enseguida.

---

## Notificaciones: xplaya entrega los avisos (NATS + revisión periódica) (2026-11-11)

xplaya ya manda por Web Push las filas `pendiente` de `Notificaciones`. Se entera por **NATS**
(JetStream): el POS publicará el Id en `notificaciones.nueva`, el stream `NOTIFICACIONES` lo
guarda y xplaya lo lee de su consumer `xplaya` (pull, ack explícito, 5 intentos). Stream,
consumer y usuarios: `k3s-manifests/CLAUDE.md`, sección NATS.

- `src/push.rs` — `entregar`: toma la fila (`FOR UPDATE SKIP LOCKED`, para que NATS y la
  revisión no la manden dos veces), busca el celular con suscripción, cifra y firma, y marca:
  `enviada`, `sin_app`, `fallida` (404/410 borran la suscripción del celular). 429/5xx/red:
  `Reintentar` → NAK a NATS (otra vez en 30 s). `vigilar`: arranca `escuchar_nats` (se reconecta
  cada 10 s si se cae) y `revisar` (cada 2 min: rezagadas de 1 min a 1 día; vence las de más de
  un día). Un mensaje que no es un Id se confirma y se descarta.
- `src/db/notificaciones.rs` — consultas de la tabla.
- `src/config.rs` — `NATS_URL`, `NATS_USUARIO`, `NATS_PASSWORD`. `Cargo.toml`: `async-nats`
  (solo `jetstream`, con `ring`: sin OpenSSL ni aws-lc) y `futures-util`.

Probado en local sin NATS: una fila de un cliente sin celular quedó `sin_app` en la revisión.

---

## Notificaciones: la app guarda la suscripción del celular (2026-11-11)

En Monedero, con sesión, aparece "🔔 Recibe avisos de tus pedidos y tu saldo" → **Activar** pide
el permiso del navegador, suscribe el celular con la llave pública VAPID y la manda al servidor,
que la guarda en `ClienteCelulares.Push*`. Todavía no se manda ningún aviso.

- `templates/app/avisos.html` — el aviso y sus estados: ofrecer, instalar (iPhone sin la app),
  bloqueado, listo. Con ✕ se oculta 30 días. Lo incluye `monedero/app.html` con sesión y llave.
- `static/app/app.js` — `avisosApp`: si ya aceptó, confirma la suscripción en silencio en cada
  visita; si la suscripción es de otra llave VAPID, la rehace (así un cambio de llave se cura solo
  cuando el cliente abre la app).
- `src/routes/notificaciones.rs` — `POST /notificaciones/suscripcion`: solo con sesión; valida
  endpoint HTTPS, `p256dh` de 65 bytes y `auth` de 16 antes de guardar.
- `src/db/celulares.rs` — `celular_de_sesion` y `guardar_push` (`FechaPush` cambia solo si
  cambia el endpoint).
- `src/main.rs` — global `vapid_publica` para los templates; `src/routes/app.rs` — `token_sesion`.

---

## Notificaciones: llaves VAPID (2026-11-11)

Primer paso de las notificaciones: xplaya lee la llave privada VAPID y deriva la pública (la que
la app le dará al navegador al suscribirse). Todavía no se manda nada.

- `src/push.rs` — `Vapid::desde_base64`: decodifica `VAPID_PRIVATE_KEY` y calcula la pública.
- `src/config.rs`, `src/main.rs` — `vapid_private_key` y `AppState.vapid` (`None` = apagadas); al
  arrancar se loguea la pública (con `RUST_LOG=info`).
- `Cargo.toml` — `web-push-native` (Rust puro) en lugar de `web-push`, que arrastra OpenSSL por su
  crate de cifrado y complicaría la compilación cruzada. `base64ct` para base64url.
- `.env.example`, `CLAUDE.md` — la variable y cómo generar la llave con `openssl`.

---

## Notificaciones: diseño y tabla en el POS (2026-11-11)

Diseño de las notificaciones de la app (pedido listo, abonos, vencimientos) en `APP_MONEDERO.md`,
sección Notificaciones: tabla `Notificaciones` como bandeja de salida + `LISTEN/NOTIFY`, y por qué
no NATS por ahora.

- Repo del POS, rama `feature/notificaciones-bd`: `Notificaciones`, columnas `Push*` en
  `ClienteCelulares` y trigger `tr_notificaciones_notify`. Aplicado en `dev_inventario_papeleria`.

---

## App: heatmaps de Umami (2026-11-11)

app.xplaya.com carga `recorder.js` de Umami para los **heatmaps** (toques y scroll, muestreo
100 %). El **replay está apagado** en la configuración del sitio en Umami: grabaría el nombre, el
saldo y el teléfono del cliente. Con el replay apagado, el script solo manda coordenadas de cada
toque, la URL y la profundidad de scroll; no manda el DOM.

- `templates/app/base.html` — `recorder.js` después de `script.js` (necesita su sesión).

Si algún día se enciende el replay, `maskLevel: "moderate"` solo oculta los campos de texto; el
nombre y el saldo se verían. Usar `strict` o `blockSelector`.

---

## App: su propio sitio en Umami (2026-11-11)

app.xplaya.com deja de contarse junto con xplaya.com en analytics.xplaya.com: tiene su propio
*Website ID*.

- `templates/app/base.html` — `data-website-id` de app.xplaya.com (`e301a863…`). El sitio sigue
  con `8ca14307…` en `templates/base.html`.

---

## Kiosko: la vitrina ya no pasa "Lleva tu monedero"; turnos descartados (2026-11-11)

La vitrina del inicio pasa **solo productos**. El QR del alta del monedero queda en "Instala
nuestra app", que dice lo mismo. Sin productos (sin red), la vitrina dice "Toca un cuadro de la
derecha 👉".

- `templates/app/kiosko.html` — se quitan la diapositiva del monedero y sus 4 pasos; aviso
  `vitrina-vacia`.
- `static/app/app.js` — `kioskoPantalla` sin `pasos` ni `diapositivas()`: `actual()` es el
  producto en curso o `null`; `nuevoQr` solo llena `qrApp`.
- `static/css/app.css` — fuera `.vitrina-monedero` y `.kiosko-pasos`; nuevo `.vitrina-vacia`.
- `ROADMAP.md` — sistema de turnos descartado: solo hay fila 2 o 3 días al año.

---

## Kiosko: la vitrina mueve el inventario lento (2026-11-10)

En lugar de productos al azar, la vitrina elige al azar entre los **100 productos con foto que no
se han vendido en 60 días y más dinero tienen parado** (existencia × precio); si no alcanzan,
completa con productos al azar. En desarrollo había 574 lentos con foto (≈ $485 mil a precio de
venta) y 1,316 sin foto (≈ $796 mil).

- `src/db/productos.rs` — `vitrina`: CTE `vendidos` (ventas de los últimos 60 días), `lentos`
  (top 100 por existencia × precio), `elegidos` al azar y `relleno`. Responde en ~0.1 s.

---

## Kiosko: ícono de WhatsApp en "Nuestro WhatsApp" y vitrina sin QR por producto (2026-11-10)

- Vitrina: se quitó el QR de cada producto (abría la tienda de la app, donde quien no tiene
  cuenta no puede terminar el pedido). Ahora dice "Pídelo en caja": los productos están detrás
  del mostrador. `GET /kiosko/vitrina` ya no genera QR (devuelve los productos tal cual).
- "Pídelo en caja con el número 1967": muestra el NID del producto para buscarlo rápido en el POS.

- `templates/app/kiosko.html` — el cuadro "Nuestro WhatsApp" usa el ícono de WhatsApp (SVG de
  Font Awesome Free 6.5.0, CC BY 4.0, el mismo que usa xplaya.com) en verde WhatsApp, en lugar
  del emoji 📱. "Te agregamos a WhatsApp" se queda con 💬 para no confundirlos.
- `static/css/app.css` — `.ico-whatsapp`.

---

## Kiosko: "Instala nuestra app" en lugar de "Haz tu pedido" (2026-11-10)

Todo lo de la Elo empuja a activar el monedero: el cuadro "Haz tu pedido" (QR a
`xplaya.com/productos`) pasa a ser "Instala nuestra app", con el mismo QR del monedero (alta con
código por WhatsApp). Al terminar, la app ya tiene la tienda y le ofrece instalarse.

- `templates/app/kiosko.html` — vista `app` (ventajas: monedero, pedidos sin escribir datos,
  entrar con huella) con `x-ref="qrApp"`; fuera la vista `tienda`.
- `static/app/app.js` — `nuevoQr` pinta el mismo QR en la vitrina y en `qrApp`.
- `src/routes/kiosko.rs` — fuera `tienda_qr`. `static/css/app.css` — `.kiosko-ventajas`.

---

## Salir del carrito de xplaya.com y pasos de instalar por navegador (2026-11-10)

- La sesión que crea "Entrar con huella" en el carrito de xplaya.com dura un año y es aparte de
  la de la app (el "Salir" de la app no la cierra). `templates/carrito/index.html` suma "¿No eres
  tú? Salir"; `src/main.rs` registra `POST /salir` en el sitio con `app::salir_sitio`, que cierra
  la sesión y regresa al carrito (`cerrar_sesion_y_volver`, compartida con el "Salir" de la app).
- `templates/app/instalar.html` y `static/app/app.js` — los pasos a mano en Android nombran el
  menú del navegador que se usa: Firefox (⋮), Samsung Internet (☰ → Agregar página a) o Chrome.

---

## Kiosko: inicio con vitrina y cuadros (2026-11-10)

Con 7 botones la barra de abajo ya no cabía. Ahora el inicio es: a la izquierda la **Vitrina**,
que alterna el QR del monedero con productos al azar (foto, precio y QR para pedirlo en la app),
con flechas ‹ › para moverse; a la derecha, seis cuadros grandes que abren cada vista en pantalla
completa con "← Inicio". Los productos están detrás del mostrador: la Elo es su vitrina.

- `src/db/productos.rs` — `vitrina`: 24 productos al azar de `v_galeria_principal` con foto.
- `src/routes/kiosko.rs` — `GET /kiosko/vitrina` (con la cookie del kiosko): JSON con nombre,
  precio, unidad, foto y el QR a `/tienda/{nid}` de la app.
- `templates/app/kiosko.html` — vista `inicio` (vitrina + cuadros) en lugar de la vista
  `monedero` y la barra; botón "← Inicio" en las demás.
- `static/app/app.js` — `kioskoPantalla`: diapositivas (el monedero, y otra vez el monedero entre
  cada 3 productos), `mover`/‹ ›; cada 15 s, si nadie tocó en un minuto, pasa a la siguiente (o
  vuelve al inicio desde otra vista); otra tanda al azar cada 10 min.
- `static/css/app.css` — estilos de la vitrina, los cuadros y "← Inicio"; fuera los de la barra.

---

## Corrección: "destino is not defined" al crear el acceso (2026-11-10)

Al agregar `destino` a `entrarHuella` (tienda, `e5f78ca`), el reemplazo también cambió el final de
`crearAcceso` (alta desde el recibo o el kiosko), donde `destino` no existe: la passkey se creaba
y el celular quedaba ligado, pero al final salía el error en lugar de ir al monedero.

- `static/app/app.js` — `crearAcceso` vuelve a `location.href = fin.ir`.

---

## Kiosko: el enlace del QR vale 3 minutos (2026-11-10)

La pantalla sigue cambiando el QR cada 30 s, pero cada enlace valía 30 a 60 s: copiándolo y
pegándolo en el navegador no alcanzaba. Ahora vale su ventana y las 5 siguientes (unos 3 min).
Lo que comprueba el teléfono es el código de 4 dígitos por WhatsApp; el QR solo evita que sirva
una foto vieja.

- `src/kiosko.rs` — `VENTANAS_VALIDAS = 6`; `codigo_valido` las recorre. Prueba actualizada.
- `APP_MONEDERO.md` — anotado.

---

## App: íconos del manifest con versión (2026-11-10)

Al instalar la app salía el ícono de una app vieja (un lápiz amarillo), aunque el servidor entrega
el nuevo: la app vieja usaba los mismos nombres de archivo y el celular tenía guardado el ícono
viejo con esa dirección.

- `src/routes/app.rs` y `static/img/favicon/site.webmanifest` — íconos con `?v=4`, para que el
  celular los vuelva a bajar. `templates/base.html` — `site.webmanifest?v=5`.

---

## App: aviso "Instala la app" en todas las páginas (2026-11-10)

Antes el botón "Instalar" solo salía en el Monedero y, en Android, solo si Chrome ya lo había
ofrecido (`beforeinstallprompt`), y Chrome lo pausa días tras desinstalar o cerrar el aviso.

- `templates/app/instalar.html` (nuevo) — aviso arriba de Monedero, Tienda y Carrito (lo incluye
  `app/base.html` cuando hay `nav`; el kiosko no). Reemplaza el bloque que estaba en
  `monedero/app.html`.
- `static/app/app.js` — `instalarApp` (sustituye a `instalar`). "Instalar" intenta en orden:
  `navigator.install()` (Web Install API, Chrome 158, estable desde el 17 nov 2026; en Android
  aún sin confirmar), la ventana de Chrome (`beforeinstallprompt`) y los pasos a mano (iPhone:
  Compartir → Agregar a inicio con "Abrir como app web"; Android: menú ⋮ → Instalar app). No sale
  dentro de la app instalada; si `getInstalledRelatedApps()` dice que ya está instalada (Chrome
  de Android), sugiere abrirla desde la pantalla de inicio. ✕ lo oculta 30 días
  (`localStorage`). iPhone no deja a la página saber si está instalada.
- `src/routes/app.rs` — el manifest suma `related_applications` (se declara a sí misma).
- `static/css/app.css` — estilos del aviso; se quitan los del bloque viejo.

---

## CSS y JS con huella: un reinicio ya no deja archivos viejos guardados (2026-11-10)

Durante un reinicio conviven el pod viejo y el nuevo. Un celular podía pedir la página al nuevo
y el CSS o JS al viejo, y se quedaba un año con el archivo viejo bajo el nombre nuevo (pasó dos
veces: la barra sin estilo y el botón "Agregar al carrito" sin texto, porque `app.js` viejo y
`huella.js` nuevo declaraban las mismas funciones).

- `src/estaticos.rs` (nuevo) — al arrancar calcula la huella (SHA-256, 10 hex) de cada `.css` y
  `.js` de `static/`. `cache_estaticos` pone `Cache-Control`: un año si la huella pedida es la de
  este pod; `no-store` si es de otra versión o el archivo no existe; sin huella (imágenes con
  `?v=`), un año como siempre. Pruebas de las dos reglas.
- `src/main.rs` — función de plantillas `estatico("css/app.css")` → `/static/css/app.css?h=…`;
  `static_files` es un `Router` con ese middleware.
- `templates/base.html`, `templates/app/base.html`, `templates/carrito/index.html` — CSS y JS con
  `estatico(...)` en vez de `?v=`.
- `static/app/sw.js` — `VERSION = 'app-v2'` (borra lo que se guardó antes) y de `/static` solo
  guarda lo marcado `immutable`.
- `CLAUDE.md` — la regla de `?v=` queda solo para imágenes.

---

## App: ícono como el de xplaya.com y CSS sin la copia vieja (2026-11-10)

- `src/routes/app.rs` — el manifest de la app suma la versión maskable de 192 px (igual que
  `site.webmanifest`): sin ella, Android podía tomar la normal y meterla en un círculo blanco.
- `templates/app/base.html` — `app.css?v=8`. Durante el reinicio conviven el pod viejo y el
  nuevo: un celular pidió la página al nuevo (`v=7`) y el CSS al viejo, y se quedó un año con el
  CSS viejo bajo el nombre nuevo (barra sin estilo, tienda descompuesta). Pendiente: versión
  calculada por el servidor para que no vuelva a pasar.

---

## xplaya.com ya no se instala como app (2026-11-10)

El manifest del sitio (el del paquete de íconos) tenía `"display": "standalone"`, así que el
celular ofrecía instalar xplaya.com y quedaban dos apps (Monedero y xplaya). Como se decidió
dejarlos separados, solo app.xplaya.com se instala.

- `static/img/favicon/site.webmanifest` — `"display": "browser"`: conserva los íconos, pero el
  sitio ya no se ofrece como app.
- `templates/base.html` — `site.webmanifest?v=4`.

---

## "Entrar con huella" en el carrito de xplaya.com (2026-11-10)

Se decidió **no unir** app.xplaya.com y xplaya.com (lo que se gana es más orden que negocio). Lo
que sí tenía valor: que el carrito de xplaya.com **no pida datos a quien ya tiene la app**.

Se descartó compartir la cookie de la app con `Domain=xplaya.com`: la llave de sesión de cada
cliente viajaría a todos los subdominios (fotos en cntnt, analytics, POS…). En su lugar, el
carrito tiene **"Entrar con huella o cara"** con la misma passkey (es de `xplaya.com`) y
xplaya.com crea **su propia sesión**, que solo viaja a xplaya.com. Un toque, una vez al año.

- `src/passkeys.rs` — acepta también el origen de `SITE_URL` si está dentro del RP ID
  (producción); en desarrollo (`localhost` contra `app.localhost`) no aplica.
- `src/main.rs` — el sitio suma `/passkey/entrar/inicio` y `/fin` (los mismos de la app, ahora
  `pub` en `routes/app.rs`).
- `src/routes/carrito.rs` — `pagina` pasa `nombre` si hay sesión; `crear_pedido` con sesión usa
  `pedidos::crear_para_cliente` (sin crear ni buscar cliente por teléfono).
- `templates/carrito/index.html` — con sesión, "Pedido a nombre de…" y Confirmar; sin sesión,
  el cuadro "¿Ya tienes la app del monedero?" con el botón de huella, arriba del formulario.
- `static/js/huella.js` (nuevo) — los ayudantes de passkeys y `entrarHuella`, sacados de
  `app.js` (que registra el service worker y no se puede cargar en el sitio). La app carga los
  dos (`app.js?v=6`).

---

## App: tienda dentro de la app (2026-11-10)

La app de clientes (app.xplaya.com) suma la tienda: una barra abajo con **Monedero · Tienda ·
Carrito**. Con el celular ligado (passkey), el pedido no pide nombre ni teléfono: sale a nombre de
ese cliente, con el teléfono ya comprobado. Sin sesión se puede ver todo, y para mandar el pedido
se entra con la huella.

- `src/routes/tienda.rs` — `lista` y `detalle` (reusan `db::productos`), `carrito` (con sesión
  muestra a nombre de quién va) y `crear_pedido` (`POST /tienda/pedido`; 401 sin sesión, errores
  como `{ "error" }`; responde la URL de la cotización en xplaya.com).
- `src/db/pedidos.rs` — `crear_para_cliente`: el mismo `insertar_pedido` (`Origen = 1`) sin
  crear ni buscar cliente.
- `src/routes/app.rs` — rutas `/tienda`, `/tienda/{nid}`, `/tienda/pedido`, `/carrito`.
- `src/routes/monedero.rs` — el monedero pasa `nav = "monedero"` cuando se sirve en la app.
- `templates/app/nav.html` (barra) y `templates/app/tienda/{lista,detalle,carrito}.html`.
  `app/base.html` incluye la barra cuando la página manda `nav` y carga `cart.js` (el mismo store
  del sitio, antes de Alpine).
- `static/app/app.js` (`?v=5`) — `productoApp` (presentación y agregar), `carritoApp` (mandar
  pedido); `entrarHuella` acepta a dónde regresar (`entrarHuella('/carrito')`).
- `static/css/app.css` (`?v=7`) — barra, catálogo, producto y carrito.
- `Cargo.toml` — minijinja con `urlencode` (la búsqueda va en los enlaces de página).

---

## Kiosko: pestaña "Fotos credencial" (2026-11-10)

QR fijo al kiosko de imagina (`https://imagina.xplaya.com/kiosko`): foto de credencial y fotos
polaroid desde el celular del cliente. El menú queda con 7 botones.

- `src/routes/kiosko.rs` — `URL_IMAGINA_KIOSKO` y `fotos_qr` en `pantalla`.
- `templates/app/kiosko.html` — vista `fotos` y su botón, antes del de la reseña.

---

## Kiosko: pestaña "Nuestro WhatsApp" (2026-11-10)

QR del contacto de WhatsApp de la papelería (el que genera WhatsApp en "Compartir contacto"),
recortado para dejar solo el código con su margen blanco. Sirve para mandar archivos a imprimir,
preguntar o pedir.

- `static/img/kiosko/whatsapp-contacto.png` — el QR recortado (360×360).
- `templates/app/kiosko.html` — vista `contacto` y su botón, junto a "Te agregamos a WhatsApp".
- `static/css/app.css` (`?v=6`) — `.kiosko-qr img` toma el mismo tamaño que el SVG.

---

## Kiosko: pestaña "Haz tu pedido" (2026-11-10)

QR fijo a `{SITE_URL}/productos`: el cliente arma su pedido en el catálogo de xplaya.com desde su
celular (mientras espera en la fila o en su casa). El carrito ya pide nombre y teléfono.

- `src/routes/kiosko.rs` — `tienda_qr` en `pantalla`.
- `templates/app/kiosko.html` — vista `tienda` y su botón, antes del de la reseña.

---

## Kiosko: pestaña "Tu reseña" (2026-11-10)

Cuarta pestaña del menú: un QR fijo al formulario de reseña de la ficha de Google (el mismo
enlace de `/resena`), para subir en Maps.

- `src/routes/kiosko.rs` — `URL_RESENA_GOOGLE` y `resena_qr` (SVG con `qr_svg`) en `pantalla`.
- `templates/app/kiosko.html` — vista `resena` y su botón. Sin cambios de CSS ni JS: usa las
  mismas clases de la vista de WiFi y `ir('resena')`.

---

## Docs: Raspberry del kiosko e ideas para después (2026-11-10)

Solo documentación, sin código.

- `APP_MONEDERO.md` — cómo quedó la Raspberry `chaac`: la Elo no avisa que está conectada, así
  que `cmdline.txt` fuerza `video=HDMI-A-2:1920x1080@60D`; el autostart de labwc abre Chromium en
  modo kiosko con la URL de activación guardada en `url.txt`.
- `ROADMAP.md` — kiosko en producción; nueva sección "Ideas para el kiosko": cuenta en vivo,
  imprimir desde el celular y productos con foto.

---

## Kiosko: menú con WiFi y "te agregamos a WhatsApp" (2026-11-10)

La pantalla del kiosko tiene un menú abajo (es táctil): **Monedero** (lo de antes), **WiFi gratis**
y **Te agregamos a WhatsApp**. Requiere el bloque 2026-11-10 (b) del POS (rama
`feature/kiosko-menu`): `KioskoSolicitudes` suma `Tipo` (`monedero` o `whatsapp`), porque las dos
solicitudes comprueban el teléfono. La rama también trae su aviso.

- `templates/app/kiosko.html` — tres vistas con `x-show` y la barra de menú. La de WhatsApp tiene
  un teclado de números en pantalla: en Chromium en modo kiosko no hay teclado del sistema.
- `static/app/app.js` — `kioskoPantalla` lleva la vista, el teléfono que se teclea y la vuelta al
  monedero tras un minuto sin tocar. Versiones: `app.css?v=5`, `app.js?v=4`.
- `src/kiosko.rs` — `texto_qr_wifi()` arma el QR de WiFi (`WIFI:T:WPA;S:…;P:…;;`, escapando
  `\ ; , : "`), con prueba.
- `src/routes/kiosko.rs` — `pantalla` manda el QR de WiFi si hay `KIOSKO_WIFI_SSID`;
  `POST /kiosko/whatsapp` (exige la cookie del kiosko) crea una solicitud de tipo `whatsapp`
  (`db/kiosko.rs`; las del monedero filtran `tipo = 'monedero'`). El cliente lo crea el POS cuando
  la persona contesta el WhatsApp.

---

## App de clientes, paso 5: kiosko en la tienda (2026-11-10)

`app.xplaya.com/kiosko` es una pantalla para la tienda (Chromium en modo kiosko) con instrucciones y
un QR que cambia cada 30 s. El cliente lo escanea, escribe nombre y teléfono, la caja le manda por
WhatsApp el código de 4 dígitos que aparece en el POS, y al escribirlo liga su celular. Requiere la
tabla `KioskoSolicitudes` del POS (rama `feature/kiosko-codigo-whatsapp`), que también trae el aviso
flotante.

- `src/kiosko.rs` (nuevo, lógica pura con pruebas) — el código del QR es un HMAC-SHA256 del número
  de ventana de 30 s, con `KIOSKO_TOKEN` como llave. El servidor no guarda nada: recalcula y compara
  (vale la ventana actual y la anterior).
- `src/db/kiosko.rs` (nuevo) — crear la solicitud con un código al azar, revisarlo (5 intentos, 15
  minutos) y cerrarla con el cliente que quedó.
- `src/routes/kiosko.rs` (nuevo):
  - `/kiosko/activar?t=` guarda el token en una cookie HttpOnly (`Path=/kiosko`); `/kiosko` y
    `/kiosko/qr` la exigen. Mismo esquema que el kiosko retirado (`git checkout kiosko-v1`).
  - El QR abre `/kiosko/k/{código}` → una **visita** en memoria de 15 min → `/kiosko/s/{visita}`
    pide nombre y teléfono → se crea la solicitud y la página pasa a `/kiosko/s/{solicitud}`, que
    pide el código.
  - Con el código bien: si el teléfono ya existe, se liga a ese cliente; si no, el cliente se crea
    al terminar la passkey (nunca antes, para no dejar clientes de teléfonos ajenos). Antes del
    código el celular no ve el nombre registrado.
- `src/routes/alta.rs` — `AltaPendiente` lleva `Origen::Recibo(venta)` u `Origen::Kiosko(id)`;
  `completar_alta()` abona y gasta el enlace si viene de un recibo, o cierra la solicitud si viene
  del kiosko.
- `templates/app/alta.html` — los formularios usan `accion` (ruta base) y los textos cambian con
  `kiosko`; paso nuevo `codigo`. `templates/app/kiosko.html` (nuevo) y estilos `.kiosko*` en
  `app.css`; `kioskoPantalla` en `app.js` pide el QR nuevo justo cuando vence el anterior.
- Versiones: `app.css?v=4`, `app.js?v=3`.

---

## App de clientes, paso 4: desligar el celular desde el POS (2026-11-10)

Sin cambios de código en xplaya: el trabajo está en el POS (rama `feature/desligar-celular`).

- **POS**, ficha del cliente (`Clientes/EditarCliente`): muestra si tiene celular ligado (tipo,
  fecha y último uso) y un botón **Desligar**, que marca `FechaDesligado` con motivo `pos`.
- xplaya no necesita nada nuevo: `cliente_de_sesion()` y `por_credencial()` ya ignoran los celulares
  con `FechaDesligado`, así que la sesión y la passkey de ese celular dejan de servir en el momento.
- `APP_MONEDERO.md` — el QR de caja que caduca se descartó por ahora: solo protegería si el POS lo
  exigiera para gastar, y hoy la verificación real se hace por WhatsApp. El QR sigue llevando el
  teléfono.

---

## App de clientes, paso 3: alta con passkey desde el recibo (2026-11-09)

El recibo de una venta de las últimas 24 horas lleva un enlace a
`app.xplaya.com/alta/recibo/{venta}`. Desde ahí el cliente liga su celular con una **passkey**
(huella o cara) y la app abre su monedero sin el GUID en la URL. Requiere los cambios de BD del POS
(rama `feature/app-monedero-bd`: tablas nuevas y `fn_asignar_cliente_venta`).

- `Cargo.toml` — `webauthn-rs` 0.6 (con `conditional-ui`, para las passkeys descubribles), `sha2`
  (hash del token de sesión) y `rand` (el token). `webauthn-rs` es Rust puro: el `Containerfile` no
  cambia.
- `src/passkeys.rs` (nuevo):
  - `webauthn()` arma la configuración con `PASSKEY_RP_ID` y `APP_URL`.
  - `opciones_descubrible()` pide al celular una passkey **descubrible**: la necesitamos porque en
    iPhone la app instalada no comparte cookies con Safari, así que al abrirla no sabemos quién es.
    `webauthn-rs` la pide "discouraged" y no exporta el tipo para cambiarlo; se ajusta el JSON.
  - `Pendientes<T>`: lo que vive unos minutos en memoria (el reto de la passkey, el alta que ya pasó
    los 4 dígitos). xplaya corre con una réplica; si el pod se reinicia, el cliente vuelve a tocar el
    botón.
- `src/db/altas.rs` (nuevo) — el enlace está vivo si la venta tiene menos de 24 h (en hora de
  Cancún, como la guarda el POS) y no tiene resultado en `AltasRecibo`. `revisar_digitos()` cuenta
  los fallos y bloquea al 5.º. `gastar()` marca el resultado y devuelve `false` si alguien llegó
  antes. `abonar_venta()` llama a `fn_asignar_cliente_venta`, la misma función que usa el POS.
- `src/db/celulares.rs` (nuevo) — `ligar()` desliga el celular anterior (`reemplazado`) y borra sus
  sesiones. Sesiones de un año: la cookie `xp_sesion` lleva el token y la BD solo su SHA-256.
- `src/routes/alta.rs` (nuevo) — los cuatro casos del diseño:
  - Venta con cliente: 4 dígitos → passkey → celular ligado y `AceptoPrograma`.
  - Sin cliente, teléfono nuevo: nombre y teléfono → passkey → cliente nuevo y compra abonada. Si
    su celular no puede crear passkeys, "Solo abona mi compra" crea el monedero sin ligar.
  - Sin cliente, teléfono que ya existe: se abona la compra; **no** se liga el celular.
  - Cliente con sesión y venta sin cliente: botón para abonarla a su monedero.
  Nada se escribe hasta que la passkey existe (salvo los intentos fallidos), y todo el alta va en una
  transacción.
- `src/routes/app.rs`:
  - `/` muestra el monedero de la sesión.
  - `POST /passkey/entrar/*` entra con la passkey.
  - `POST /salir` cierra la sesión.
  - `/manifest.webmanifest` abre la app instalada en `/`.
  - `/monedero/{guid}` sigue igual para quien no ha ligado su celular.
- `src/routes/monedero.rs` — el recibo de xplaya.com muestra el enlace mientras esté vivo. Si el
  cliente ya tiene celular, el texto es "¿Cambiaste de celular? Liga el nuevo".
- Plantillas y estáticos:
  - `templates/app/alta.html` (nuevo): una plantilla con un `paso` por pantalla.
  - `templates/app/inicio.html`: botón "Entrar con huella o cara".
  - `templates/monedero/app.html`: manifest según haya sesión, y "Salir".
  - `templates/monedero/recibo.html`: el enlace de alta.
  - `templates/pages/terminos.html`: sección de la app y el celular ligado; los datos de privacidad
    agregan la llave pública y el tipo de celular.
  - `static/app/app.js` (`?v=2`): componentes `crearAcceso` y `entrarHuella`, que convierten
    base64url ↔ bytes para la API de passkeys del navegador. `static/css/app.css` (`?v=3`).
- `config.rs` / `.env.example` / `CLAUDE.md` — `PASSKEY_RP_ID` (por defecto `xplaya.com`; en
  desarrollo `app.localhost`, porque Chrome no acepta `localhost` para una página en `app.localhost`).

Probado contra la BD de desarrollo con Playwright y un autenticador virtual (20 casos), más curl para
el bloqueo, el vencimiento de 24 h y "solo abonar". Los datos de prueba se borraron.

---

## Fix: el build del contenedor no encontraba `sw.js` (2026-11-09)

El paso 2 incrustaba `static/app/sw.js` en el binario con `include_str!`, pero el `Containerfile`
compila solo con `src/` (copia `static/` después, en la imagen final). El build de GitHub falló;
producción siguió con la imagen anterior.

- `src/routes/app.rs` — `/sw.js` se sirve desde el disco con `ServeFile` (como el resto de
  estáticos) y `Cache-Control: no-cache`. Regla: nada de `include_str!` sobre archivos fuera de `src/`.

---

## App de clientes, paso 2: app.xplaya.com y PWA instalable (2026-11-09)

El mismo servidor atiende `xplaya.com` y `app.xplaya.com`. En la app, el monedero se puede
instalar en el celular y abre sin señal con la última copia guardada.

- `src/main.rs` — dos routers (`site` y `app`) y un tercero (`root`) que reparte cada petición
  según su `Host`. Un `Router` de Axum también es un `Service` de tower, por eso se le puede pasar la
  petición con `oneshot`. `tower` sube a 0.5 (la que ya usa Axum) para tener `ServiceExt`.
  `/static` pasa a una función `static_files()` que comparten los dos.
- `src/config.rs` — `APP_URL` (por defecto `https://app.xplaya.com`) y `app_host()`.
- `src/routes/app.rs` (nuevo) — rutas de la app:
  - `/` — si hay cookie `xp_monedero`, redirige a ese monedero; si no, pide el teléfono.
  - `POST /entrar` — busca por teléfono, como `/saldo`.
  - `/monedero/{guid}` — la tarjeta con la PWA activada; guarda la cookie un año. Si el GUID no
    existe, borra la cookie y vuelve a `/`.
  - `/monedero/{guid}/manifest.webmanifest` — manifest **por cliente**: su `start_url` es el
    monedero de ese cliente, así la app instalada entra directo, aun en iPhone (que no comparte
    cookies entre Safari y la app instalada).
  - `/sw.js` — el service worker, desde la raíz para controlar toda la app.
  - `/robots.txt` — `Disallow: /`.
- `src/routes/monedero.rs` — `render_monedero(state, id, en_app)` sirve la tarjeta a los dos
  dominios. `normalizar_telefono()` reemplaza la lógica que estaba repetida en `/saldo`.
- `static/app/sw.js` (nuevo) — páginas: red primero, copia guardada si no hay señal. `/static`,
  Alpine y fuentes: caché primero, para que el QR funcione sin internet en caja.
- `static/app/app.js` (nuevo) — botón "Instalar app" en Android (`beforeinstallprompt`), pasos para
  iPhone y aviso "Sin conexión".
- `templates/app/base.html` — con `en_app`: manifest, metas de Apple y `app.js`.
- `templates/app/inicio.html` (nuevo) — portada de la app.
- `templates/monedero/app.html` — bloque para instalar (en la app) o botón "Abrir como app" (en
  xplaya.com). Recibo, términos y xplaya.com se enlazan con `site_url`, porque en la app esas rutas
  no existen.
- `CLAUDE.md`, `.env.example` — `APP_URL` y cómo probar la app en `app.localhost`.

**Pendiente fuera del repo:** agregar `app.xplaya.com` en el túnel de Cloudflare apuntando al mismo
servicio que `xplaya.com`. Hasta entonces, el botón "Abrir como app" en xplaya.com no lleva a nada.

---

## App de clientes, paso 1: la tarjeta del monedero (2026-11-09)

`/monedero/{guid}` deja de verse como página de la tienda y se ve como app: tarjeta oscura con la
insignia del logo, saldo grande, botón "Mostrar en caja" y el historial ordenado. Es la misma
URL de hoy; en el paso 2 este layout se sirve en `app.xplaya.com`.

- `templates/app/base.html` (nuevo) — layout de la app, distinto de `base.html`: sin navbar, footer
  ni Bulma; fuentes Fredoka e Instrument Sans (las del logo oficial), Alpine para el QR y siempre
  `noindex`.
- `static/css/app.css` (nuevo) — estilos con los colores del logo v3; la página sigue el modo
  claro/oscuro del celular y la tarjeta siempre es oscura. El QR se muestra sobre blanco aun en modo
  oscuro, porque es lo que mejor leen los escáneres.
- `templates/monedero/app.html` — rediseño. Alpine (`x-data="{ qr: false }"`) abre el QR a pantalla
  completa; se cierra con el botón, tocando fuera o con Esc.
- `src/routes/monedero.rs` — el handler `app` genera el QR de caja con el teléfono del cliente,
  igual que las tarjetas impresas del POS, que ya lo lee. En el paso 4 se cambia por ID + código que
  caduca.
- `src/db/monedero.rs`, `src/models/monedero.rs` — la consulta trae `telefono`; solo se usa para
  el QR, no se muestra como texto.

---

## Diseño de la app de clientes (2026-11-09)

Solo documentación, en la rama nueva `develop`. La idea de la app del monedero vivía en el repo
`pwa-monedero`; como casi todo se construye aquí, se trae a este repo y aquel se retira.

- `APP_MONEDERO.md` (nuevo) — la idea con las decisiones de hoy: `app.xplaya.com`, un celular por
  cliente (el nuevo reemplaza al anterior), kiosko en tienda con QR de 30 s y aprobación por ntfy
  cuando el teléfono ya existe, enlace de alta en el recibo con sus cuatro casos, y abonos detectados
  con una tarea que revisa cada minuto. Termina con las piezas nuevas y el orden propuesto.
- `ROADMAP.md` — sección nueva con los seis pasos de la etapa 1.

---

## Se retira el aviso de Día de Muertos (2026-11-03)

Deshace el commit `9c2a0bf`. El aviso ya se ocultaba solo desde el 3 de noviembre, así que los
visitantes no ven ningún cambio; solo se limpia código que ya no se usa.

- `templates/base.html` — se quita el bloque `.aviso-muertos`; `main.css` sube a `?v=4` porque
  cambió su contenido y `/static` se sirve con caché `immutable`.
- `static/css/main.css` — se quitan los estilos `.aviso-muertos*`.
- `src/main.rs` — se quita la función `today()` de las plantillas, que solo usaba este aviso.

---

## CLAUDE.md: ruta real del monedero (2026-10-05)

Solo documentación. `CLAUDE.md` decía que el monedero vivía en `/app/{guid}`, pero la ruta real es
`/monedero/{guid}` (`src/main.rs:64`). Ese dato viejo hizo creer que la línea
`Disallow: /monedero/` de `robots.txt` no servía, cuando sí protege el monedero.

- `CLAUDE.md` — `/app/{guid}` → `/monedero/{guid}` en la estructura (`routes/monedero.rs`) y en la
  tabla de vistas migradas del POS, incluido el redirect de `/saldo`.
- `CLAUDE.md` — la columna "Origen en POS" ahora aclara que esas rutas se retiraron del POS el
  26 sep 2026 (commit `6547341` de `inventario_papeleria`). Se conserva como referencia histórica.

---

## Feeds para Google Merchant Center (2026-10-05)

Para que los productos aparezcan en Google Shopping, AI Mode y Maps ("búscame un borrador" cerca
de la tienda) con fichas gratuitas e inventario local. Merchant Center descarga los feeds una vez al
día; se arman desde la BD en cada request, así que un producto entra solo en cuanto tiene foto.

- `src/routes/feeds.rs` (nuevo) — `GET /feeds/google.xml` (catálogo) y
  `GET /feeds/google-local.xml` (inventario de la tienda). RSS 2.0 con namespace `g:`, armado a
  mano como `sitemap.xml`. `xml_escape()` también quita caracteres de control que XML 1.0 no
  permite: uno solo invalidaría el feed completo. `Cache-Control: public, max-age=3600`.
- `src/db/productos.rs` — `feed_productos()`: la regla del catálogo (`stock > 0 OR stockilimitado`,
  un kit sin material no sale) más lo que Google exige: sin servicios ni precio libre, precio > 0 y
  al menos una foto. `DISTINCT ON (nid)` porque `v_inventario` repite un producto si tiene filas
  duplicadas en `CategoriasProductos` (hoy el nid 5).
- `src/db/productos.rs` — `clasificar_codigo_barras()` ahora valida el dígito verificador GS1
  (`es_gtin_valido()`). Antes bastaba la longitud; un GTIN con checksum malo se rechaza en
  Merchant Center. Sin GTIN válido el feed manda `g:identifier_exists=no` (no guardamos MPN).
- `g:quantity` es la existencia real en piezas enteras (`floor`, hay productos por metro); se
  omite en stock ilimitado.
- `src/config.rs` / `.env.example` — `GOOGLE_STORE_CODE`: el "código de tienda" del Perfil de
  Negocio (no el Profile ID). Sin él, el feed local responde 404.
- `templates/productos/detalle.html` — `availableAtOrFrom → #store` en el `Offer`.

---

## Retiro del kiosko, la búsqueda semántica y las familias (2026-09-26)

Se quitó todo lo que dependía de embeddings: la columna `vector(1024)` por producto había
llevado la BD de ~10 a ~34 MB y seguía creciendo con cada producto nuevo, para un fallback de
búsqueda con muy poco tráfico. Con eso se fueron también el kiosko (piloto sin fecha, sin
mobiliario todavía) y sus tiles de `FamiliasSemanticas`. El código completo quedó en el tag
`kiosko-v1`.

- Borrados: `src/embeddings.rs`, `src/routes/kiosko.rs`, `src/db/kiosko.rs`,
  `src/models/kiosko.rs`, `templates/kiosko/`.
- `src/routes/productos.rs` — sin el bloque de fallback semántico; si la búsqueda no encuentra
  nada se muestra el "sin resultados" de siempre. `templates/productos/partials/grid.html` ya no
  recibe `semantica`.
- `src/db/productos.rs` — sin `busqueda_semantica()`, `SemanticaRow` ni `vector_literal()`.
- `src/db/pedidos.rs` / `src/models/pedido.rs` — sin `crear_kiosko()` ni los tipos
  `KioskoPedido*`; `insertar_pedido()` se conserva para `POST /pedidos` (web).
- `src/config.rs` / `.env.example` — sin `KIOSKO_TOKEN` ni `BGE_EMBEDDINGS_URL`.
- Los links a `{{ imagina_url }}/kiosko/...` en `fotos.html` y `foto-credencial.html` se quedan:
  son del kiosko de **imagina**, otro proyecto.

---

## Imágenes para compartir (`og_*`) con un ícono que invita a la acción

Las ocho `og_*.jpeg` se rehicieron para que se reconozcan aun a 300 px (como las muestra
WhatsApp): en vez de titular y lista, cada una es un ícono grande de lo que hace la página sobre
un panel de color, con el logo v3 puesto en el objeto y una insignia naranja de la acción. Todas
comparten posición, sombra dura, trazo, titular y línea naranja; solo cambia el panel (verde
azulado, mostaza o café oscuro) y el objeto.

| Imagen | Objeto | Insignia | Panel |
|---|---|---|---|
| `og_xplaya` | tienda | cursor | café |
| `og_terminos` | carpeta con hoja | "i" | mostaza |
| `og_saldo` | celular | lupa | verde azulado |
| `og_resena` | globo con estrellas | pin de mapa | café |
| `og_recibo` | ticket | palomita | verde azulado |
| `og_monedero` | cartera | "$" | mostaza |
| `og_producto` | bolsa | carrito | verde azulado |
| `og_cotizacion` | documento PDF | descarga | mostaza |

**Archivos a mirar:**
- `static/img/og_*.jpeg` — mismas medidas (1024 × 541) y mismos nombres; 46 a 52 KB cada una.
- Plantillas (15 archivos) — `?v=2` pasó a `?v=3` en cada `og:image` / `twitter:image` para que
  WhatsApp y Facebook vuelvan a bajarlas. Los enlaces ya compartidos pueden tardar en refrescarse.
- `.claude/skills/logo-designer/scripts/og-icons.js` (dibujo de los íconos), `build-og.js`
  (composición) y `assets/papeleria/og.json` (textos, paneles y páginas donde se usa cada una).
- `templates/pages/logo-oficial.html` — regenerada con el `?v=3`.

**Para ver el resultado:** correr el sitio y abrir el código fuente de cualquier página; el
`og:image` debe terminar en `?v=3`. Requiere deploy para que las apps las vean.

---

## Imágenes para compartir (`og_*`) rehechas en estilo Swiss con el logo v3

Las `og_*.jpeg` eran fotos hechas con IA con el logo 3D viejo horneado dentro (varias con textos
con errores). Ahora son ocho imágenes nuevas, sin fotos y sin datos de contacto: fondo blanco,
logo v3, un titular grande y una lista corta de lo que ofrece cada página, con el texto tomado de
la propia página. La del ticket digital (`og_recibo`) es una ilustración de un ticket con el logo.
Mismas medidas y mismos nombres que las anteriores.

**Archivos a mirar:**
- `static/img/og_*.jpeg` — solo quedan las ocho nuevas (1024 × 541, 43 a 59 KB; las anteriores
  pesaban 65 a 144 KB). Se **borraron** `og_catalogo.jpeg` y `og_futbol.jpeg`: ninguna página
  las usaba (la de fútbol era del tema del mundial).
- Plantillas (14 archivos) — cada `og:image` y `twitter:image` lleva `?v=2` porque WhatsApp y
  Facebook guardan la imagen por dirección y `/static` va con caché `immutable`. También se
  corrigieron las medidas declaradas: decían 1200 × 630 y los archivos miden 1024 × 541. La foto
  propia de cada producto (externa) conserva 1200 × 630.
- `.claude/skills/logo-designer/scripts/build-og.js` y `assets/papeleria/og.json` — generan las
  imágenes; el texto de cada una y las páginas donde se usa están en `og.json`.

**Cuando cambien:** regenerar con `build-og.js`, subir el `?v=` en las plantillas y regenerar
`/logo-oficial` (está en `SKILL.md` de `logo-designer`).

---

## `/logo-oficial`: descargas en SVG, PNG y JPG

La página ya enlazaba los SVG; ahora cada versión trae una tabla con **las tres formas** (una
línea, circular, dos líneas) y **tres formatos** (SVG, PNG, JPG), para bajar rápido el logo y
usarlo en herramientas externas.

**Archivos a mirar:**
- `static/img/logo/{v1,v2,v3}/papeleria-*.png` y `*.jpg` — 18 archivos nuevos, 3000 px en el lado
  largo (3.2 MB en total). El PNG conserva la transparencia; el **JPG lleva fondo café oscuro
  `#231916`** porque JPG no admite transparencia y los colores del logo están hechos para fondo oscuro.
- `.claude/skills/logo-designer/scripts/build-rasters.js` — los genera desde los SVG.
- `.claude/skills/logo-designer/scripts/build-lab.js` (`--site`) y la plantilla — arman la tabla de
  descargas; `templates/pages/logo-oficial.html` es la salida, **generada, no se edita a mano**.
  Cada enlace lleva `?v=<hash del archivo>` por el caché `immutable` de `/static`.

**Al cambiar un logo:** regenerar primero los PNG/JPG y después la página (está en `SKILL.md` de
`logo-designer`), porque los enlaces llevan el hash de cada archivo.

---

## Íconos y logo dentro de las páginas: de imágenes 3D a la insignia SVG v3

El favicon, el ícono de iOS, los íconos del manifiesto y el logo del encabezado eran el render 3D
viejo (`circleai`, un JPG/PNG cuadrado y pesado). Ahora salen de una **insignia**: el logo
circular v3 sobre su disco oscuro `#231916`, que se lee igual sobre fondo claro y oscuro.

**Archivos a mirar:**
- `static/img/logo/v3/papeleria-insignia.svg` — la insignia (la genera `build-logo.js` con
  `"badge"` en `v3.json`; los otros nueve SVG oficiales no cambiaron, se comprobó byte a byte).
- `static/img/favicon/*` — `favicon.svg`, `favicon.ico` (16/32/48), `favicon-96x96.png`,
  `apple-touch-icon.png`, `web-app-manifest-192/512.png` y `site.webmanifest`, regenerados con
  `.claude/skills/logo-designer/scripts/build-icons.js`. Pesan mucho menos (el PNG de 512 pasa de 334 KB a 25 KB).
- `templates/base.html`, `templates/kiosko/base.html`, `templates/pages/404.html`,
  `templates/pages/resena.html` — usan la insignia en el encabezado, la 404 y la reseña; los
  `<link>` de íconos y el JSON-LD llevan `?v=3`.
- `site.webmanifest` — además se arregló: decía "MyWebSite" y apuntaba a rutas que no existían.

**Caché:** `/static` va con `immutable` de un año, así que las URLs llevan `?v=3`; cuando cambien
los íconos hay que subir ese número en todas las plantillas (regla añadida en `CLAUDE.md`).

**Pendiente a propósito:** las imágenes `og_*.jpeg` de vista previa social siguen con el logo 3D
viejo horneado en fotos hechas con IA (algunas con textos con errores). Se rehacen aparte.

---

## Skill de carteles genérico: una marca por ficha

El skill `poster-designer` estaba escrito solo para la papelería. Ahora separa el **método** (que
sirve para cualquier cartel) de la **marca**, que entra como una ficha. Sirve para dos casos:
carteles propios (con el logo oficial y los datos reales del negocio) y carteles que se hacen
para otros negocios (con la marca del cliente, sin mezclar nada de la nuestra).

**Archivos a mirar** (`.claude/skills/poster-designer/`):
- `SKILL.md` — el método: primero decide para quién es el cartel, carga esa marca, junta los
  datos exactos, elige formato y estilo, construye un HTML con texto real, revisa y entrega.
- `brands/papeleria.md` — nuestra marca: logos v1/v2/v3, paleta y contraste, datos de contacto
  tomados del propio sitio (WhatsApp, dirección, horario), voz, tipografía sugerida, encaje de estilos.
- `brands/_template.md` — plantilla para registrar la marca de un cliente.
- `references/brief.md` — qué preguntar, cómo revisar el logo de un cliente (formato, resolución,
  fondos incrustados) y dónde guardar sus archivos: **fuera de `static/` y del repo** salvo que se diga lo contrario.
- `references/formats.md` — tamaños (tabloide, carta, WhatsApp, feed), impresión en casa contra
  imprenta (sin sangrado en casa, 0.125 in en imprenta), QR y fuentes incrustadas, cómo exportar.
- `references/styles.md` — menú de estilos, coherencia y cómo leer el feedback.
- `assets/poster-starter.html` — archivo base con el tamaño en tres sitios marcados `SIZE`,
  zona segura y guías (`#guides`).

En esta máquina no hay navegador utilizable, así que los carteles se entregan como HTML que se
abre y se exporta desde el navegador; las instrucciones de exportación no se han ejecutado aquí.

---

## Logos oficiales en SVG + página pública `/logo-oficial` + skill `logo-designer`

Los logos de la papelería eran renders 3D en JPG/PNG (fondo incrustado, baja resolución para
imprimir). Ahora hay **tres versiones oficiales en SVG**, transparentes y pintables letra por
letra: v1 mayúsculas en Fredoka Bold con la I de barras, v2 "Papelería" con la tipografía del
título (SemiBold), v3 mayúsculas SemiBold con la I de barras. Cada una en tres formatos: una
línea, circular con aro (grosor oficial = 100 % del trazo de la letra) y dos líneas sin aro.

`/logo-oficial` muestra las tres versiones, deja probarlas sobre distintos fondos, pintarlas,
cambiar el grosor del aro, quitarlo y copiar las variables CSS, y enlaza la descarga de cada
SVG. Es una página autocontenida (no extiende `base.html`, para que Bulma no pelee con su CSS),
con `noindex`.

**Archivos a mirar:**
- `src/main.rs` y `src/routes/pages.rs` — la ruta `GET /logo-oficial` y su handler `logo_oficial`,
  igual que los demás de `pages.rs` (lee la plantilla en tiempo de ejecución con Minijinja).
- `templates/pages/logo-oficial.html` — **generada**, no se edita a mano. Todo el contenido va
  dentro de `{% raw %}` para que Minijinja no lea llaves de CSS/JS como sintaxis; solo `site_url`
  queda fuera. No se puede usar `include_str!` porque la `Containerfile` compila solo con `src/`
  y copia `templates/` y `static/` después.
- `static/img/logo/{v1,v2,v3}/` — los SVG oficiales. `/static` se sirve con caché `immutable` de
  un año, por eso los enlaces de descarga llevan `?v=<hash del archivo>`.
- `.claude/skills/logo-designer/` — generador, configuraciones de las tres versiones, verificador,
  plantilla de la página y fuentes Fredoka (licencia OFL). Regenera la página con
  `build-lab.js ... --site` **cada vez que cambie un SVG**, porque van incrustados en ella.
- `.claude/skills/poster-designer/brands/papeleria.md` — ficha de marca de la papelería con las tres versiones oficiales.

---

## JSON-LD: entidad única `Store` + catálogo de servicios

Los servicios (engargolado, enmicado, escáner, trámites, envíos, arreglos de ropa) solo
existían como `<li>` de texto en el footer y prosa en `llms.txt` — sin datos estructurados.
Además el sitio describía la papelería como 4 entidades sueltas: el `Store` del `base.html`,
un `Store` inline duplicado en cada página de servicio (`provider`) y un `Organization` con
otro nombre en el `seller` de cada `Product`. Ahora hay **un solo nodo canónico**.

**Archivos a mirar:**
- `templates/base.html` — nodo `Store` (en todas las páginas):
  - `"@type": ["Store", "LocalBusiness"]` y `"@id": "{{ site_url }}/#store"` (ancla de la entidad).
  - `"alternateName": ["Papelería y Mercería El Gordo", "Papelería El Gordo"]` — enlaza el
    nombre con que Google conoce el negocio al nombre del sitio.
  - `"hasMap"` con la URL de Google Maps que ya estaba en `sameAs`.
  - `"hasOfferCatalog"` → `OfferCatalog` con 9 `Offer` → `Service`: copiado, fotos y
    foto-credencial referencian su página (`url` + `@id` del `Service` de esa página);
    engargolados, enmicados, escáner, trámites, envíos y arreglos de ropa son `Service`
    ligeros sin página. Todos con `provider: { "@id": ".../#store" }` y `areaServed`
    (ciudad, salvo envíos que es país México).
- `templates/pages/impresiones.html`, `fotos.html`, `foto-credencial.html` — el `Service`
  de cada página: se le puso `"@id"` (`.../<ruta>#service`, el mismo que referencia el
  catálogo) y `"url"`; el `provider` pasó de `Store` inline duplicado a
  `{ "@id": "{{ site_url }}/#store" }`. Se resuelve dentro del mismo DOM porque `base.html`
  ya trae el nodo completo del `Store` en cada página.
- `templates/productos/detalle.html` — `offers.seller` pasó de `Organization` suelto a
  `{ "@id": "{{ site_url }}/#store" }`.

**Verificado (local):** `cargo build` OK; los 2 bloques JSON-LD de `base.html` parsean como
JSON válido tras neutralizar jinja; el resto de páginas sin cambios de sintaxis jinja.
**Pendiente de decisión del usuario:** `aggregateRating` (requiere el conteo y promedio
reales de reseñas de Google — no se inventa); `HowTo` en `/impresiones` (Google retiró el
rich result de HowTo en 2023; `/foto-credencial` ya tiene `HowTo` para consumo de LLMs).

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

---

## Paso N — Título
**Archivos tocados:** `src/main.rs`, `templates/base.html`, ...
**Conceptos introducidos:** Axum Router, Minijinja Environment, ...

-->
