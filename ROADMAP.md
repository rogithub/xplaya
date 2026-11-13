# ROADMAP — xplaya

> **Cómo usar este archivo:**
> Al iniciar una sesión nueva, lee este archivo primero. Encuentra el primer ⬜ de la lista de trabajo, lee la sección de detalle correspondiente más abajo, y arranca. Al terminar un paso, cámbialo a ✅ y anota en "Notas de sesiones" qué quedó hecho.

---

## Contexto

Papelería física en Playa del Carmen. Rust + Axum + sqlx + Minijinja + HTMX + Alpine.js + Bulma.
La web pública (`xplaya.com`) está en producción: catálogo, carrito/pedidos, monedero, recibos y cotizaciones.

**Proyectos involucrados:**
- `/mnt/storage/data/code/xplaya` — este proyecto (Rust+Axum)
- `/mnt/storage/data/code/inventario_papeleria` — POS, BD PostgreSQL compartida

---

## Lista de trabajo

### Base completada ✅

- ✅ Servidor base (Axum + Minijinja + Bulma/HTMX/Alpine)
- ✅ Catálogo `/productos` con paginación y búsqueda HTMX
- ✅ Detalle `/productos/:id` con galería Alpine
- ✅ Carrito Alpine + `POST /pedidos`
- ✅ Monedero, recibos, cotizaciones, URL cortas (`/app`, `/recibo`, `/cotizacion`, `/r/:code`)
- ✅ Página `/resena`
- ✅ SEO y Open Graph (meta tags, JSON-LD, OG dinámico por página)
- ✅ Soporte kits (compuestos) en catálogo, detalle y recibo
- ✅ Presentaciones de producto (unidad/caja/paquete)
- ✅ Página `/cortinas`
- ✅ Cashback potencial en cotizaciones
- ✅ QR de validación en impresiones de recibos y cotizaciones

### App de clientes (etapa 1: tarjeta del monedero)

Diseño completo en `APP_MONEDERO.md`. Se trabaja en la rama `develop`.

- ✅ Tarjeta de solo lectura con el diseño nuevo (`/monedero/{guid}`, 2026-11-09)
- ✅ `app.xplaya.com` + PWA instalable (2026-11-09)
- ✅ Tablas nuevas, términos y alta con passkey desde el recibo (2026-11-09, en producción el
  2026-11-10)
- ✅ Ver y desligar el celular en la ficha del cliente del POS (2026-11-10; el QR que caduca se
  descartó por ahora, ver `APP_MONEDERO.md`)
- ✅ Kiosko en tienda: QR de 30 s y código por WhatsApp desde el POS (2026-11-10, en producción
  con la Raspberry `chaac`)
- ✅ Menú del kiosko: WiFi para clientes y "te agregamos a WhatsApp" (2026-11-10)
- ✅ Pestaña "Tu reseña" en el kiosko: QR a la reseña de Google (2026-11-10)
- ✅ Pestaña "Haz tu pedido" en el kiosko (2026-11-10); reemplazada el mismo día por "Instala
  nuestra app", con el QR del monedero
- ✅ Pestaña "Nuestro WhatsApp" en el kiosko: QR del contacto de la papelería (2026-11-10)
- ✅ Pestaña "Fotos credencial" en el kiosko: QR a `imagina.xplaya.com/kiosko` (2026-11-10)
- ✅ Inicio del kiosko con Vitrina (monedero + productos al azar con QR) y cuadros (2026-11-10)
- ✅ Tienda dentro de la app: barra Monedero · Tienda · Carrito; el pedido sale a nombre del
  cliente de la sesión, sin pedir datos (2026-11-10)
- ✅ Se quedan separados app.xplaya.com y xplaya.com (2026-11-10); el carrito de xplaya.com
  tiene "Entrar con huella" (su propia sesión) y no pide datos a quien ya tiene la app
- ✅ **Cuenta de la caja en vivo** (2026-11-12): con el cliente elegido al empezar la venta, la app
  muestra la cuenta mientras se arma y al cobrar el monedero ganado y el saldo nuevo; Web Push al
  abrirla y al cobrar si la app no está en pantalla. POS → NATS `caja.cuenta` → SSE. Nota de
  diseño: `inventario_papeleria/docs/decisiones/2026-11-12-cuenta-en-vivo-en-la-app.md`.
- ⬜ Notificaciones: pedido listo, abonos y vencimiento (diseño en `APP_MONEDERO.md`)
- ⬜ **Recordatorio de pago de servicios** (más adelante): clientes que pagan agua, luz o teléfono
  en la tienda reciben aviso antes de su fecha de corte. Usa la misma tabla `Notificaciones` (un
  tipo nuevo). Falta decidir de dónde sale la fecha: registrada en el POS por servicio del cliente,
  o deducida de sus pagos anteriores.

### Ideas para el kiosko (sin fecha)

- Cuenta en vivo en el kiosko: se hizo en la app de cada cliente en lugar del kiosko (2026-11-12,
  ver "App de clientes"): solo la ve él y no depende de la pantalla pública.
