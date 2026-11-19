# Chat de la app

Chat directo entre la papelería y los clientes que tienen la app (`app.xplaya.com`). Diseño cerrado el
19 nov 2026. El avance por etapas se sigue en el artifact
https://claude.ai/artifact/UfNTCHwqPsWu19eS4gzqcn; este archivo es la referencia para el código y para
los otros proyectos (POS, asistente).

## Idea

- Una conversación **por cliente**, como en WhatsApp. Un mensaje se puede ligar a un encargo (etapa 7).
- Al principio se contesta **desde la caja** (POS). La conversación se abre en otra pestaña, siempre la
  misma, para no interrumpir un cobro. Para grabar notas de voz esa pestaña va por
  `https://papeleria.xplaya.com` (el micrófono pide https).
- **WhatsApp se queda** para dudas, clientes nuevos y estados. Llamadas, grupos, estados, reenviar y
  ubicación no se hacen.

## Cómo viaja un mensaje

Todo pasa por NATS, para que cualquier programa con su usuario pueda escuchar y reaccionar (el POS hoy,
un asistente de IA después). **Solo xplaya escribe en la tabla `Mensajes`**: lee el stream `CHAT` y
guarda todo, venga de quien venga. Así el asistente no necesita acceso a la base de datos.

```
celular ──HTTP/SSE── xplaya ──publica──▶ NATS stream CHAT ◀──publica── caja (POS)
                       │                       │                         (asistente, etapa 8)
                       └──── consumer xplaya ◀─┘
                                 │
                                 ▼
                          tabla Mensajes
```

### Temas: `chat.{clienteId}.{lado}.{evento}`

El lado va en el tema para que **los permisos de NATS garanticen quién es quién**: el POS solo puede
publicar en `chat.*.papeleria.>`, así que nadie edita ni borra un mensaje del otro lado. El autor de un
mensaje sale del tema, no del JSON.

| Lado | Quién publica | Eventos |
|---|---|---|
| `cliente` | xplaya (lo hizo el cliente en la app) | `mensaje`, `leido`, `editado`, `eliminado`, `escribiendo` |
| `papeleria` | POS (la caja) | los mismos |
| `automatico` | xplaya: respuesta fuera de horario, avisos de "Avisar por la app", "Recibimos tu pedido" | `mensaje` |
| `asistente` | el asistente (etapa 8) | `sugerencia`; `mensaje` cuando tenga permiso |

- Stream `CHAT`: todos los eventos menos `escribiendo`, **30 días** (`nats stream edit CHAT --max-age`).
  Ventana de duplicados de 2 min con el id del mensaje en `Nats-Msg-Id`.
- `escribiendo` va por NATS núcleo: es del momento y no se guarda.
- Quien escuche `editado` o `eliminado` **debe** actualizar o borrar su copia.

## Contrato v1

Ejemplos en `contratos/chat-v1/` (las pruebas los leen; el POS y el asistente pueden probar contra
los mismos archivos). Si el formato cambia, sube `v`; un programa que recibe una `v` que no conoce
la ignora.

