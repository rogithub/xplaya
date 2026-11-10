// Service worker de la app de clientes (app.xplaya.com).
//
// Se sirve en /sw.js (no en /static) para controlar toda la app. Su trabajo:
// que la app abra sin señal — en caja el internet a veces falla — mostrando la
// última versión del monedero y su QR.
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
