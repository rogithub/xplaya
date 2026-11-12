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
    // Aviso "Instala la app" (templates/app/instalar.html).
    const CERRADO = 'xp_instalar_cerrado';
    const DIAS_OCULTO = 30;
    Alpine.data('instalarApp', () => ({
        ios: /iphone|ipad|ipod/i.test(navigator.userAgent)
            || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1),   // iPad con iPadOS
        // Para los pasos a mano en Android: cada navegador tiene su menú.
        navegador: /Firefox\//.test(navigator.userAgent) ? 'firefox'
            : /SamsungBrowser/.test(navigator.userAgent) ? 'samsung' : 'chrome',
        // Abierta desde el ícono: su propia ventana, sin barra del navegador.
        enLaApp: matchMedia('(display-mode: standalone)').matches || navigator.standalone === true,
        yaInstalada: false,
        cerrado: false,
        pasos: '',

        async init() {
            try {
                const t = Number(localStorage.getItem(CERRADO) || 0);
                this.cerrado = Date.now() - t < DIAS_OCULTO * 864e5;
            } catch { /* sin localStorage: se muestra */ }
            window.addEventListener('appinstalled', () => { this.enLaApp = true; });
            // Chrome de Android: ¿ya está instalada? (manifest con related_applications)
            if (!this.enLaApp && navigator.getInstalledRelatedApps) {
                try {
                    this.yaInstalada = (await navigator.getInstalledRelatedApps()).length > 0;
                } catch { /* no responde: se ofrece instalar */ }
            }
        },

        get visible() { return !this.enLaApp && !this.cerrado; },

        cerrar() {
            this.cerrado = true;
            try { localStorage.setItem(CERRADO, String(Date.now())); } catch { /* nada */ }
        },

        async instalar() {
            // 1) Web Install API (Chrome 158+): la página pide instalar y Chrome muestra su
            //    ventana, sin esperar a que Chrome "quiera" ofrecerlo.
            if (navigator.install) {
                try { await navigator.install(); return; } catch { /* canceló o no se pudo: sigue */ }
            }
            // 2) La ventana de siempre de Chrome, si ya la ofreció (beforeinstallprompt).
            if (avisoInstalar) {
                avisoInstalar.prompt();
                await avisoInstalar.userChoice;
                avisoInstalar = null;
                return;
            }
            // 3) A mano.
            this.pasos = this.pasos ? '' : (this.ios ? 'ios' : 'android');
        },
    }));
});