```jsonc
// chat.{clienteId}.{lado}.mensaje
{
  "v": 1,
  "id": "0193f2c4-…",            // lo genera quien publica; sirve para no guardar dos veces
  "clienteId": "8a1e…",          // Clientes.Id, igual al del tema
  "tipo": "texto",               // texto | aviso | producto | recibo | cotizacion
  "texto": "¿Así está bien para engargolar?",
  "adjuntos": [                  // opcional; el archivo vive en MinIO (bucket papeleria-chat)
    { "nombre": "tesis.docx", "mime": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
      "bytes": 4194304, "llave": "8a1e…/0193f2c4-…/{archivo}.docx",  // el nombre del cliente no va en la llave
      "paginas": 48,             // PDF, o Word/Excel/PowerPoint ya convertido
      "pdf": "8a1e…/0193f2c4-…/{archivo}-pdf.pdf",   // el convertido; el original sigue en "llave"
      "miniatura": null }        // solo imágenes
  ],
  "respondeA": null,             // id del mensaje que cita
  "ref": null,                   // id del producto, recibo o cotización de la tarjeta
  "encargoId": null,             // etapa 7
  "fecha": "2026-11-19T17:42:00-05:00"
}
// chat.{clienteId}.{lado}.leido     → el lado leyó todo lo del otro lado hasta "hasta"
{ "v": 1, "clienteId": "…", "hasta": "2026-11-19T17:45:00-05:00" }
// chat.{clienteId}.{lado}.editado   → solo texto, solo en los primeros CHAT_MINUTOS_EDITAR
{ "v": 1, "clienteId": "…", "id": "…", "texto": "…", "fecha": "…" }
// chat.{clienteId}.{lado}.eliminado → solo el lado que lo escribió
{ "v": 1, "clienteId": "…", "id": "…", "fecha": "…" }
```

El autor no va en el JSON: es el lado del tema (`cliente`, `papeleria`, `automatico`, `asistente`).

## Datos

- **Tabla `Mensajes`** (repo del POS, `dbchanges/updates.sql`, 19 nov 2026): `Autor`, `Tipo`, `Texto`,
  `Adjuntos` (JSONB), `Ref`, `RespondeA`, `Secuencia` (para borrarlo del stream), `Fecha`,
  `FechaLeido`, `FechaEditado`, `FechaEliminado`. Fechas en hora de la tienda.
- **`Clientes.ChatBloqueado`**: la caja bloquea por abuso; el cliente ve que está bloqueado y no puede
  mandar. No hay límite de mensajes.
- **Ajustes en `Settings`**: `CHAT_DIAS_TABLA` (vacío: para siempre), `CHAT_MINUTOS_AVISO` (5),
  `CHAT_MINUTOS_EDITAR` (15), `CHAT_MB_DOCUMENTO` (25), `CHAT_MB_VIDEO` (100).
- **`CHAT_CLIENTES_PRUEBA`** (`Settings`): quién ve el chat en la app. **Sin la fila, nadie**; con
  la fila vacía (o NULL), todos; si no, los `Clientes.Id` separados por coma. No está en los
  scripts del POS a propósito: se agrega a mano con los clientes de prueba y se vacía cuando la
  caja pueda contestar (etapa 5).
- **Archivos**: MinIO, bucket `papeleria-chat`, **90 días** y sin copia a R2. Después, en el chat sale
  "Archivo vencido" con su nombre.

## Comportamiento acordado

- **Fuera de horario** (L–V 6:30–18:00, S 8:00–14:00; `reglas::abierto`): respuesta automática con el horario, una vez por
  cada cierre.
- **✓ enviado y ✓✓ leído** en los dos sentidos; **"escribiendo…"** en los dos sentidos.
- **Editar**: solo texto, los primeros 15 min, con la marca "Editado"; no se guarda la versión anterior.
- **Eliminar para todos**: cada lado borra los suyos, sin límite. El archivo se borra de MinIO al
  momento y el mensaje se quita del stream; queda "Mensaje eliminado". Al cliente: "Se borró de la app".
- **Responder a un mensaje** (`respondeA`), **mandar sin señal** (queda "enviando…"), **compartir a la
  app** desde otras apps en Android, **respuestas rápidas** en la caja.
- **Archivos**: imágenes (miniatura), videos (se reproducen ahí), PDF (páginas), Word/Excel/PowerPoint
  convertidos a PDF con Gotenberg; el original se puede bajar. 25 MB documentos, 95 MB videos
  (el tope de 100 MB de Cloudflare).
  Notas de voz. Para después: miniatura de PDF y HEIC.
- **Tarjetas propias** (no Open Graph) para productos, recibos y cotizaciones, con datos al día de la
  BD. Un link de xplaya.com (o `/r/…`) se vuelve tarjeta solo; los de otros sitios quedan como link.