- ✅ **Buscar por número** (2026-11-12): el NID en el teclado de la pantalla muestra el producto en
  grande (fotos, videos de TikTok, precio, presentaciones, si hay). La opción de mandarlo desde el
  POS ("Mostrar en kiosko") quedó para después.
- ✅ **Videos de TikTok incrustados** (2026-11-13) en el kiosko y en la página del producto; el POS
  ya solo acepta ligas de TikTok y guarda la completa.
- Imprimir: se descartó un flujo propio en imagina (subir archivos, acomodar fotos, Word a PDF):
  los clientes se atoran y le quitan tiempo a la caja. Lo cubre la pestaña "Nuestro WhatsApp".
- Turnos: se descartó (2026-11-11) un sistema de turnos (tomar turno en el kiosko, seguirlo en la
  app o en una página web sin app, botón "Siguiente" en el POS). Solo hay fila 2 o 3 días al año:
  no justifica tocar POS, kiosko y app, y nadie en caja tendría práctica con el botón. Esos días,
  números de papel. Si la fila se vuelve semanal, retomarlo.
- ⬜ **Carrito con código**: el pedido entra al momento y el teléfono se confirma con un código
  de 4 dígitos por WhatsApp (aviso en el POS y por ntfy, tema `pedidos`; con el puente
  `nats-ntfy`, ver "Avisos a la papelería"). Después: "Entrar con tu
  huella" para quien ya tiene la app.
- ✅ **Productos**: hecho como la Vitrina del inicio (2026-11-10); desde el mismo día muestra el
  inventario lento con más dinero parado.
- ⬜ **Lentos sin foto**: lista en el POS de los productos sin venderse en 60 días y sin foto,
  ordenada por dinero parado, para fotografiar primero esos (la vitrina solo muestra con foto).
- ⬜ **Medir la pantalla**: reporte semanal en Superset (altas de monedero desde el kiosko,
  contactos de WhatsApp, cuántos volvieron a comprar).

### Ganchos para la app: "Mis encargos" (por probar a mano primero)

**Problema (2026-11-13):** la app no tiene clientes reales; solo la tienen instalada los dueños. La
gente no la va a instalar si no le resuelve algo que hoy le cuesta. Se revisaron ideas y salió un
patrón común: **el cliente pide algo, la papelería lo atiende y la app le avisa**. Puede ser una
sola sección "Mis encargos" con estados y notificaciones, para tres servicios:

| Servicio | El cliente | La papelería | La app avisa |
|---|---|---|---|
| Impresiones | manda archivos + "cómo lo quiere" | imprime y acomoda | "Tu impresión está lista" |
| Paquetes | pone la dirección de la tienda a su nombre | lo guarda, foto de la etiqueta | "Tu paquete llegó" |
| Encargos | pega la liga de Amazon / Mercado Libre y paga en efectivo | lo compra con su tarjeta, cobra la diferencia | "Tu encargo llegó" |

**Datos que lo justifican (BD, 10 nov 2025 – 9 nov 2026):** copias e impresión fueron $55,599 de
$210,274 vendidos: **26.4 %** del total y **31.7 %** sin los servicios trasladados (63 pagos de
servicio por $32,702). Aparecen en **3,212 de 5,352 tickets (60 %)**. Imagina (el editor para
acomodar imágenes) no lo usan: quieren mandar los archivos y que la papelería los acomode, como por
WhatsApp. La competencia cercana imprime con chorro de tinta, más lento y cobra aparte el acomodo.

**Plan:**
- ⬜ **Probar paquetes y encargos sin sistema, un mes:** anuncio (letrero, estado de WhatsApp,
  TikTok), un producto en el POS ("Resguardo de paquete $X", "Encargo") y libreta o fotos; avisar
  por WhatsApp. Contar cuántos llegan y cuántos repiten. Antes del anuncio decidir: cuánto se
  cobra, cuántos días se guarda un paquete, si se aceptan paquetes contra entrega y qué
  devoluciones se cubren en los encargos.
- ⬜ **"Mandar a imprimir" en la app (versión mínima):** archivos (compartir desde el celular en
  Android con *share target*; botón "Subir archivo" en cualquiera, porque iPhone no tiene share
  target) + texto libre de cómo lo quiere → MinIO (bucket `papeleria-impresiones`) + tabla
  `TrabajosImpresion` (repo del POS) → aviso por ntfy (`avisos.impresiones`) con liga para ver los
  archivos y botón a WhatsApp → "Listo" → Web Push al cliente. Sin precios, pagos ni panel en el
  POS. Falta decidir: solo clientes con passkey (recomendado), días que se guardan los archivos
  (propuesta: 7 tras "Listo"), tipos y tamaños (PDF, JPG, PNG, HEIC; 10 archivos de 25 MB) y si
  "Listo" se marca también desde el POS.
- ⬜ **Después, con números:** cotización y pedido desde el POS, chat por encargo, y fijar fecha
  para que los trabajos de impresión lleguen por la app.

**Reglas que salieron de la plática:**
- **WhatsApp se queda** para dudas, clientes nuevos y sobre todo los **estados**, donde la
  publicidad de TikTok llega gratis porque la gente tiene guardado el número. Una app no iguala
  ese alcance. Un chat propio sirve ligado a cada encargo, no para reemplazar WhatsApp.
