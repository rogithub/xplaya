// JavaScript de la app de clientes (solo en app.xplaya.com).
// Se carga antes que Alpine (los dos con `defer`, en orden) para registrar el
// componente `instalar` en el evento `alpine:init`.

// Chrome/Android avisa con `beforeinstallprompt` que la app se puede instalar.
// Puede llegar antes de que Alpine arranque, así que se guarda aquí.
let avisoInstalar = null;
window.addEventListener('beforeinstallprompt', (e) => {
    e.preventDefault();               // que no salga el aviso del navegador; usamos nuestro botón
    avisoInstalar = e;
    window.dispatchEvent(new Event('xp-instalable'));
});

document.addEventListener('alpine:init', () => {
    // Bloque "Instala la app": botón en Android, pasos en iPhone, nada si ya está instalada.
    Alpine.data('instalar', () => ({
        ios: /iphone|ipad|ipod/i.test(navigator.userAgent)
            || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1),   // iPad con iPadOS
        instalada: matchMedia('(display-mode: standalone)').matches || navigator.standalone === true,
        puedeInstalar: avisoInstalar !== null,
        pasos: false,

        init() {
            window.addEventListener('xp-instalable', () => { this.puedeInstalar = true; });
            window.addEventListener('appinstalled', () => { this.instalada = true; });
        },

        get visible() {
            return !this.instalada && (this.ios || this.puedeInstalar);
        },

        async instalarAndroid() {
            if (!avisoInstalar) return;
            avisoInstalar.prompt();
            await avisoInstalar.userChoice;
            avisoInstalar = null;
            this.puedeInstalar = false;
        },
    }));
});

// Avisa si no hay señal: la página que se ve puede ser la copia guardada.
document.addEventListener('alpine:init', () => {
    Alpine.data('conexion', () => ({
        enLinea: navigator.onLine,
        init() {
            window.addEventListener('online', () => { this.enLinea = true; });
            window.addEventListener('offline', () => { this.enLinea = false; });
        },
    }));
});

if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').catch((e) => console.warn('SW:', e));
}