- **Carrito y cotización separados**: al enviar el carrito, su cotización aparece en el chat.
- **Leer en voz alta** (🔊, voz del celular) y **tamaño de letra** A−/A+ para toda la app.
- **Avisos**: aviso flotante en el POS; si nadie lee en 5 min, ntfy al celular de la papelería
  (`avisos.chat`). Web Push al cliente con la app cerrada. Los avisos de "Avisar por la app" aparecen
  también en el chat.
- **Privacidad**: actualizar el aviso (LFPDPPP). Si un cliente pide borrar sus datos, se borran sus
  mensajes y archivos.
- **Para después**: reacciones, buscar, imprimir desde el chat (hoy solo hay blanco y negro).

## La app (etapa 3a)

```
celular ─POST /chat/mensaje─▶ xplaya ─publica (Nats-Msg-Id = id)─▶ CHAT
   ▲                                                                 │
   └──SSE /chat/eventos◀── broadcast ◀── consumer (después de guardar)┘
```

- Rutas en `src/routes/chat.rs`; pantalla en `templates/app/chat.html` y `static/app/chat.js`.
- El **id** lo genera el celular (un reintento sin señal no se guarda dos veces: `Nats-Msg-Id` en
  el stream y `ON CONFLICT` en la tabla). La **fecha** la pone xplaya al publicar.
- **✓** cuando NATS confirma la publicación; **✓✓** cuando llega el `leido` de la papelería.
- Los cambios en vivo los reparte el consumer después de guardar (`chat/nats.rs`), en memoria:
  sirve porque xplaya corre con **una réplica**. Con dos, habría que repartir por NATS.
- **Fuera de horario**: el consumer, al guardar un mensaje del cliente con la tienda cerrada,
  publica el aviso en `chat.{cliente}.automatico.mensaje`. Su id (UUID v5 del cliente y la hora
  del cierre) es el mismo en todo el cierre: si ya está guardado, no se repite.
- Editar y eliminar desde la app se revisan con `reglas::aplicar` antes de publicar, para
  contestar el error al momento; el consumer los vuelve a revisar al llegar.

## Archivos y tarjetas (etapa 3b)

```
celular ─POST /chat/archivo (1 por petición, 3 a la vez)─▶ xplaya ─/tmp─▶ MinIO papeleria-chat
   │                          ◀── número del archivo ──┘   (Adjunto en memoria, chat/subidas.rs)
   └─POST /chat/mensaje {archivos: [números], texto}─▶ publica UN mensaje con todos
```

- **Subida en dos tiempos**: cada archivo se escribe a `/tmp` (emptyDir de 1Gi en el pod) y va a
  MinIO sin pasar por memoria. El `Adjunto` lo arma el servidor: tipo por los primeros bytes, no
  por el mime del celular (`chat/archivos.rs`). Espera su mensaje 1 h; si nunca se manda, se borra.
- **Según el tipo**: imagen con miniatura, PDF con páginas, Word/Excel/PowerPoint convertidos con
  Gotenberg (uno a la vez; si falla, solo se baja el original), video y audio tal cual. La
  miniatura abre la foto en memoria, una a la vez: por eso el pod tiene 256Mi.
- **Servir**: `GET /chat/archivo/{mensaje}/{n}/{original|pdf|mini}` con `Range` (los videos se
  adelantan) y `GET /chat/zip/{mensaje}` (todo, en stream, con carpetas por tipo). Pasados los
  90 días: "Archivo vencido".
- **Notas de voz**: 🎤 con `MediaRecorder` (`.weba`, o `.m4a` en iPhone), por la misma subida.
  En el contrato son `tipo: texto` con un adjunto `audio/*`.
- **Tarjetas** (`chat/tarjetas.rs`): se resuelven al dar la vista, con datos al día. Salen de:
  - un link de xplaya.com o `/r/…` en el texto;
  - "Preguntar por el chat" en `/tienda/{nid}` (la tarjeta queda lista arriba del cuadro);
  - el carrito: al mandarlo desde la app, xplaya publica en `chat.{c}.automatico.mensaje` la
    cotización (id UUID v5 del pedido, no se repite).