- **Primero que la app sea tan fácil como WhatsApp, luego pedir que la usen.** Con la ventaja de
  precio y rapidez frente a la competencia, se puede pedir con fecha, pero no antes de que la
  versión mínima funcione bien.
- **Encargos:** revisar antes los términos de Amazon Prime (parecen prohibir comprar para
  revender) y las condiciones de Mercado Libre; que paguen por adelantado; separar la tarjeta de
  los gastos de la tienda.
- **Paquetes:** los programas formales (puntos de entrega de Mercado Libre, Amazon, paqueterías)
  piden estar registrado como negocio; por cuenta propia se puede desde ya. Hace falta una regla de
  responsabilidad (valor máximo, "no respondemos por el contenido").

**Descartado o en espera:**
- **Cobro en línea con Mercado Pago** (2026-11-13): 3.49 % + $4 + IVA por venta al instante; la
  cuota fija hace caros los trabajos chicos (~$5.68 de recargo en $20, ~$6.95 en $50, contra ~$0.84 y
  ~$2.11 con la terminal). Los anticipos siguen por transferencia. CoDi: revisar cuando la reforma
  llegue a los bancos.
- **Lista de útiles escolares:** se surte una vez al año, poco, y la escuela cercana los regala.
- **Escáner de código de barras para clientes** (`BarcodeDetector`, solo Chrome de Android): en
  espera. Solo 706 de 2,083 productos del catálogo (34 %) tienen código capturado.

### Avisos a la papelería por ntfy (por revisar)

Desde el 12 nov 2026 hay un puente **`nats-ntfy`** (github.com/rogithub/nats-ntfy): cualquier
programa publica en NATS `avisos.<topic>` el JSON de ntfy y le llega al celular de quien esté
suscrito. Es para avisar a **personas de la papelería** (a los clientes los avisa el Web Push de la
app). Para sumar un aviso: publicar desde el programa (patrón en `src/avisos.rs`), dar el permiso
en NATS y usar un topic `papeleria-*` (así le llega a `dayana` y a `ro` sin tocar ntfy).

- ✅ **Código del alta del kiosko** al celular de la papelería, con botón a WhatsApp (2026-11-12),
  topic `papeleria-kiosko`.
- ✅ **Pedido nuevo en línea** (carrito de xplaya.com y tienda de la app) (2026-11-12), topic
  `pedidos`: número, productos, total y teléfono, con botones a la cotización y a WhatsApp.
- ✅ **"Agrégame a WhatsApp"** del kiosko (2026-11-12), topic `papeleria-kiosko`: teléfono, si ya
  es cliente, y botón que abre WhatsApp con el saludo escrito.
- ⬜ **Foto carnet / polaroid de imagina**: pedido nuevo en el kiosko de imagina, con botón al panel
  admin. Va en el repo de pdfeitor (`kiosko-carnet.md`, "Futuro").
- ⬜ **POS / agente de la PC de la papelería**: trabajo de impresión fallido o agente sin
  responder. El POS ya publica en NATS (.NET); le faltaría el permiso de `avisos.>`.

### Pendiente sin fecha

- ⬜ **ANALYTICS** — middleware que inserta en `Visitas`, gestión `SessionId` en cookie, excluir `/static/*`

---

## Retirado (2026-09-26)

Se retiraron el **kiosko táctil** (`/kiosko/*`), la **búsqueda semántica** (fallback con embeddings bge-m3) y las **FamiliasSemanticas** (tiles de categorías del kiosko):

- El kiosko era un piloto sin fecha: depende de mobiliario para los kioskos físicos. Las pantallas Elo se usan hoy en la venta touch del POS (`inventario_papeleria`).
- Los embeddings (`vector(1024)` por producto) triplicaban el tamaño de la BD (~10 → ~34 MB) y crecían ~4 KB por producto nuevo, para un fallback de búsqueda con muy poco tráfico.
- En la BD se quitaron `Productos.embedding`, `EmbeddingGeneratedAt`, `FamiliaSemanticaId`, la tabla `FamiliasSemanticas`, la extensión `vector` y el cliente de sistema "Kiosko en tienda" (`ID_CLIENTE_KIOSKO`). En k3s: `bge-embeddings`, el CronJob `embeddings-ingest` y el secret `papeleria-kiosko-secret`.

**Para retomarlo:** `git checkout kiosko-v1` tiene el código completo del kiosko y del fallback semántico, y este ROADMAP con el diseño detallado de cada fase (teclado en pantalla, token por cookie HttpOnly, pedidos con `Origen=0`, curación de las 37 familias). Los scripts de BD retirados están en el historial de `inventario_papeleria` (`dbchanges/`, `EMBEDDINGS_PLAN.md`).

**Lección que conviene no perder:** nunca `TRUNCATE ... CASCADE` sobre una tabla referenciada por `Productos` — en 2026-07-01 vació `Productos` y 13 tablas más en producción (se recuperó del backup de R2).