// Notificaciones (templates/app/avisos.html): pide el permiso, suscribe el celular con la
// llave pública VAPID y le pasa la suscripción al servidor (routes/notificaciones.rs).
document.addEventListener('alpine:init', () => {
    const CERRADO = 'xp_avisos_cerrado';
    const DIAS_OCULTO = 30;
    Alpine.data('avisosApp', (publica) => ({
        // '': nada que mostrar · ofrecer · instalar (iPhone sin instalar) · bloqueado · listo
        estado: '',
        ocupado: false,
        error: '',
        cerrado: false,

        async init() {
            if (!('serviceWorker' in navigator) || !('PushManager' in window) || !('Notification' in window)) {
                // iPhone en Safari (sin instalar) no tiene PushManager: instalada, sí.
                const ios = /iphone|ipad|ipod/i.test(navigator.userAgent);
                this.estado = ios ? 'instalar' : '';
                this.leerCerrado();
                return;
            }
            if (Notification.permission === 'granted') {
                // Ya aceptó: confirmarla en silencio (o rehacerla si cambió la llave).
                try { await this.suscribir(); } catch (e) { console.warn('avisos:', e); }
                return;
            }
            this.estado = Notification.permission === 'denied' ? 'bloqueado' : 'ofrecer';
            this.leerCerrado();
        },

        get visible() { return this.estado !== '' && (!this.cerrado || this.estado === 'listo'); },

        leerCerrado() {
            try {
                const t = Number(localStorage.getItem(CERRADO) || 0);
                this.cerrado = Date.now() - t < DIAS_OCULTO * 864e5;
            } catch { /* sin localStorage: se muestra */ }
        },

        cerrar() {
            this.cerrado = true;
            try { localStorage.setItem(CERRADO, String(Date.now())); } catch { /* nada */ }
        },

        async activar() {
            this.error = '';
            this.ocupado = true;
            try {
                const permiso = await Notification.requestPermission();
                if (permiso === 'granted') {
                    await this.suscribir();
                    this.estado = 'listo';
                } else if (permiso === 'denied') {
                    this.estado = 'bloqueado';
                }
                // 'default': cerró la pregunta sin contestar; el botón sigue ahí.
            } catch (e) {
                this.error = e.message || 'No se pudieron activar los avisos.';
            } finally {
                this.ocupado = false;
            }
        },

        async suscribir() {
            const reg = await navigator.serviceWorker.ready;
            let sub = await reg.pushManager.getSubscription();
            // Suscrita con otra llave (la de otro ambiente o una llave nueva): se rehace.
            if (sub && sub.options.applicationServerKey
                && bytesAB64(sub.options.applicationServerKey) !== publica) {
                await sub.unsubscribe();
                sub = null;
            }
            if (!sub) {
                sub = await reg.pushManager.subscribe({
                    userVisibleOnly: true,             // cada aviso se ve: lo piden Chrome y Safari
                    applicationServerKey: b64aBytes(publica),
                });
            }
            await postJson('/notificaciones/suscripcion', sub.toJSON());
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
                location.href = fin.ir;
            } catch (e) {
                this.error = mensajeCancelado(e, 'Se canceló. Toca el botón para intentarlo otra vez.');
            } finally {
                this.ocupado = false;
            }
        },
    }));

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

    // Pantalla del kiosko: vitrina de productos, cuadros del menú y vuelta al inicio tras un
    // minuto sin que nadie toque la pantalla.
    Alpine.data('kioskoPantalla', () => ({
        vista: 'inicio',
        telefono: '',
        enviando: false,
        error: '',
        ultimoToque: 0,
        // Vitrina: productos al azar (/kiosko/vitrina) y en qué diapositiva va.
        productos: [],
        i: 0,

        init() {
            this.nuevoQr();
            this.cargarVitrina();
            setInterval(() => this.cargarVitrina(), 10 * 60 * 1000);   // otra tanda al azar
            setInterval(() => {
                const quieto = Date.now() - this.ultimoToque > 60000;
                if (!quieto) return;
                if (this.vista !== 'inicio') this.ir('inicio', false);
                else this.mover(1, false);              // la vitrina pasa sola
            }, 15000);
        },

        actividad() { this.ultimoToque = Date.now(); },

        ir(vista, tocado = true) {
            this.vista = vista;
            this.telefono = '';
            this.error = '';
            if (tocado) this.actividad();
        },

        // La diapositiva actual de la vitrina, o null si no hay productos.
        actual() { return this.productos[this.i] || null; },
        mover(paso, tocado = true) {
            const n = this.productos.length;
            if (!n) return;
            this.i = (this.i + paso + n) % n;
            if (tocado) this.actividad();
        },
        siguiente() { this.mover(1); },
        anterior() { this.mover(-1); },

        async cargarVitrina() {
            try {
                const r = await fetch('/kiosko/vitrina', { cache: 'no-store' });
                if (r.ok) {
                    this.productos = await r.json();
                    this.i = 0;
                }
            } catch (e) {
                // Sin red: se queda la tanda anterior (o el aviso de vitrina vacía).
            }
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
                setTimeout(() => { if (this.vista === 'gracias') this.ir('inicio', false); }, 12000);
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
                    // QR de "Instala nuestra app" (siempre en la página, para que se renueve solo).
                    this.$refs.qrApp.innerHTML = svg;
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

// La cuenta de la caja en vivo (recuadro en el monedero, solo con sesión). Escucha
// /cuenta/eventos (SSE, routes/cuenta.rs): `cuenta` trae el JSON que publicó el POS y
// `sin-cuenta` lo quita. EventSource se reconecta solo si se corta la señal.
document.addEventListener('alpine:init', () => {
    const PESOS = new Intl.NumberFormat('es-MX', { style: 'currency', currency: 'MXN' });

    Alpine.data('cuentaCaja', () => ({
        cuenta: null,
        // Al cobrar: monedero ganado y saldo nuevo (/cuenta/venta/{id}).
        premio: null,
        ventaPremio: null,

        init() {
            const fuente = new EventSource('/cuenta/eventos');
            fuente.addEventListener('cuenta', (e) => this.mostrar(JSON.parse(e.data)));
            fuente.addEventListener('sin-cuenta', () => this.mostrar(null));
        },

        mostrar(cuenta) {
            this.cuenta = cuenta;
            if (cuenta?.estado !== 'cobrada') {
                this.premio = null;
                this.ventaPremio = null;
                return;
            }
            if (cuenta.ventaId === this.ventaPremio) return;
            const venta = cuenta.ventaId;
            this.ventaPremio = venta;
            fetch(`/cuenta/venta/${venta}`, { cache: 'no-store' })
                .then((r) => (r.ok ? r.json() : null))
                .then((p) => { if (this.ventaPremio === venta) this.premio = p; })
                .catch(() => {});
        },

        get cobrada() { return this.cuenta?.estado === 'cobrada'; },
        pesos(n) { return PESOS.format(n ?? 0); },
        cantidad(n) { return Number(n).toLocaleString('es-MX', { maximumFractionDigits: 3 }); },
    }));
});
