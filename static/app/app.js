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

// ── Instalar y avisos: lo comparten los avisos del monedero (instalar.html, avisos.html) y
// la pantalla "¡Listo!" que sale al terminar el alta (listo.html). ──

const esIos = () => /iphone|ipad|ipod/i.test(navigator.userAgent)
    || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);   // iPad con iPadOS
// Para los pasos a mano en Android: cada navegador tiene su menú.
const navegadorAndroid = () => (/Firefox\//.test(navigator.userAgent) ? 'firefox'
    : /SamsungBrowser/.test(navigator.userAgent) ? 'samsung' : 'chrome');
// Abierta desde el ícono: su propia ventana, sin barra del navegador.
const abiertaComoApp = () => matchMedia('(display-mode: standalone)').matches || navigator.standalone === true;

// Chrome de Android: ¿ya está instalada? (manifest con related_applications)
async function yaInstalada() {
    if (!navigator.getInstalledRelatedApps) return false;
    try { return (await navigator.getInstalledRelatedApps()).length > 0; } catch { return false; }
}

// Pide instalar. 'instalada' · 'cancelada' · 'a-mano' (no hay ventana: toca enseñar los pasos).
async function pedirInstalar() {
    // 1) Web Install API (Chrome 158+): la página pide instalar y Chrome muestra su
    //    ventana, sin esperar a que Chrome "quiera" ofrecerlo.
    if (navigator.install) {
        try { await navigator.install(); return 'instalada'; } catch { /* canceló o no se pudo: sigue */ }
    }
    // 2) La ventana de siempre de Chrome, si ya la ofreció (beforeinstallprompt).
    if (avisoInstalar) {
        avisoInstalar.prompt();
        const { outcome } = await avisoInstalar.userChoice;
        avisoInstalar = null;
        return outcome === 'accepted' ? 'instalada' : 'cancelada';
    }
    // 3) A mano.
    return 'a-mano';
}

// Estado de los avisos en este celular:
// '' (no se pueden) · instalar (iPhone sin instalar) · ofrecer · bloqueado · aceptados
function estadoAvisos() {
    if (!('serviceWorker' in navigator) || !('PushManager' in window) || !('Notification' in window)) {
        // iPhone en Safari (sin instalar) no tiene PushManager: instalada, sí.
        return esIos() ? 'instalar' : '';
    }
    return { granted: 'aceptados', denied: 'bloqueado' }[Notification.permission] || 'ofrecer';
}

// Pide el permiso y suscribe. 'listo' · 'bloqueado' · 'ofrecer' (cerró la pregunta sin contestar).
async function activarAvisos(publica) {
    const permiso = await Notification.requestPermission();
    if (permiso === 'granted') {
        await suscribirAvisos(publica);
        return 'listo';
    }
    return permiso === 'denied' ? 'bloqueado' : 'ofrecer';
}

// Suscribe el celular con la llave pública VAPID y le pasa la suscripción al servidor
// (routes/notificaciones.rs).
async function suscribirAvisos(publica) {
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
}

// "No mostrar" por 30 días en este celular.
const DIAS_OCULTO = 30;
function ocultoPorAhora(clave) {
    try { return Date.now() - Number(localStorage.getItem(clave) || 0) < DIAS_OCULTO * 864e5; } catch { return false; }
}
function ocultar(clave) {
    try { localStorage.setItem(clave, String(Date.now())); } catch { /* nada */ }
}

document.addEventListener('alpine:init', () => {
    // Aviso "Instala la app" (templates/app/instalar.html).
    const CERRADO = 'xp_instalar_cerrado';
    Alpine.data('instalarApp', () => ({
        ios: esIos(),
        navegador: navegadorAndroid(),
        enLaApp: abiertaComoApp(),
        yaInstalada: false,
        cerrado: false,
        pasos: '',

        async init() {
            this.cerrado = ocultoPorAhora(CERRADO);
            window.addEventListener('appinstalled', () => { this.enLaApp = true; });
            if (!this.enLaApp) this.yaInstalada = await yaInstalada();
        },

        get visible() { return !this.enLaApp && !this.cerrado; },

        cerrar() {
            this.cerrado = true;
            ocultar(CERRADO);
        },

        async instalar() {
            if (await pedirInstalar() === 'a-mano') {
                this.pasos = this.pasos ? '' : (this.ios ? 'ios' : 'android');
            }
        },
    }));
});

document.addEventListener('alpine:init', () => {
    // Aviso "Recibe avisos" (templates/app/avisos.html).
    const CERRADO = 'xp_avisos_cerrado';
    Alpine.data('avisosApp', (publica) => ({
        // '': nada que mostrar · ofrecer · instalar (iPhone sin instalar) · bloqueado · listo
        estado: '',
        ocupado: false,
        error: '',
        cerrado: false,

        async init() {
            const estado = estadoAvisos();
            if (estado === 'aceptados') {
                // Ya aceptó: confirmarla en silencio (o rehacerla si cambió la llave).
                try { await suscribirAvisos(publica); } catch (e) { console.warn('avisos:', e); }
                return;
            }
            this.estado = estado;
            this.cerrado = ocultoPorAhora(CERRADO);
        },

        get visible() { return this.estado !== '' && (!this.cerrado || this.estado === 'listo'); },

        cerrar() {
            this.cerrado = true;
            ocultar(CERRADO);
        },

        async activar() {
            this.error = '';
            this.ocupado = true;
            try {
                this.estado = await activarAvisos(publica);
            } catch (e) {
                this.error = e.message || 'No se pudieron activar los avisos.';
            } finally {
                this.ocupado = false;
            }
        },
    }));
});

