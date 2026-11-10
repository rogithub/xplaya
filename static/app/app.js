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

// ── Passkeys (huella o cara) ────────────────────────────────────────────────
// El servidor (webauthn-rs) manda y recibe los datos binarios en base64url; la API del
// navegador los quiere como bytes. Estas dos funciones convierten de un lado al otro.

function b64aBytes(texto) {
    let s = texto.replace(/-/g, '+').replace(/_/g, '/');
    while (s.length % 4) s += '=';
    return Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
}

function bytesAB64(buffer) {
    let s = '';
    new Uint8Array(buffer).forEach((b) => { s += String.fromCharCode(b); });
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

async function postJson(url, datos) {
    const resp = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(datos),
    });
    const json = await resp.json().catch(() => ({}));
    if (!resp.ok) throw new Error(json.error || 'Algo falló. Intenta de nuevo.');
    return json;
}

// El cliente canceló o el celular no encontró la passkey: no es un error "real".
function mensajeCancelado(e, texto) {
    return e.name === 'NotAllowedError' ? texto : e.message;
}

const passkeysSoportadas = typeof window.PublicKeyCredential === 'function';

document.addEventListener('alpine:init', () => {
    // Alta: crea la passkey del ticket del alta (routes/alta.rs).
    Alpine.data('crearAcceso', (ticket) => ({
        soportado: passkeysSoportadas,
        acepto: false,
        ocupado: false,
        error: '',

        async crear() {
            this.error = '';
            this.ocupado = true;
            try {
                const opciones = await postJson('/passkey/registro/inicio', { ticket });
                const pk = opciones.publicKey;
                pk.challenge = b64aBytes(pk.challenge);
                pk.user.id = b64aBytes(pk.user.id);
                (pk.excludeCredentials || []).forEach((c) => { c.id = b64aBytes(c.id); });

                const cred = await navigator.credentials.create({ publicKey: pk });
                const fin = await postJson('/passkey/registro/fin', {
                    ticket,
                    terminos: this.acepto,
                    credencial: {
                        id: cred.id,
                        rawId: bytesAB64(cred.rawId),
                        type: cred.type,
                        response: {
                            attestationObject: bytesAB64(cred.response.attestationObject),
                            clientDataJSON: bytesAB64(cred.response.clientDataJSON),
                            transports: cred.response.getTransports ? cred.response.getTransports() : undefined,
                        },
                        extensions: cred.getClientExtensionResults(),
                    },
                });
                location.href = fin.ir;
            } catch (e) {
                this.error = mensajeCancelado(e, 'Se canceló. Toca el botón para intentarlo otra vez.');
            } finally {
                this.ocupado = false;
            }
        },
    }));

    // Portada: entrar con la passkey que tenga el celular para este dominio.
    Alpine.data('entrarHuella', () => ({
        soportado: passkeysSoportadas,
        ocupado: false,
        error: '',

        async entrar() {
            this.error = '';
            this.ocupado = true;
            try {
                const { id, opciones } = await postJson('/passkey/entrar/inicio', {});
                const pk = opciones.publicKey;
                pk.challenge = b64aBytes(pk.challenge);
                (pk.allowCredentials || []).forEach((c) => { c.id = b64aBytes(c.id); });

                // Sin `mediation`: es un botón, no el autollenado del formulario.
                const cred = await navigator.credentials.get({ publicKey: pk });
                const r = cred.response;
                const fin = await postJson('/passkey/entrar/fin', {
                    id,
                    credencial: {
                        id: cred.id,
                        rawId: bytesAB64(cred.rawId),
                        type: cred.type,
                        response: {
                            authenticatorData: bytesAB64(r.authenticatorData),
                            clientDataJSON: bytesAB64(r.clientDataJSON),
                            signature: bytesAB64(r.signature),
                            userHandle: r.userHandle ? bytesAB64(r.userHandle) : null,
                        },
                        extensions: cred.getClientExtensionResults(),
                    },
                });
                location.href = fin.ir;
            } catch (e) {
                this.error = mensajeCancelado(e,
                    'No encontramos tu acceso en este celular. Liga tu celular desde el enlace de un recibo.');
            } finally {
                this.ocupado = false;
            }
        },
    }));

    // Kiosko, en el celular: espera a que la caja apruebe (ntfy) y entonces sigue solo.
    Alpine.data('esperarCaja', (espera) => ({
        estado: 'pendiente',

        init() { this.revisar(); },

        async revisar() {
            try {
                const r = await fetch(`/kiosko/espera/${espera}`, { cache: 'no-store' });
                this.estado = (await r.json()).estado;
            } catch (e) {
                // Sin señal un momento: se vuelve a intentar.
            }
            if (this.estado === 'aprobada' || this.estado === 'rechazada') {
                this.$refs.continuar.submit();
            } else if (this.estado === 'pendiente') {
                setTimeout(() => this.revisar(), 2000);
            }
        },
    }));

    // Pantalla del kiosko: pide un QR nuevo justo cuando vence el anterior y pasa las
    // instrucciones una por una.
    Alpine.data('kioskoPantalla', (pasos) => ({
        paso: 0,

        init() {
            this.nuevoQr();
            setInterval(() => { this.paso = (this.paso + 1) % pasos; }, 6000);
        },

        async nuevoQr() {
            let espera = 5000;
            try {
                const r = await fetch('/kiosko/qr', { cache: 'no-store' });
                if (r.ok) {
                    const { svg, ms } = await r.json();
                    this.$refs.qr.innerHTML = svg;
                    espera = ms + 200;
                }
            } catch (e) {
                // Sin red: se reintenta en 5 s.
            }
            setTimeout(() => this.nuevoQr(), espera);
        },
    }));
});

if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').catch((e) => console.warn('SW:', e));
}
