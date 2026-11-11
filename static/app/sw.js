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
                claves.filter((c) => c !== VERSION).map((c) => caches.delete(c))))
            .then(() => self.clients.claim())
    );
});

self.addEventListener('fetch', (event) => {
    const req = event.request;
    if (req.method !== 'GET') return;
    const url = new URL(req.url);

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

// ── Avisos (Web Push) ───────────────────────────────────────────────────────
// El servidor (src/push.rs) manda, cifrado, {id, titulo, texto, url}; el navegador lo
// descifra y despierta a este service worker unos segundos para mostrarlo.

const ICONO = '/static/img/favicon/web-app-manifest-192x192.png?v=4';

self.addEventListener('push', (event) => {
    let aviso = {};
    try { aviso = event.data ? event.data.json() : {}; } catch { /* sin JSON: aviso genérico */ }
    const titulo = aviso.titulo || 'Papelería xplaya';
    event.waitUntil(Promise.all([
        self.registration.showNotification(titulo, {
            body: aviso.texto || '',
            icon: ICONO,
            tag: aviso.id,                    // el mismo aviso dos veces se reemplaza, no se duplica
            data: { id: aviso.id, url: aviso.url || '/' },
        }),
        marcar(aviso.id, 'recibida'),
    ]));
});

self.addEventListener('notificationclick', (event) => {
    event.notification.close();
    const { id, url } = event.notification.data || {};
    // La URL puede ser de la app (/…) o completa (https://xplaya.com/cotizacion/…).
    const destino = new URL(url || '/', self.location.origin).href;
    event.waitUntil(Promise.all([abrir(destino), marcar(id, 'abierta')]));
});

// Si la app ya está abierta en esa página, la trae al frente; si no, abre una ventana.
async function abrir(destino) {
    const ventanas = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    const misma = ventanas.find((v) => v.url === destino);
    if (misma) return misma.focus();
    return self.clients.openWindow(destino);
}

// Le avisa al servidor; si no hay red, no pasa nada (el aviso ya se mostró).
function marcar(id, que) {
    if (!id) return Promise.resolve();
    return fetch(`/notificaciones/${id}/${que}`, { method: 'POST' }).catch(() => {});
}
