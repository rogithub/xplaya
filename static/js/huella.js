// Entrar con huella o cara (passkey de xplaya.com). Lo usan la app (app.xplaya.com) y el
// carrito de xplaya.com: cada dominio crea su propia sesión, que solo viaja a ese dominio.
// Se carga antes que Alpine (y que app.js, que usa estos ayudantes).

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
    // Portada: entrar con la passkey que tenga el celular para este dominio.
    // `destino`: a dónde volver al entrar (p. ej. el carrito); sin él, a lo que diga el servidor.
    Alpine.data('entrarHuella', (destino) => ({
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
                location.href = destino || fin.ir;
            } catch (e) {
                this.error = mensajeCancelado(e,
                    'No encontramos tu acceso en este celular. Liga tu celular desde el enlace de un recibo.');
            } finally {
                this.ocupado = false;
            }
        },
    }));
});
