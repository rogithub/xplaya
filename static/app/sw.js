// Service worker de la app de clientes (app.xplaya.com).
//
// Se sirve en /sw.js (no en /static) para controlar toda la app. Sus trabajos:
// 1. Que la app abra sin señal — en caja el internet a veces falla — mostrando la
//    última versión del monedero y su QR.
// 2. Mostrar los avisos (Web Push) aunque la app esté cerrada, y contarle al servidor
//    cuándo llegaron y cuándo los tocó el cliente (routes/notificaciones.rs).
//
//  - Páginas: red primero; si no hay red, o el servidor no contesta (Cloudflare responde por
//    él con su página de error), la última copia guardada o la página "sin conexión".
//  - /static, Alpine y las fuentes: caché primero (no cambian sin cambiar de URL).
//
// Si cambias este archivo de forma que la caché vieja ya no sirva, sube VERSION.

// app-v2: borra lo guardado antes de las huellas (archivos viejos bajo nombres nuevos).
const VERSION = 'app-v2';

// Lo compartido desde otra app espera aquí a que el chat lo tome (no se borra al cambiar VERSION).
const COMPARTIDO = 'compartido';

const CACHE_PRIMERO = [
    'https://cdn.jsdelivr.net',
    'https://fonts.googleapis.com',
    'https://fonts.gstatic.com',
];

// El logo de la página "sin conexión": se guarda al instalar para que esté aunque el servidor no conteste.
const LOGO = '/static/img/logo/v3/papeleria-insignia.svg';

self.addEventListener('install', (event) => {
    event.waitUntil(
        caches.open(VERSION)
            .then((cache) => cache.add(LOGO))
            .catch(() => {})          // sin el logo la página sigue sirviendo
            .then(() => self.skipWaiting())
    );
});

self.addEventListener('activate', (event) => {
    event.waitUntil(
        caches.keys()
            .then((claves) => Promise.all(
                claves.filter((c) => c !== VERSION && c !== COMPARTIDO).map((c) => caches.delete(c))))
            .then(() => self.clients.claim())
    );
});

self.addEventListener('fetch', (event) => {
    const req = event.request;
    const url = new URL(req.url);
    if (req.method === 'POST' && url.origin === location.origin && url.pathname === '/chat/compartir') {
        event.respondWith(compartir(req));
        return;
    }
    if (req.method !== 'GET') return;

    if (req.mode === 'navigate') {
        event.respondWith(redPrimero(req));
    } else if ((url.origin === location.origin && url.pathname.startsWith('/static/'))
               || CACHE_PRIMERO.includes(url.origin)) {
        event.respondWith(cachePrimero(req));
    }
    // Lo demás (manifest, analytics) va directo a la red.
});

async function redPrimero(req) {
    const cache = await caches.open(VERSION);
    let resp;
    try {
        resp = await fetch(req);
    } catch {
        return guardadaOSinConexion(cache, req);
    }
    // Si el servidor está caído (sin luz en la papelería), Cloudflare contesta por él con su
    // página de error: 502–504 o sus propios 520–530. Para el cliente es lo mismo que no tener red.
    if (servidorCaido(resp.status)) return guardadaOSinConexion(cache, req);
    if (resp.ok) cache.put(req, resp.clone());
    return resp;
}

function servidorCaido(status) {
    return (status >= 502 && status <= 504) || (status >= 520 && status <= 530);
}