document.addEventListener('alpine:init', () => {
    // Pantalla "¡Listo!" al terminar el alta (templates/app/listo.html): lo que falta para
    // tener la app completa, en filas que se ponen ✅. Pensada para que la cajera, con el
    // celular del cliente en la mano, vea de un vistazo qué falta.
    Alpine.data('listoApp', (publica) => ({
        ios: esIos(),
        navegador: navegadorAndroid(),
        // instalar: '' · instalada · pasos (enseñando los pasos a mano)
        instalar: abiertaComoApp() ? 'instalada' : '',
        // avisos: '' (no se ofrecen) · instalar · ofrecer · bloqueado · listo
        avisos: '',
        ocupado: false,
        error: '',

        async init() {
            window.addEventListener('appinstalled', () => { this.instalar = 'instalada'; });
            if (!this.instalar && await yaInstalada()) this.instalar = 'instalada';
            if (!publica) return;                       // sin llave VAPID: no hay avisos
            const estado = estadoAvisos();
            if (estado === 'aceptados') {
                try { await suscribirAvisos(publica); this.avisos = 'listo'; } catch (e) { console.warn('avisos:', e); this.avisos = 'ofrecer'; }
            } else {
                this.avisos = estado;
            }
        },

        async pedirInstalar() {
            const r = await pedirInstalar();
            if (r === 'instalada') this.instalar = 'instalada';
            else if (r === 'a-mano') this.instalar = this.instalar === 'pasos' ? '' : 'pasos';
        },

        async activarAvisos() {
            this.error = '';
            this.ocupado = true;
            try {
                this.avisos = await activarAvisos(publica);
            } catch (e) {
                this.error = e.message || 'No se pudieron activar los avisos.';
            } finally {
                this.ocupado = false;
            }
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

// Vitrina del kiosko: la diapositiva de la app ('app') después de cada `CADA_APP` productos.
// Sin productos (sin red o sin fotos), solo la de la app.
const CADA_APP = 5;
function conLaApp(productos) {
    const d = [];
    productos.forEach((p, k) => {
        d.push(p);
        if ((k + 1) % CADA_APP === 0) d.push('app');
    });
    if (productos.length % CADA_APP !== 0 || !productos.length) d.push('app');
    return d;
}

document.addEventListener('alpine:init', () => {
    // Alta: crea la passkey del ticket del alta (routes/alta.rs).
    // `yaAcepto`: ya es cliente (aceptó el programa en la caja), sin casilla de términos.
    // `fallo`: la huella no funcionó (o falló el regreso de Google); entonces se ofrece Google.
    Alpine.data('crearAcceso', (ticket, yaAcepto, falloInicial) => ({
        soportado: passkeysSoportadas,
        acepto: yaAcepto,
        ocupado: false,
        error: '',
        fallo: falloInicial,

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
                // Android sin bloqueo de pantalla no puede crear passkeys: aquí aparece Google.
                this.fallo = true;
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
        nuevaVersion: false,    // el servidor ya tiene otra versión de la página (nuevoQr)
        // Vitrina: productos al azar (/kiosko/vitrina); las diapositivas son esos productos con la
        // de la app cada `CADA_APP`; `i` es la que se ve y `desde`, cuándo empezó a verse.
        productos: [],
        diapositivas: [],
        i: 0,
        desde: 0,
        // Buscar por número: el NID que se escribe, el producto encontrado, sus medios (primero los
        // videos de TikTok, luego las fotos) y cuál se ve.
        nid: '',
        producto: null,
        medios: [],
        medio: 0,
        buscando: false,

        init() {
            this.nuevoQr();
            this.cargarVitrina();
            setInterval(() => this.cargarVitrina(), 10 * 60 * 1000);   // otra tanda al azar
            // Producto buscado: las fotos pasan solas si nadie toca la pantalla; en un video se queda.
            setInterval(() => {
                if (this.vista !== 'producto' || this.medioActual()?.tipo === 'video') return;
                if (Date.now() - this.ultimoToque > 4000) this.moverMedio(1, false);
            }, 4000);
            // El reproductor de TikTok no tiene parámetro para arrancar sin sonido (solo uno que lo
            // bloquea): al avisar que está listo se le pide silencio y que reproduzca; así se
            // puede subir el volumen. developers.tiktok.com/doc/embed-player
            window.addEventListener('message', (e) => {
                if (e.origin !== 'https://www.tiktok.com' || e.data?.type !== 'onPlayerReady') return;
                for (const type of ['mute', 'play']) {
                    e.source.postMessage({ type, 'x-tiktok-player': true }, e.origin);
                }
            });
            setInterval(() => {
                const quieto = Date.now() - this.ultimoToque > 60000;
                if (!quieto) return;
                if (this.nuevaVersion) { location.reload(); return; }
                if (this.vista !== 'inicio') this.ir('inicio', false);
                // La vitrina pasa sola; la de la app se queda el doble, para dar tiempo de sacar el celular.
                else if (!this.esApp() || Date.now() - this.desde >= 29000) this.mover(1, false);
            }, 15000);
        },

        actividad() { this.ultimoToque = Date.now(); },

        ir(vista, tocado = true) {
            this.vista = vista;
            this.telefono = '';
            this.nid = '';
            this.error = '';
            if (tocado) this.actividad();
        },

        // El producto de la diapositiva actual, o null (no hay productos o se ve la de la app).
        actual() {
            const d = this.diapositivas[this.i];
            return d && d !== 'app' ? d : null;
        },
        esApp() { return this.diapositivas[this.i] === 'app'; },
        mover(paso, tocado = true) {
            const n = this.diapositivas.length;
            if (!n) return;
            this.i = (this.i + paso + n) % n;
            this.desde = Date.now();
            if (tocado) this.actividad();
        },
        siguiente() { this.mover(1); },
        anterior() { this.mover(-1); },

        async cargarVitrina() {
            try {
                const r = await fetch('/kiosko/vitrina', { cache: 'no-store' });
                if (r.ok) {
                    this.productos = await r.json();
                    this.diapositivas = conLaApp(this.productos);
                    this.i = 0;
                    this.desde = Date.now();
                }
            } catch (e) {
                // Sin red: se queda la tanda anterior (o el aviso de vitrina vacía).
            }
        },

        medioActual() { return this.medios[this.medio] || null; },

        quitarFoto(url) {
            const j = this.medios.findIndex((m) => m.src === url);
            if (j < 0) return;          // ya se quitó, o ya se buscó otro producto
            this.medios.splice(j, 1);
            if (this.medio >= this.medios.length) this.medio = 0;
        },

        moverMedio(paso, tocado = true) {
            const n = this.medios.length;
            if (n < 2) return;
            this.medio = (this.medio + paso + n) % n;
            if (tocado) this.actividad();
        },

        teclaNid(d) { if (this.nid.length < 6) this.nid += d; this.error = ''; },

        async buscarNid() {
            const nid = this.nid;
            this.buscando = true;
            this.error = '';
            try {
                const r = await fetch(`/kiosko/producto/${nid}`, { cache: 'no-store' });
                if (r.status === 404) { this.error = `No encontramos el número ${nid}.`; return; }
                if (!r.ok) throw new Error();
                this.producto = await r.json();
                this.medios = [
                    ...this.producto.videos.map((src) => ({ tipo: 'video', src })),
                    ...this.producto.fotos.map((src) => ({ tipo: 'foto', src })),
                ];
                this.medio = 0;
                // Una foto registrada que no está en MinIO no carga: se quita de la lista para no
                // dejar flechas que no cambian nada.
                this.producto.fotos.forEach((url) => {
                    const img = new Image();
                    img.onerror = () => this.quitarFoto(url);
                    img.src = url;
                });
                this.ir('producto');
            } catch (e) {
                this.error = 'Algo falló. Intenta otra vez.';
            } finally {
                this.buscando = false;
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
                    const { svg, ms, version } = await r.json();
                    // Hubo deploy: la página es vieja. Se recarga sola en cuanto nadie la use
                    // (el kiosko no tiene teclado para Ctrl+Shift+R).
                    if (version && version !== document.querySelector(".kiosko")?.dataset.version) this.nuevaVersion = true;
                    // QR de la app: en su ficha y en la vitrina (siempre en la página, para que se
                    // renueve solo).
                    this.$refs.qrApp.innerHTML = svg;
                    this.$refs.qrVitrina.innerHTML = svg;
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
// La conexión solo está abierta con la app en pantalla: así el servidor sabe cuándo avisar por
// Web Push (caja.rs) y el celular no gasta batería en segundo plano.
document.addEventListener('alpine:init', () => {
    const PESOS = new Intl.NumberFormat('es-MX', { style: 'currency', currency: 'MXN' });

    Alpine.data('cuentaCaja', () => ({
        cuenta: null,
        // Al cobrar: monedero ganado y saldo nuevo (/cuenta/venta/{id}).
        premio: null,
        ventaPremio: null,

        fuente: null,

        init() {
            this.conectar();
            document.addEventListener('visibilitychange', () => {
                if (document.hidden) this.desconectar(); else this.conectar();
            });
        },

        conectar() {
            if (this.fuente) return;
            this.fuente = new EventSource('/cuenta/eventos');
            this.fuente.addEventListener('cuenta', (e) => this.mostrar(JSON.parse(e.data)));
            this.fuente.addEventListener('sin-cuenta', () => this.mostrar(null));
            // Ya está viendo la app: el aviso de la caja (si llegó) sobra.
            navigator.serviceWorker?.ready
                .then((r) => r.getNotifications({ tag: 'cuenta-caja' }))
                .then((avisos) => avisos.forEach((a) => a.close()))
                .catch(() => {});
        },

        desconectar() {
            this.fuente?.close();
            this.fuente = null;
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
                .then((p) => {
                    if (this.ventaPremio !== venta || !p) return;
                    this.premio = p;
                    // La tarjeta de saldo (monedero/app.html) se pone al día sin recargar.
                    if (p.tieneMonedero) window.dispatchEvent(new CustomEvent('saldo-monedero', { detail: p.saldo }));
                })
                .catch(() => {});
        },

        get cobrada() { return this.cuenta?.estado === 'cobrada'; },
        pesos(n) { return PESOS.format(n ?? 0); },
        cantidad(n) { return Number(n).toLocaleString('es-MX', { maximumFractionDigits: 3 }); },
    }));
});

// "Imprime tú solito" (templates/app/imprimir*.html, routes/imprimir.rs).
document.addEventListener('alpine:init', () => {
    // Subir el PDF: el tamaño se revisa antes de mandarlo (el servidor lo revisa otra vez).
    Alpine.data('subirPdf', (maxMb) => ({
        ocupado: false,
        error: '',

        elegido(e) {
            this.error = '';
            const archivo = e.target.files[0];
            if (!archivo) return;
            if (archivo.size > maxMb * 1024 * 1024) {
                this.error = `Tu PDF pesa más de ${maxMb} MB. Pídelo en caja.`;
                e.target.value = '';
                return;
            }
            e.target.form.requestSubmit();
        },

        enviar(e) {
            if (this.error) { e.preventDefault(); return; }
            this.ocupado = true;
        },
    }));

    // Opciones de un trabajo: la cuenta la saca el servidor (impresion::calcular), la misma que
    // se guarda, para que nunca difiera de lo que se cobra.
    // Opciones de un trabajo de impresión (imprimir_trabajo.html). Las páginas se eligen en una
    // cuadrícula de miniaturas con palomita (todas marcadas al principio) o escribiendo el rango;
    // las dos cosas se mantienen iguales. Lo que se manda es el texto `paginas` ("1-3,5"; vacío
    // = todas), el mismo que valida y guarda el servidor (impresion.rs).
    Alpine.data('opcionesImpresion', (inicial) => {
        // El documento de pdf.js va fuera del objeto de Alpine: su proxy reactivo rompe los
        // campos privados de pdf.js.
        let documento = null;
        let observador = null;
        let cola = Promise.resolve();          // una miniatura a la vez, para no trabar el celular

        return {
            n: inicial.paginasPdf,
            marcadas: leerPaginas(inicial.paginas, inicial.paginasPdf)
                      || Array(inicial.paginasPdf).fill(true),
            paginas: inicial.paginas.replaceAll(',', ', '),
            escribir: false,
            copias: inicial.copias,
            doble: inicial.doble,
            caras: inicial.caras,
            hojas: inicial.hojas,
            total: inicial.total,
            error: '',
            ocupado: false,
            pedido: 0,

            get cuantas() { return this.marcadas.filter(Boolean).length; },

            init() {
                const cuadricula = this.$el.querySelector('.imp-paginas');
                observador = new IntersectionObserver((entradas) => {
                    for (const e of entradas) {
                        if (!e.isIntersecting) continue;
                        observador.unobserve(e.target);
                        const num = Number(e.target.dataset.pagina);
                        cola = cola.then(() => this.dibujar(num, e.target.querySelector('canvas')));
                    }
                }, { root: cuadricula, rootMargin: '300px' });
                this.abrirPdf(cuadricula);
            },

            async abrirPdf(cuadricula) {
                try {
                    const pdfjs = await import('https://cdn.jsdelivr.net/npm/pdfjs-dist@4.10.38/build/pdf.min.mjs');
                    pdfjs.GlobalWorkerOptions.workerSrc =
                        'https://cdn.jsdelivr.net/npm/pdfjs-dist@4.10.38/build/pdf.worker.min.mjs';
                    documento = await pdfjs.getDocument(`/imprimir/${inicial.id}/pdf`).promise;
                    cuadricula.querySelectorAll('.imp-pagina').forEach((el) => observador.observe(el));
                } catch (e) {
                    // Sin miniaturas la cuadrícula sigue sirviendo: cada página con su número.
                    console.warn('imprimir: sin miniaturas', e);
                }
            },

            async dibujar(num, canvas) {
                if (!documento || !canvas) return;
                try {
                    const pagina = await documento.getPage(num);
                    const base = pagina.getViewport({ scale: 1 });
                    const ancho = canvas.parentElement.clientWidth * (window.devicePixelRatio || 1);
                    const vista = pagina.getViewport({ scale: ancho / base.width });
                    canvas.width = Math.floor(vista.width);
                    canvas.height = Math.floor(vista.height);
                    await pagina.render({ canvasContext: canvas.getContext('2d'), viewport: vista }).promise;
                    canvas.classList.add('lista');
                    pagina.cleanup();
                } catch (e) {
                    console.warn(`imprimir: página ${num}`, e);
                }
            },

            // Tocar una página, Todas o Ninguna: el texto se vuelve a escribir desde las palomitas.
            alternar(num) {
                this.marcadas[num - 1] = !this.marcadas[num - 1];
                this.desdeMarcadas();
            },
            todas(valor) {
                this.marcadas = Array(this.n).fill(valor);
                this.desdeMarcadas();
            },
            desdeMarcadas() {
                this.paginas = escribirPaginas(this.marcadas).replaceAll(',', ', ');
                this.calcular();
            },

            // Escribir el rango: si se entiende, mueve las palomitas; si no, el servidor dice qué
            // está mal.
            desdeTexto() {
                const m = leerPaginas(this.paginas, this.n);
                if (m) this.marcadas = m;
                this.calcular();
            },

            async calcular() {
                const n = ++this.pedido;          // solo vale la respuesta del último cambio
                if (!this.paginas.trim() && this.cuantas === 0) {
                    this.error = 'Elige al menos una página.';
                    this.ocupado = false;
                    return;
                }
                const q = new URLSearchParams({ paginas: this.paginas, copias: this.copias });
                if (this.doble) q.set('doble', '1');
                this.ocupado = true;
                try {
                    const r = await fetch(`/imprimir/${inicial.id}/cuenta?${q}`, { cache: 'no-store' });
                    const c = await r.json();
                    if (n !== this.pedido) return;
                    this.error = c.error || '';
                    if (!c.error) Object.assign(this, { caras: c.caras, hojas: c.hojas, total: c.total });
                } catch {
                    if (n === this.pedido) this.error = 'Sin conexión. Intenta otra vez.';
                } finally {
                    if (n === this.pedido) this.ocupado = false;
                }
            },
        };
    });
});

// "1-3, 5" → [true, true, true, false, true, …] para un PDF de `n` páginas; vacío = todas.
// null si no se entiende (el servidor da el mensaje; impresion.rs::leer_paginas).
function leerPaginas(texto, n) {
    const limpio = texto.trim();
    if (!limpio || /^todas$/i.test(limpio)) return Array(n).fill(true);
    const m = Array(n).fill(false);
    const partes = limpio.split(',').map((p) => p.trim()).filter(Boolean);
    if (!partes.length) return null;
    for (const parte of partes) {
        const r = parte.match(/^(\d+)\s*(?:-\s*(\d+))?$/);
        if (!r) return null;
        const a = Number(r[1]), b = Number(r[2] ?? r[1]);
        if (a < 1 || b < a || b > n) return null;
        for (let i = a; i <= b; i++) m[i - 1] = true;
    }
    return m;
}

// [true, true, true, false, true] → "1-3,5"; todas → "" (así lo guarda el servidor).
function escribirPaginas(marcadas) {
    if (marcadas.every(Boolean)) return '';
    const rangos = [];
    let inicio = 0;
    for (let i = 1; i <= marcadas.length + 1; i++) {
        const dentro = i <= marcadas.length && marcadas[i - 1];
        if (dentro && !inicio) inicio = i;
        if (!dentro && inicio) {
            rangos.push(inicio === i - 1 ? `${inicio}` : `${inicio}-${i - 1}`);
            inicio = 0;
        }
    }
    return rangos.join(',');
}

// ── Anuncios del kiosko (anuncio.html y la franja del kiosko) ──

// Letra de un anuncio de texto, como los estados de WhatsApp: la más grande con la que todo cabe
// sin partir palabras (poco texto, letra enorme). En % del ancho, así se ve igual en el celular
// que en el kiosko. Si ni con la más chica cabe, entonces sí parte palabras.
window.ajustarTexto = (el) => {
    if (!el.clientWidth) return;           // oculto: se ajusta cuando se vea
    el.style.overflowWrap = 'normal';
    for (let t = 16; t >= 4; t -= 0.5) {
        el.style.fontSize = (el.clientWidth * t / 100) + 'px';
        if (el.scrollHeight <= el.clientHeight && el.scrollWidth <= el.clientWidth) return;
    }
    el.style.overflowWrap = 'anywhere';
};

document.addEventListener('alpine:init', () => {
    // Poner o cambiar el anuncio: imagen (vista previa local antes de subirla) o texto con color.
    Alpine.data('anuncioForm', (o) => ({
        tipo: o.tipo,
        texto: o.texto,
        color: o.color,
        abierto: o.abierto,
        foto: '',
        acepto: false,
        error: '',
        ocupado: false,

        elegida(e) {
            this.error = '';
            const archivo = e.target.files[0];
            if (this.foto) URL.revokeObjectURL(this.foto);
            this.foto = '';
            if (!archivo) return;
            if (archivo.size > o.maxMb * 1024 * 1024) {
                this.error = `Tu imagen pesa más de ${o.maxMb} MB.`;
                e.target.value = '';
                return;
            }
            this.foto = URL.createObjectURL(archivo);
        },

        enviar(e) {
            if (this.tipo === 'imagen' && !this.foto) this.error = 'Elige una imagen.';
            else if (this.tipo === 'texto' && !this.texto.trim()) this.error = 'Escribe tu anuncio.';
            else if (!this.acepto) this.error = 'Para mandarlo, acepta las reglas.';
            else this.error = '';
            if (this.error) { e.preventDefault(); return; }
            this.ocupado = true;
        },
    }));
});

document.addEventListener('alpine:init', () => {
    // Franja derecha del kiosko (kiosko.html): los anuncios aprobados, uno cada 10 s, y al final
    // de la vuelta la invitación a poner el suyo (i === anuncios.length). Pide la lista cada 2 min.
    Alpine.data('kioskoAnuncios', () => ({
        anuncios: [],
        i: 0,
        desde: Date.now(),      // cuándo empezó el actual: cambia a los 10 s
        pausaHasta: 0,          // si alguien lo tocó, se queda quieto hasta entonces
        xToque: null,           // dónde empezó a deslizar el dedo

        init() {
            this.cargar();
            setInterval(() => this.cargar(), 2 * 60 * 1000);
            setInterval(() => {
                const ahora = Date.now();
                if (ahora >= this.pausaHasta && ahora - this.desde >= 10000) this.mover(1, false);
            }, 1000);
        },

        // Con los anuncios más la invitación del final.
        mover(paso, tocado = true) {
            const n = this.anuncios.length + 1;
            this.i = (this.i + paso + n) % n;
            this.desde = Date.now();
            // Quien lo recorre con el dedo lo quiere leer: 30 s quieto.
            if (tocado) this.pausaHasta = this.desde + 30000;
        },
        tocar(e) { this.xToque = e.clientX; },
        soltar(e) {
            if (this.xToque === null) return;
            const dx = e.clientX - this.xToque;
            this.xToque = null;
            if (Math.abs(dx) > 50) this.mover(dx < 0 ? 1 : -1);
        },

        async cargar() {
            try {
                const r = await fetch('/kiosko/anuncios');
                if (!r.ok) return;
                this.anuncios = await r.json();
                if (this.i > this.anuncios.length) this.i = 0;
            } catch { /* sin red: sigue con los que tenía */ }
        },
    }));
});

// Foto de credencial (fotos_credencial.html, routes/fotos.rs). Basado en el kiosko de imagina
// (CameraCapture.svelte): la cámara llena la pantalla y una ventana al tamaño elegido marca lo que
// sale; al disparar se toma ese rectángulo del sensor. Luego se acomoda (arrastrar y acercar) en un
// marco del tamaño, se hornea a los px de impresión (300 dpi) y se manda DIRECTO a imagina con el
// permiso firmado de xplaya (encabezado X-Permiso). imagina lo deja en el panel de la operadora.
document.addEventListener('alpine:init', () => {
    const DPI = 300;
    const px = (mm) => Math.round((mm / 25.4) * DPI);

    Alpine.data('fotoCredencial', () => ({
        // Infantil primero: es la que más se pide.
        tamanos: [
            { clave: 'infantil', nombre: 'Infantil', w: 25, h: 30, nota: 'La más pedida' },
            // También la del título profesional. Sin pasaporte ni visa: el consulado casi siempre
            // rechaza una foto de celular (xplaya.com/foto-credencial); esas, en la papelería.
            { clave: 'credencial', nombre: 'Credencial', w: 35, h: 45, nota: 'También título' },
        ],
        t: 0,
        paso: 'tamano',          // tamano → camara → ajuste → revisar → enviando
        error: '',

        // Cámara
        stream: null,
        facing: 'environment',   // la de atrás: casi siempre alguien más toma la foto
        puedeVoltear: false,
        iniciando: false,
        camaraError: '',
        cont: { w: 0, h: 0 },    // medidas de la pantalla de la cámara

        // Foto fuente y su acomodo en el marco
        fuenteUrl: '',
        img: { w: 0, h: 0 },     // px naturales
        marco: { w: 0, h: 0 },   // px del marco en pantalla
        zoom: 1,
        pos: { x: 0, y: 0 },     // esquina de la foto respecto al marco (≤ 0)
        arrastre: null,

        finalBlob: null,
        finalUrl: '',

        get tam() { return this.tamanos[this.t]; },
        cm(mm) { return String(mm / 10); },

        // ── Cámara ─────────────────────────────────────────────────────────
        async abrirCamara() {
            this.error = '';
            this.paso = 'camara';
            await this.$nextTick();
            this.medirCamara();
            await this.encender(this.facing);
        },

        medirCamara() {
            const c = this.$refs.camara;
            this.cont = { w: c.clientWidth, h: c.clientHeight };
        },

        // La ventana: el rectángulo más grande al tamaño elegido que deja aire para las barras.
        rect() {
            const { w: cw, h: ch } = this.cont;
            if (!cw || !ch) return { left: 0, top: 0, w: 0, h: 0 };
            const aspecto = this.tam.w / this.tam.h;
            let w = cw * 0.86;
            let h = w / aspecto;
            if (h > ch * 0.62) { h = ch * 0.62; w = h * aspecto; }
            return { left: (cw - w) / 2, top: (ch - h) / 2, w, h };
        },
        ventana() {
            const r = this.rect();
            return `left:${r.left}px;top:${r.top}px;width:${r.w}px;height:${r.h}px`;
        },

        async encender(quiere) {
            this.iniciando = true;
            this.camaraError = '';
            if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
                this.camaraError = 'Este navegador no deja usar la cámara aquí. Elige una foto o tómala con la cámara del celular.';
                this.iniciando = false;
                return;
            }
            try {
                // 1080p sobra para una credencial (3.5 × 4.5 cm a 300 dpi = 413 × 531 px); pedir el
                // máximo del sensor deja a algunos celulares con el autofoco lento.
                const nuevo = await navigator.mediaDevices.getUserMedia({
                    video: { facingMode: { ideal: quiere }, width: { ideal: 1920 }, height: { ideal: 1080 },
                             advanced: [{ focusMode: 'continuous' }] },
                    audio: false,
                });
                this.apagarCamara();
                this.stream = nuevo;
                this.facing = quiere;
                const v = this.$refs.video;
                v.srcObject = nuevo;
                await v.play().catch(() => {});
                // Muchos Android abren con foco fijo: se pide el continuo (iOS lo hace solo).
                const pista = nuevo.getVideoTracks()[0];
                try {
                    if (pista?.getCapabilities?.().focusMode?.includes('continuous')) {
                        await pista.applyConstraints({ advanced: [{ focusMode: 'continuous' }] });
                    }
                } catch { /* sin control de foco */ }
                try {
                    const disp = await navigator.mediaDevices.enumerateDevices();
                    this.puedeVoltear = disp.filter((d) => d.kind === 'videoinput').length > 1;
                } catch { this.puedeVoltear = false; }
            } catch (e) {
                const n = e?.name || '';
                this.camaraError = n === 'NotAllowedError' || n === 'SecurityError'
                    ? 'No diste permiso para la cámara. Puedes elegir una foto o tomarla con la cámara del celular.'
                    : n === 'NotFoundError' || n === 'OverconstrainedError'
                        ? 'No encontramos una cámara. Elige una foto.'
                        : 'No se pudo abrir la cámara. Elige una foto o tómala con la cámara del celular.';
            } finally {
                this.iniciando = false;
            }
        },

        voltear() { this.encender(this.facing === 'user' ? 'environment' : 'user'); },

        apagarCamara() {
            this.stream?.getTracks().forEach((p) => p.stop());
            this.stream = null;
        },

        cerrarCamara() {
            this.apagarCamara();
            this.paso = 'tamano';
        },

        // La ventana (coordenadas de la pantalla) → píxeles del sensor. El video llena la pantalla
        // recortado al centro (object-fit: cover): se compensa ese recorte.
        disparar() {
            const v = this.$refs.video;
            const r = this.rect();
            if (!v?.videoWidth || !r.w) return;
            const { w: cw, h: ch } = this.cont;
            const vw = v.videoWidth, vh = v.videoHeight;
            const escala = Math.max(cw / vw, ch / vh);
            const offX = (vw * escala - cw) / 2, offY = (vh * escala - ch) / 2;
            const sx = Math.max(0, (offX + r.left) / escala);
            const sy = Math.max(0, (offY + r.top) / escala);
            const sw = Math.min(r.w / escala, vw - sx);
            const sh = Math.min(r.h / escala, vh - sy);
            const lienzo = document.createElement('canvas');
            lienzo.width = Math.round(sw);
            lienzo.height = Math.round(sh);
            lienzo.getContext('2d').drawImage(v, sx, sy, sw, sh, 0, 0, lienzo.width, lienzo.height);
            lienzo.toBlob((b) => b && this.usarFoto(b), 'image/jpeg', 0.95);
        },

        elegida(e) {
            const archivo = e.target.files[0];
            e.target.value = '';
            if (!archivo) return;
            if (!archivo.type.startsWith('image/')) { this.error = 'Elige una foto.'; return; }
            this.usarFoto(archivo);
        },

        // ── Acomodar ───────────────────────────────────────────────────────
        async usarFoto(blob) {
            this.apagarCamara();
            if (this.fuenteUrl) URL.revokeObjectURL(this.fuenteUrl);
            this.fuenteUrl = URL.createObjectURL(blob);
            const im = new Image();
            try {
                await new Promise((ok, mal) => { im.onload = ok; im.onerror = mal; im.src = this.fuenteUrl; });
            } catch {
                this.error = 'No pudimos abrir esa foto. Prueba con otra (JPG o PNG).';
                this.paso = 'tamano';
                return;
            }
            this.img = { w: im.naturalWidth, h: im.naturalHeight };
            this.zoom = 1;
            this.zoomPrevio = 1;
            this.paso = 'ajuste';
            // Se mide ya pintado: x-show lo muestra después del siguiente cuadro.
            await new Promise((listo) => requestAnimationFrame(() => requestAnimationFrame(listo)));
            this.medirMarco();
        },

        // Medidas del marco en pantalla y la foto centrada, cubriéndolo todo.
        medirMarco() {
            const m = this.$refs.marco;
            this.marco = { w: m.clientWidth, h: m.clientHeight };
            const e = this.escala();
            this.pos = { x: (this.marco.w - this.img.w * e) / 2, y: (this.marco.h - this.img.h * e) / 2 };
        },

        // px de pantalla por px de la foto: lo justo para cubrir el marco, por el zoom.
        escala() {
            if (!this.img.w || !this.marco.w) return 0;
            return Math.max(this.marco.w / this.img.w, this.marco.h / this.img.h) * this.zoom;
        },

        // Que la foto siempre cubra el marco (sin huecos en las orillas).
        acotar(x, y) {
            const e = this.escala();
            return {
                x: Math.min(0, Math.max(this.marco.w - this.img.w * e, x)),
                y: Math.min(0, Math.max(this.marco.h - this.img.h * e, y)),
            };
        },

        tomar(ev) {
            ev.currentTarget.setPointerCapture(ev.pointerId);
            this.arrastre = { x: ev.clientX, y: ev.clientY, pos: { ...this.pos } };
        },
        mover(ev) {
            if (!this.arrastre) return;
            const a = this.arrastre;
            this.pos = this.acotar(a.pos.x + ev.clientX - a.x, a.pos.y + ev.clientY - a.y);
        },
        soltar() { this.arrastre = null; },

        // Al acercar, el centro del marco se queda en el mismo punto de la foto.
        acercar() {
            const anterior = this.zoomPrevio || 1;
            const cx = this.marco.w / 2, cy = this.marco.h / 2;
            const f = this.zoom / anterior;
            this.pos = this.acotar(cx - (cx - this.pos.x) * f, cy - (cy - this.pos.y) * f);
            this.zoomPrevio = this.zoom;
        },
        zoomPrevio: 1,

        // Lo que se ve en el marco → JPEG a los px de impresión exactos.
        hornear() {
            const e = this.escala();
            const ancho = px(this.tam.w), alto = px(this.tam.h);
            const lienzo = document.createElement('canvas');
            lienzo.width = ancho;
            lienzo.height = alto;
            const c = lienzo.getContext('2d');
            c.imageSmoothingQuality = 'high';
            c.drawImage(this.$refs.fuente, -this.pos.x / e, -this.pos.y / e, this.marco.w / e, this.marco.h / e, 0, 0, ancho, alto);
            lienzo.toBlob((b) => {
                if (!b) return;
                if (this.finalUrl) URL.revokeObjectURL(this.finalUrl);
                this.finalBlob = b;
                this.finalUrl = URL.createObjectURL(b);
                this.error = '';
                this.paso = 'revisar';
            }, 'image/jpeg', 0.92);
        },

        // ── Mandar a imagina ───────────────────────────────────────────────
        async permiso() {
            const r = await fetch('/fotos/permiso', { cache: 'no-store' });
            if (r.status === 401) throw new Error('Tu sesión se cerró. Vuelve a entrar a la app.');
            if (!r.ok) throw new Error('No pudimos preparar el envío. Intenta de nuevo.');
            return r.json();
        },

        async mandar() {
            if (!this.finalBlob) return;
            this.paso = 'enviando';
            this.error = '';
            try {
                let intento = 0;
                for (;;) {
                    const { permiso, imagina } = await this.permiso();
                    const datos = new FormData();
                    datos.append('file', this.finalBlob, 'credencial.jpg');
                    datos.append('widthMM', String(this.tam.w));
                    datos.append('heightMM', String(this.tam.h));
                    datos.append('dpi', String(DPI));
                    const r = await fetch(`${imagina}/api/v1/kiosko/carnet`, {
                        method: 'POST', headers: { 'X-Permiso': permiso }, body: datos,
                    });
                    const cuerpo = await r.json().catch(() => ({}));
                    if (r.ok) {
                        location.href = '/fotos?mandado=' + encodeURIComponent(cuerpo.orderCode || '');
                        return;
                    }
                    // Permiso vencido (se tardó mucho): uno nuevo y otra vez, una sola vez.
                    if (r.status === 400 && /permiso/i.test(cuerpo.error || '') && intento++ === 0) continue;
                    throw new Error(cuerpo.error || 'No pudimos mandar tu foto. Intenta de nuevo.');
                }
            } catch (e) {
                this.error = e instanceof TypeError ? 'Sin conexión. Revisa tu internet e intenta de nuevo.' : e.message;
                this.paso = 'revisar';
            }
        },

        init() {
            window.addEventListener('resize', () => {
                if (this.paso === 'camara') this.medirCamara();
                if (this.paso === 'ajuste') this.medirMarco();
            });
        },
    }));
});

// Barra de abajo y hoja de "Más" (templates/app/nav.html).
// - La pestaña Chat solo sale a quien lo puede ver: lo dice /chat/sin-leer (404 si no). La
//   respuesta se recuerda en el celular para que la barra no brinque al cargar cada página.
// - Tamaño de letra: % del <html> (todo lo de app.css va en rem). Se aplica antes de pintar con
//   el script del <head> de app/base.html, que lee la misma llave.
const LETRA_LLAVE = 'xp_letra';
const LETRA_PASOS = [87.5, 100, 112.5, 125, 140];
const LETRA_NOMBRES = ['Chica', 'Normal', 'Grande', 'Más grande', 'Enorme'];
const CHAT_VISIBLE_LLAVE = 'xp_chat_visible';

function leerLocal(llave) {
    try { return localStorage.getItem(llave); } catch { return null; }
}

function guardarLocal(llave, valor) {
    try { localStorage.setItem(llave, valor); } catch { /* sin almacenamiento */ }
}

document.addEventListener('alpine:init', () => {
    Alpine.data('menuApp', () => ({
        mas: false,
        chat: location.pathname === '/chat' || leerLocal(CHAT_VISIBLE_LLAVE) === '1',
        sinLeer: 0,
        pasos: LETRA_PASOS,
        paso: Math.max(0, LETRA_PASOS.indexOf(Number(leerLocal(LETRA_LLAVE) || 100))),

        get nombreLetra() { return LETRA_NOMBRES[this.paso]; },

        init() {
            this.revisarChat();
            // El chat marcó leído (chat.js): el globito se va.
            window.addEventListener('chat-leido', () => { this.sinLeer = 0; });
        },

        async revisarChat() {
            try {
                const r = await fetch('/chat/sin-leer');
                if (r.ok) {
                    this.chat = true;
                    this.sinLeer = location.pathname === '/chat' ? 0 : (await r.json()).sinLeer;
                } else if (r.status === 404 || r.status === 401) {
                    this.chat = false;
                }
                guardarLocal(CHAT_VISIBLE_LLAVE, this.chat ? '1' : '0');
            } catch { /* sin red: se queda lo recordado */ }
        },

        abrir() {
            this.mas = true;
            document.documentElement.classList.add('sin-scroll');
        },

        cerrar() {
            this.mas = false;
            document.documentElement.classList.remove('sin-scroll');
        },

        letraMenos() { this.ponerLetra(this.paso - 1); },
        letraMas() { this.ponerLetra(this.paso + 1); },

        ponerLetra(k) {
            this.paso = Math.min(Math.max(k, 0), LETRA_PASOS.length - 1);
            const pct = LETRA_PASOS[this.paso];
            document.documentElement.style.fontSize = pct === 100 ? '' : pct + '%';
            guardarLocal(LETRA_LLAVE, String(pct));
        },
    }));
});
