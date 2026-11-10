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

// Passkeys: los ayudantes (b64aBytes, bytesAB64, postJson, mensajeCancelado,
// passkeysSoportadas) y `entrarHuella` están en /static/js/huella.js, que se carga antes.

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
                location.href = destino || fin.ir;
            } catch (e) {
                this.error = mensajeCancelado(e, 'Se canceló. Toca el botón para intentarlo otra vez.');
            } finally {
                this.ocupado = false;
            }
        },
    }));

    // Pantalla del kiosko: menú (monedero, WiFi, WhatsApp), QR del monedero que se pide justo
    // cuando vence el anterior, instrucciones que pasan solas y vuelta al monedero tras un
    // minuto sin que nadie toque la pantalla.
    // ── Tienda (routes/tienda.rs) ──

    // Detalle de producto: presentación elegida y "Agregar al carrito" (store de cart.js).
    // Los datos vienen del <script id="producto-datos"> de la página.
    Alpine.data('productoApp', () => {
        const p = JSON.parse(document.getElementById('producto-datos').textContent);
        return {
            clave: null,
            precio: p.precio,
            unidad: p.unidad,
            agregado: false,

            elegir(clave) {
                const pres = clave && p.presentaciones.find((x) => x.id === clave);
                this.clave = pres ? clave : null;
                this.precio = pres ? pres.precio_venta : p.precio;
                this.unidad = pres ? pres.nombre : p.unidad;
                this.agregado = false;
            },

            agregar() {
                this.$store.carrito.agregar({
                    id: p.id,
                    nid: p.nid,
                    nombre: p.nombre,
                    precio: this.precio,
                    unidad_medida: this.unidad,
                    presentacion_id: this.clave,
                });
                this.agregado = true;
            },
        };
    });

    // Carrito: manda el pedido a nombre del cliente de la sesión.
    Alpine.data('carritoApp', () => ({
        enviando: false,
        error: '',
        cotizacion: '',

        async enviar() {
            this.error = '';
            this.enviando = true;
            try {
                const items = this.$store.carrito.items.map((i) => ({
                    producto_id: i.id,
                    cantidad: i.cantidad,
                    presentacion_id: i.presentacion_id || null,
                }));
                const r = await postJson('/tienda/pedido', { items });
                this.$store.carrito.vaciar();
                this.cotizacion = r.cotizacion;
            } catch (e) {
                this.error = e.message;
            } finally {
                this.enviando = false;
            }
        },
    }));

    Alpine.data('kioskoPantalla', (pasos) => ({
        vista: 'monedero',
        paso: 0,
        telefono: '',
        enviando: false,
        error: '',
        ultimoToque: Date.now(),

        init() {
            this.nuevoQr();
            setInterval(() => { this.paso = (this.paso + 1) % pasos; }, 6000);
            setInterval(() => {
                if (this.vista !== 'monedero' && Date.now() - this.ultimoToque > 60000) this.ir('monedero');
            }, 5000);
        },

        actividad() { this.ultimoToque = Date.now(); },

        ir(vista) {
            this.vista = vista;
            this.telefono = '';
            this.error = '';
            this.actividad();
        },

        tecla(d) { if (this.telefono.length < 10) this.telefono += d; this.error = ''; },
        borrar() { this.telefono = this.telefono.slice(0, -1); },
        telefonoBonito() {
            const t = this.telefono;
            return [t.slice(0, 3), t.slice(3, 6), t.slice(6)].filter((p) => p).join(' ');
        },

        async pedirWhatsApp() {
            this.enviando = true;
            this.error = '';
            try {
                await postJson('/kiosko/whatsapp', { telefono: this.telefono });
                this.ir('gracias');
                setTimeout(() => { if (this.vista === 'gracias') this.ir('monedero'); }, 12000);
            } catch (e) {
                this.error = e.message || 'Algo falló. Pide ayuda en la caja.';
            } finally {
                this.enviando = false;
            }
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