async function guardadaOSinConexion(cache, req) {
    const guardada = await cache.match(req);
    if (guardada) return guardada;
    return new Response(SIN_CONEXION, {
        status: 503,
        headers: { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' },
    });
}

// No sabemos si falla el internet del cliente o el de la papelería: el texto sirve para los dos.
// Se recarga sola cada 30 s, así el cliente no tiene que hacer nada cuando vuelva.
const SIN_CONEXION = `<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#EEF2F1">
<title>Papelería xplaya · Sin conexión</title>
<style>
  :root { --bg:#EEF2F1; --surface:#FFFFFF; --ink:#18211F; --muted:#56635F; --accent:#1F7A76; --on-accent:#FFFFFF; }
  @media (prefers-color-scheme: dark) {
    :root { --bg:#141110; --surface:#1F1917; --ink:#F2ECE7; --muted:#A99F98; --accent:#5CC3BE; --on-accent:#141110; }
  }
  * { box-sizing: border-box; }
  body { margin:0; min-height:100dvh; display:grid; place-items:center; padding:1.5rem;
         background:var(--bg); color:var(--ink); font-family:system-ui, sans-serif; text-align:center; }
  main { max-width:22rem; background:var(--surface); border-radius:1.25rem; padding:2rem 1.5rem; }
  img { width:96px; height:96px; }
  h1 { font-size:1.35rem; margin:1rem 0 .5rem; }
  p { color:var(--muted); line-height:1.5; margin:.5rem 0; }
  button { margin-top:1.25rem; font:inherit; font-weight:600; color:var(--on-accent); background:var(--accent);
           border:0; border-radius:999px; padding:.75rem 1.75rem; cursor:pointer; }
  small { display:block; margin-top:1rem; color:var(--muted); }
</style>
</head>
<body>
<main>
  <img src="${LOGO}" alt="Papelería xplaya" onerror="this.remove()">
  <h1>No pudimos conectar</h1>
  <p>Puede ser tu internet o el de la papelería (a veces se va la luz). Tu saldo está seguro.</p>
  <p>En cuanto vuelva la conexión, la app abre sola.</p>
  <button onclick="location.reload()">Intentar de nuevo</button>
  <small>Lo volvemos a intentar cada 30 segundos.</small>
</main>
<script>setTimeout(() => location.reload(), 30000);</script>
</body>
</html>`;

async function cachePrimero(req) {
    const cache = await caches.open(VERSION);
    const guardada = await cache.match(req);
    if (guardada) return guardada;
    const resp = await fetch(req);
    // De /static solo se guarda lo que el servidor marca para guardar (`immutable`): un
    // archivo pedido con la huella de otra versión llega con `no-store` (estaticos.rs).
    // Las respuestas de otros dominios sin CORS llegan "opacas" (status 0); también sirven.
    const guardable = resp.type === 'opaque'
        || (resp.ok && (new URL(req.url).origin !== location.origin
                        || (resp.headers.get('Cache-Control') || '').includes('immutable')));
    if (guardable) cache.put(req, resp.clone());
    return resp;
}

// ── Compartir a la app (Android) ────────────────────────────────────────────
// El manifest (share_target) manda aquí lo que el cliente comparte desde otra app. Se guarda
// en la caché del celular y se abre el chat, que lo toma (`chatTomarCompartido` en chat.js) y
// lo sube como si lo hubiera elegido con 📎: 3 a la vez, con avance y reintento.
async function compartir(req) {
    try {
        const datos = await req.formData();
        const cache = await caches.open(COMPARTIDO);
        // Lo de un compartido anterior que nunca se abrió ya no sirve.
        await Promise.all((await cache.keys()).map((k) => cache.delete(k)));
        const archivos = datos.getAll('files').filter((f) => f instanceof File && f.size);
        await Promise.all(archivos.map((f, n) => cache.put(`/compartido/${n}`, new Response(f, {
            headers: {
                'Content-Type': f.type || 'application/octet-stream',
                'X-Nombre': encodeURIComponent(f.name || `archivo-${n + 1}`),
            },
        }))));
        // Android pone el link en `text` o en `url` según la app; el título casi nunca sirve.
        const texto = [datos.get('text'), datos.get('url')]
            .filter((t) => typeof t === 'string' && t.trim())
            .filter((t, n, todos) => !todos.slice(0, n).some((otro) => otro.includes(t)))
            .join('\n');
        await cache.put('/compartido/texto', new Response(JSON.stringify({ texto, archivos: archivos.length })));
    } catch (e) {
        console.warn('Compartir:', e);
    }
    return Response.redirect('/chat?compartido=1', 303);
}

// ── Avisos (Web Push) ───────────────────────────────────────────────────────
// El servidor (src/push.rs) manda, cifrado, {id, titulo, texto, url}; el navegador lo
// descifra y despierta a este service worker unos segundos para mostrarlo. Los avisos de la
// cuenta de la caja (caja.rs) no traen `id` (no hay fila que marcar) y sí `tag`: el del cobro
// reemplaza al de la cuenta abierta y vuelve a sonar.

const ICONO = '/static/img/favicon/web-app-manifest-192x192.png?v=4';

// Los del chat (`tag: 'chat'`, chat/consumer.rs) no se muestran si el cliente tiene el chat a la
// vista: ya lo está leyendo, como en WhatsApp. En iPhone sí, siempre: Apple quita el permiso a la
// app que recibe avisos sin mostrarlos.
const IPHONE = /iPhone|iPad|iPod/.test(self.navigator.userAgent);

async function chatALaVista() {
    const ventanas = await self.clients.matchAll({ type: 'window' });
    return ventanas.some((v) => v.visibilityState === 'visible' && new URL(v.url).pathname === '/chat');
}

self.addEventListener('push', (event) => {
    let aviso = {};
    try { aviso = event.data ? event.data.json() : {}; } catch { /* sin JSON: aviso genérico */ }
    event.waitUntil(mostrar(aviso));
});

async function mostrar(aviso) {
    if (aviso.tag === 'chat' && !IPHONE && await chatALaVista()) return;
    const titulo = aviso.titulo || 'Papelería xplaya';
    await Promise.all([
        self.registration.showNotification(titulo, {
            body: aviso.texto || '',
            icon: ICONO,
            tag: aviso.tag || aviso.id,       // el mismo aviso dos veces se reemplaza, no se duplica
            renotify: Boolean(aviso.tag),     // reemplazar con `tag` suena otra vez
            data: { id: aviso.id, url: aviso.url || '/' },
        }),
        marcar(aviso.id, 'recibida'),
    ]);
}

self.addEventListener('notificationclick', (event) => {
    event.notification.close();
    const { id, url } = event.notification.data || {};
    // La URL puede ser de la app (/…) o completa (https://xplaya.com/cotizacion/…).
    const destino = new URL(url || '/', self.location.origin).href;
    event.waitUntil(Promise.all([abrir(destino), marcar(id, 'abierta')]));
});

// Si la app ya está abierta en esa página, la trae al frente; si está abierta en otra página de
// la app, la lleva ahí (así no se juntan ventanas); si no, abre una.
async function abrir(destino) {
    const ventanas = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    const misma = ventanas.find((v) => v.url === destino);
    if (misma) return misma.focus();
    const deLaApp = ventanas.find((v) => new URL(v.url).origin === new URL(destino).origin && 'navigate' in v);
    if (deLaApp) {
        try {
            const v = await deLaApp.focus();
            return await v.navigate(destino);
        } catch { /* no se dejó: una ventana nueva */ }
    }
    return self.clients.openWindow(destino);
}

// Le avisa al servidor; si no hay red, no pasa nada (el aviso ya se mostró).
function marcar(id, que) {
    if (!id) return Promise.resolve();
    return fetch(`/notificaciones/${id}/${que}`, { method: 'POST' }).catch(() => {});
}
