// Service worker de la app de clientes (app.xplaya.com).
//
// Se sirve en /sw.js (no en /static) para controlar toda la app. Sus trabajos:
// 1. Que la app abra sin señal — en caja el internet a veces falla — mostrando la
//    última versión del monedero y su QR.
// 2. Mostrar los avisos (Web Push) aunque la app esté cerrada, y contarle al servidor
//    cuándo llegaron y cuándo los tocó el cliente (routes/notificaciones.rs).
//
//  - Páginas: red primero; si no hay red, la última copia guardada.
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

self.addEventListener('install', () => self.skipWaiting());

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
    try {
        const resp = await fetch(req);
        if (resp.ok) cache.put(req, resp.clone());
        return resp;
    } catch {
        const guardada = await cache.match(req);
        if (guardada) return guardada;
        return new Response(
            '<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width">' +
            '<body style="font-family:system-ui;padding:2rem;text-align:center">' +
            '<p>Sin conexión. Abre la app cuando tengas internet.</p></body>',
            { status: 503, headers: { 'Content-Type': 'text/html; charset=utf-8' } });
    }
}

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