- **Compartir a la app** (Android): `share_target` del manifest → `POST /chat/compartir`. Lo
  recibe el service worker (`static/app/sw.js`), no el servidor: guarda los archivos en la caché
  del celular y abre el chat, que los sube como si vinieran del 📎.

## Reglas del código

1. **Decisiones en funciones puras** (`src/chat/reglas.rs`): reciben el evento, lo guardado, la hora y
   los ajustes, y devuelven qué hacer. Pruebas unitarias sin BD ni red.
2. **Adaptadores delgados** (`src/chat/consumer.rs`, `src/db/chat.rs`): reciben, llaman a la lógica y
   ejecutan. No deciden.
3. **Pruebas de la BD** con `#[sqlx::test]`, sobre el schema real del POS
   (`../inventario_papeleria/dbscripts/postgresql_inventario.sql`, o la ruta en `POS_SCHEMA`).
4. **Prueba del contrato**: los JSON de `contratos/chat-v1/`.
5. **Traits solo con dos implementaciones reales.**

## Etapas

1. Diseño — hecho (19 nov 2026).
2. Base — hecho y probado en producción (19 nov 2026): tabla, stream `CHAT`, permisos de NATS,
   bucket `papeleria-chat`, xplaya guarda lo que llega al stream. Prueba: `xplaya-chat/probar-chat.sh`.
3. a) Chat de texto en la app — hecho y probado en producción (19 nov 2026) con los clientes
   de `CHAT_CLIENTES_PRUEBA`. Prueba: `xplaya-chat/contestar-como-caja.sh` (contesta como la
   caja) y `clientes-prueba.sql`. b) Archivos, voz y tarjetas — hecho y probado en producción (19 nov 2026):
   foto, video, PDF, Word, nota de voz, tarjetas, carrito y compartir desde Android. Para que
   salga en el menú de compartir hubo que reinstalar la app (Android guardaba el manifest viejo).
4. (unida a 3b)
5. Chat en la caja (POS).
6. Avisos con la app cerrada.
7. Marcar encargos.
8. Asistente de IA: propone respuestas (`sugerencia`) que la caja aprueba; sus herramientas para
   Claude en `MCP_papeleria_poc`.

## Etapa 2 en el cluster

Scripts en el share (`/home/ro/srv/share/xplaya-chat/`), en curicaueri:

- `crear-chat-nats.sh`: después de que ArgoCD sincronice los permisos, reinicia NATS y crea el
  stream y el consumer.
- `chat-minio.sh`: crea el bucket `papeleria-chat` con borrado a 90 días y amplía la política
  `xplaya-impresiones`.

```
# Lo que crea crear-chat-nats.sh (usuario ro)
nats stream add CHAT --defaults --subjects "chat.*.*.mensaje,chat.*.*.leido,chat.*.*.editado,chat.*.*.eliminado,chat.*.*.sugerencia" --storage file --retention limits --max-age 30d --max-bytes 512MB --replicas 1 --discard old --dupe-window 2m
nats consumer add CHAT xplaya --defaults --pull --deliver all --ack explicit --wait 30s --max-deliver 10
```

Permisos (`workloads/nats/configmap.yaml`): **xplaya** publica `$JS.API.CONSUMER.INFO.CHAT.xplaya`,
`$JS.API.CONSUMER.MSG.NEXT.CHAT.xplaya`, `$JS.ACK.CHAT.xplaya.>`, `$JS.API.STREAM.INFO.CHAT`,
`$JS.API.STREAM.MSG.DELETE.CHAT`, `chat.*.cliente.>`, `chat.*.automatico.>` y escucha
`chat.*.papeleria.escribiendo`. **pos** publica `chat.*.papeleria.>` y escucha `chat.>`.

