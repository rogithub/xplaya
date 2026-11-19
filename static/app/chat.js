// Chat con la papelería (templates/app/chat.html, routes/chat.rs, CHAT.md).
//
// - Los mensajes llegan del servidor ya guardados: al abrir (JSON en la página), al subir
//   (/chat/anteriores) y en vivo por SSE (/chat/eventos).
// - Lo que escribe el cliente se queda en "enviando…" hasta que el servidor dice que NATS lo
//   tiene (✓). Sin señal se guarda en localStorage y se vuelve a mandar al regresar la conexión;
//   el id lo pone el celular, así un reintento nunca sale dos veces.
// - ✓✓ cuando la papelería lo leyó (evento `leido` con `delCliente`).

const CHAT_PENDIENTES = 'xp_chat_pendientes';
const CHAT_REINTENTO_MS = 15000;
const CHAT_ESCRIBIENDO_MS = 3000;  // cada cuánto se avisa que el cliente escribe
const CHAT_ESCRIBIENDO_VE_MS = 5000; // cuánto se ve "escribiendo…" de la papelería

// Fechas del servidor: hora de la tienda (Cancún, UTC−5) sin zona y con microsegundos.
function chatFecha(texto) {
    return new Date(texto.replace(/(\.\d{3})\d+/, '$1') + '-05:00');
}

function chatId() {
    if (window.crypto && crypto.randomUUID) return crypto.randomUUID();
    const b = crypto.getRandomValues(new Uint8Array(16));
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    const h = [...b].map((x) => x.toString(16).padStart(2, '0')).join('');
    return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

const CHAT_VISIBLES = 4; // con más archivos se ven 4 y el último dice "+N"

function chatPeso(bytes) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function chatIcono(a) {
    if (a.clase === 'imagen') return '🖼️';
    if (a.clase === 'video') return '🎬';
    if (a.clase === 'audio') return '🎵';
    if (a.clase === 'pdf') return '📄';
    if (a.clase === 'office') {
        if (/\.(xlsx?|ods|csv)$/i.test(a.nombre)) return '📊';
        if (/\.(pptx?|odp)$/i.test(a.nombre)) return '📽️';
        return '📝';
    }
    return '📎';
}

// Una foto o un video que se puede dibujar (los locales que regresan de localStorage no traen URL).
function chatEsMedio(a) { return (a.clase === 'imagen' || a.clase === 'video') && !!(a.mini || a.original); }

// ── Subir archivos ──
// Los `File` que se están subiendo, por id de mensaje. No van a localStorage: si se cierra la app
// a media subida, ese mensaje se pierde. Ya subidos, el mensaje sí se guarda como pendiente (con
// los números que dio el servidor, que los guarda una hora).
const chatSubidas = new Map();
const CHAT_SUBIDAS_A_LA_VEZ = 3;
const CHAT_INTENTOS = 3;

// Qué es, solo para la vista previa y el límite; el servidor lo decide por los bytes.
function chatClaseLocal(file) {
    if (/^image\/(jpeg|png|webp|gif)$/.test(file.type)) return 'imagen';
    if (file.type.startsWith('video/')) return 'video';
    if (file.type.startsWith('audio/')) return 'audio';
    if (file.type === 'application/pdf') return 'pdf';
    if (/\.(docx?|xlsx?|pptx?|odt|ods|odp|rtf)$/i.test(file.name)) return 'office';
    return 'archivo';
}

function chatSoltarUrls(adjuntos) {
    (adjuntos || []).forEach((a) => {
        [a.original, a.mini, a.vista].forEach((u) => { if (u && u.startsWith('blob:')) URL.revokeObjectURL(u); });
    });
}

// POST /chat/archivo con XMLHttpRequest para tener el avance de la subida.
function chatSubir(file, mensaje, alAvanzar) {
    return new Promise((ok, mal) => {
        const x = new XMLHttpRequest();
        x.open('POST', '/chat/archivo');
        x.upload.onprogress = (e) => { if (e.lengthComputable) alAvanzar(e.loaded); };
        x.onload = () => {
            let j = {};
            try { j = JSON.parse(x.responseText); } catch { /* no es JSON */ }
            if (x.status === 200 && j.archivo) ok(j);
            else mal({ status: x.status, error: j.error });
        };
        x.onerror = () => mal({ status: 0 });
        const f = new FormData();
        f.append('mensaje', mensaje);  // primero el mensaje: así lo pide el servidor
        f.append('archivo', file, file.name);
        x.send(f);
    });
}

// ── Notas de voz ──
// Opus en webm donde se puede (Android, computadora); el iPhone graba mp4/AAC. El servidor
// reconoce la nota por el nombre: `.weba` o `.m4a` (un `.webm` lo tomaría por video).
const CHAT_NOTA_MAX_S = 5 * 60;
let chatGrabadora = null;   // MediaRecorder: fuera de Alpine
let chatPedazos = [];

function chatEsperar(ms) { return new Promise((r) => setTimeout(r, ms)); }

// Para 🔊 y para la cita cuando el mensaje no tiene texto.
function chatQueEs(adjuntos) {
    if (!adjuntos || !adjuntos.length) return '';
    if (adjuntos.length > 1) return `${adjuntos.length} archivos`;
    const nombres = { imagen: 'Foto', video: 'Video', audio: 'Nota de voz', pdf: 'Documento', office: 'Documento' };
    return nombres[adjuntos[0].clase] || 'Archivo';
}

// ── Links y tarjetas ──
// Los links del texto se vuelven <a> armando pedazos (sin x-html): solo http(s), así un
// "javascript:" escrito en el chat nunca es un link.
const CHAT_LINK = /(?:https?:\/\/|www\.)[^\s<>"]+|\b(?:app\.)?xplaya\.com\/[^\s<>"]*/gi;

function chatPartes(texto) {
    const partes = [];
    let desde = 0;
    for (const m of texto.matchAll(CHAT_LINK)) {
        // La puntuación del final de la oración no es parte del link.
        const link = m[0].replace(/[.,;:!?)\]'"]+$/, '');
        if (m.index > desde) partes.push({ t: texto.slice(desde, m.index) });
        partes.push({ t: link, url: /^https?:\/\//i.test(link) ? link : `https://${link}` });
        desde = m.index + link.length;
    }
    if (desde < texto.length) partes.push({ t: texto.slice(desde) });
    return partes;
}

// Lo que dice la tarjeta en la cita y en 🔊 cuando el mensaje no trae texto.
function chatQueTarjeta(t) {
    if (!t) return '';
    if (t.tipo === 'producto') return t.nombre;
    if (t.tipo === 'recibo') return `Recibo del ${t.fecha}`;
    return `Cotización ${t.numero}`;
}

function chatLeerPendientes() {
    try { return JSON.parse(localStorage.getItem(CHAT_PENDIENTES) || '[]'); } catch { return []; }
}

function chatGuardarPendientes(lista) {
    try { localStorage.setItem(CHAT_PENDIENTES, JSON.stringify(lista)); } catch { /* sin almacenamiento */ }
}

document.addEventListener('alpine:init', () => {
    Alpine.data('chatApp', () => ({
        mensajes: [],
        bloqueado: false,
        minutosEditar: 15,
        hayMas: false,
        cargandoMas: false,
        texto: '',
        respondiendo: null,   // mensaje citado
        editando: null,       // mensaje que se edita
        abierto: null,        // id del mensaje con su menú abierto
        escribe: false,       // la papelería escribe
        error: '',
        visor: null,          // foto abierta en grande
        elegidos: [],         // archivos con 📎, antes de mandar: {id, nombre, clase, bytes, vista}
        permiteArchivos: false,
        bytesVideo: 0,
        bytesDocumento: 0,
        maxAdjuntos: 100,
        puedeGrabar: false,
        grabando: false,
        segundos: 0,
        _reloj: null,
        _cancelada: false,
        puedeLeer: 'speechSynthesis' in window,
        _ultimoEscribiendo: 0,
        _escribeHasta: null,
        _eventos: null,
        _conectado: false,

        init() {
            const datos = JSON.parse(document.getElementById('chat-datos').textContent);
            this.bloqueado = datos.bloqueado;
            this.minutosEditar = datos.minutosEditar;
            this.hayMas = datos.hayMas;
            this.permiteArchivos = !!datos.archivos;
            this.bytesVideo = datos.bytesVideo;
            this.bytesDocumento = datos.bytesDocumento;
            this.maxAdjuntos = datos.maxAdjuntos;
            this.puedeGrabar = this.permiteArchivos && 'MediaRecorder' in window
                && !!(navigator.mediaDevices && navigator.mediaDevices.getUserMedia);
            this.mensajes = datos.mensajes.map((m) => ({ ...m, estado: 'guardado' }));

            // Lo que quedó sin mandar (sin señal, app cerrada).
            const guardados = new Set(this.mensajes.map((m) => m.id));
            const pendientes = chatLeerPendientes().filter((p) => !guardados.has(p.id));
            chatGuardarPendientes(pendientes);
            pendientes.forEach((p) => this.mensajes.push(this.local(p, 'enviando')));

            this.conectar();
            window.addEventListener('online', () => this.reintentar());
            setInterval(() => this.reintentar(), CHAT_REINTENTO_MS);
            document.addEventListener('visibilitychange', () => this.marcarLeido());
            this.reintentar();
            this.marcarLeido();
            this.colocarRedactor();
            this.$nextTick(() => this.alFinal(false));
        },

        // ── Mostrar ──

        local(p, estado) {
            return {
                id: p.id, autor: 'cliente', tipo: 'texto', texto: p.texto, respondeA: p.respondeA || null,
                cita: p.cita || null, fecha: p.fecha, leido: false, editado: false, eliminado: false, estado,
                adjuntos: (p.adjuntos || []).map((a) => ({ ...a, local: true })), zip: null,
            };
        },

        // ── Archivos ──
        // Fotos y videos primero (cuadrícula), luego audio y documentos. Con más de 4 se ven los
        // primeros 4 de ese orden y el último lleva `mas` (cuántos faltan) hasta que se toca.

        visibles(m) {
            const todos = [...(m.adjuntos || [])].sort((a, b) => chatEsMedio(b) - chatEsMedio(a));
            if (m._todos || todos.length <= CHAT_VISIBLES) return todos.map((a) => ({ a, mas: 0 }));
            return todos.slice(0, CHAT_VISIBLES).map((a, i) => ({
                a, mas: i === CHAT_VISIBLES - 1 ? todos.length - CHAT_VISIBLES : 0,
            }));
        },

        medios(m) { return this.visibles(m).filter((v) => chatEsMedio(v.a)); },
        docs(m) { return this.visibles(m).filter((v) => !chatEsMedio(v.a)); },
        icono(a) { return chatIcono(a); },
        partes(texto) { return chatPartes(texto || ''); },

        // La tarjeta en tres renglones: qué es, el título y el dato principal.
        tarjeta(t) {
            if (t.tipo === 'producto') {
                const hay = parseFloat(t.existencia) > 0 ? 'Hay en tienda' : 'Pregunta si hay';
                return { que: 'Producto', titulo: t.nombre, dato: `${t.precio} · ${hay}`, icono: '🛍️' };
            }
            if (t.tipo === 'recibo') return { que: 'Recibo', titulo: `${t.fecha} · ${t.hora}`, dato: `Total ${t.total}`, icono: '🧾' };
            return { que: 'Cotización', titulo: `Núm. ${t.numero}`, dato: `${t.estatus} · Total ${t.total}`, icono: '📋' };
        },
        puedeReintentar(m) { return chatSubidas.has(m.id); },

        detalle(a) {
            const paginas = a.paginas ? `${a.paginas} ${a.paginas === 1 ? 'página' : 'páginas'} · ` : '';
            return paginas + chatPeso(a.bytes);
        },

        mio(m) { return m.autor === 'cliente'; },
        aviso(m) { return m.tipo === 'aviso'; },

        hora(m) {
            return chatFecha(m.fecha).toLocaleTimeString('es-MX', { hour: 'numeric', minute: '2-digit', timeZone: 'America/Cancun' });
        },

        dia(m) {
            const f = chatFecha(m.fecha);
            const clave = (d) => d.toLocaleDateString('es-MX', { timeZone: 'America/Cancun' });
            const hoy = new Date();
            const ayer = new Date(hoy.getTime() - 86400000);
            if (clave(f) === clave(hoy)) return 'Hoy';
            if (clave(f) === clave(ayer)) return 'Ayer';
            return f.toLocaleDateString('es-MX', { weekday: 'long', day: 'numeric', month: 'long', timeZone: 'America/Cancun' });
        },

        nuevoDia(i) {
            return i === 0 || this.dia(this.mensajes[i]) !== this.dia(this.mensajes[i - 1]);
        },

        // "Enviando…", ✓ o ✓✓ (solo en los del cliente).
        marca(m) {
            if (m.estado === 'enviando') return '🕓';
            if (m.estado === 'error') return '⚠️';
            return m.leido ? '✓✓' : '✓';
        },

        etiquetaMarca(m) {
            if (m.estado === 'enviando') return 'Enviando…';
            if (m.estado === 'error') return 'No se mandó';
            return m.leido ? 'Leído' : 'Enviado';
        },

        textoCita(m) {
            const citado = this.mensajes.find((x) => x.id === m.respondeA);
            if (citado) {
                if (citado.eliminado) return 'Mensaje eliminado';
                return citado.texto || chatQueEs(citado.adjuntos) || chatQueTarjeta(citado.tarjeta);
            }
            return m.cita || 'Mensaje eliminado';
        },

        autorCita(m) {
            const citado = this.mensajes.find((x) => x.id === m.respondeA);
            return citado && this.mio(citado) ? 'Tú' : 'Papelería';
        },

        cerca() {
            const resto = document.documentElement.scrollHeight - window.scrollY - window.innerHeight;
            return resto < 200;
        },

        alFinal(suave = true) {
            window.scrollTo({ top: document.documentElement.scrollHeight, behavior: suave ? 'smooth' : 'auto' });
        },

        // El cuadro de texto va fijo sobre la barra de abajo; su alto cambia con el tamaño de letra.
        colocarRedactor() {
            const nav = document.querySelector('.app-nav');
            const r = this.$refs.redactor;
            if (!r) return;
            const ajustar = () => {
                r.style.bottom = (nav ? nav.offsetHeight : 0) + 'px';
                this.$el.style.paddingBottom = (r.offsetHeight + 12) + 'px';
            };
            ajustar();
            if ('ResizeObserver' in window) {
                const ro = new ResizeObserver(ajustar);
                ro.observe(r);
                if (nav) ro.observe(nav);
            }
        },

        // ── En vivo ──

        conectar() {
            const es = new EventSource('/chat/eventos');
            this._eventos = es;
            es.addEventListener('open', () => {
                // Al reconectar puede haber llegado algo mientras no había conexión.
                if (this._conectado) this.ponerseAlDia();
                this._conectado = true;
                this.reintentar();
            });
            const datos = (e) => JSON.parse(e.data);
            es.addEventListener('mensaje', (e) => this.llego(datos(e)));
            es.addEventListener('leido', (e) => {
                const d = datos(e);
                const hasta = chatFecha(d.hasta);
                this.mensajes.forEach((m) => {
                    if (this.mio(m) === d.delCliente && m.estado === 'guardado' && chatFecha(m.fecha) <= hasta) m.leido = true;
                });
            });
            es.addEventListener('editado', (e) => {
                const d = datos(e);
                const m = this.mensajes.find((x) => x.id === d.id);
                if (m) { m.texto = d.texto; m.editado = true; }
            });
            es.addEventListener('eliminado', (e) => {
                const d = datos(e);
                const m = this.mensajes.find((x) => x.id === d.id);
                if (m) { m.texto = null; m.eliminado = true; m.adjuntos = []; m.zip = null; m.tarjeta = null; }
                if (this.visor && this.visor.original.includes(d.id)) this.visor = null;
            });
            es.addEventListener('escribiendo', () => {
                this.escribe = true;
                clearTimeout(this._escribeHasta);
                this._escribeHasta = setTimeout(() => { this.escribe = false; }, CHAT_ESCRIBIENDO_VE_MS);
            });
            es.addEventListener('recargar', () => this.ponerseAlDia());
            // Sin sesión o sin permiso: EventSource reintenta solo; no hay nada que hacer aquí.
        },

        llego(v) {
            const abajo = this.cerca();
            const i = this.mensajes.findIndex((x) => x.id === v.id);
            const nuevo = { ...v, estado: 'guardado' };
            if (i >= 0) {
                // Uno propio que estaba enviándose: ya está guardado.
                if (this.visor && this.visor.local) this.visor = null;
                chatSoltarUrls(this.mensajes[i].adjuntos);
                chatSubidas.delete(v.id);
                this.mensajes[i] = { ...nuevo, cita: this.mensajes[i].cita || v.cita };
                this.quitarPendiente(v.id);
            } else {
                this.mensajes.push(nuevo);
                this.ordenar();
            }
            if (!this.mio(v)) {
                this.escribe = false;
                this.marcarLeido();
            }
            if (abajo || this.mio(v)) this.$nextTick(() => this.alFinal());
        },

        ordenar() {
            // Los que no se han guardado van al final, en el orden en que se escribieron.
            const peso = (m) => (m.estado === 'guardado' ? 0 : 1);
            this.mensajes.sort((a, b) => peso(a) - peso(b) || chatFecha(a.fecha) - chatFecha(b.fecha));
        },

        async ponerseAlDia() {
            try {
                const r = await fetch('/chat/anteriores');
                if (!r.ok) return;
                (await r.json()).forEach((v) => {
                    const i = this.mensajes.findIndex((x) => x.id === v.id);
                    if (i >= 0) {
                        this.mensajes[i] = { ...v, estado: 'guardado' };
                        this.quitarPendiente(v.id);
                    } else {
                        this.mensajes.push({ ...v, estado: 'guardado' });
                    }
                });
                this.ordenar();
                this.marcarLeido();
            } catch { /* sin red: el siguiente `open` lo vuelve a intentar */ }
        },

        async masAntiguos() {
            const primero = this.mensajes.find((m) => m.estado === 'guardado');
            if (!primero || this.cargandoMas) return;
            this.cargandoMas = true;
            const alto = document.documentElement.scrollHeight;
            try {
                const r = await fetch('/chat/anteriores?antes=' + encodeURIComponent(primero.id));
                if (r.ok) {
                    const viejos = await r.json();
                    this.hayMas = viejos.length > 0;
                    this.mensajes.unshift(...viejos.map((v) => ({ ...v, estado: 'guardado' })));
                    // Que la pantalla no salte: se queda en el mensaje que se estaba viendo.
                    this.$nextTick(() => window.scrollBy(0, document.documentElement.scrollHeight - alto));
                }
            } catch { /* sin red */ }
            this.cargandoMas = false;
        },

        // ── Leído ──

        marcarLeido() {
            if (document.visibilityState !== 'visible') return;
            const sinLeer = this.mensajes.filter((m) => !this.mio(m) && !m.leido && !m.eliminado);
            if (!sinLeer.length) return;
            sinLeer.forEach((m) => { m.leido = true; });
            fetch('/chat/leido', { method: 'POST' }).catch(() => {});
            window.dispatchEvent(new CustomEvent('chat-leido'));
        },

        // ── Mandar ──

        escribiendo() {
            const ahora = Date.now();
            if (ahora - this._ultimoEscribiendo < CHAT_ESCRIBIENDO_MS) return;
            this._ultimoEscribiendo = ahora;
            fetch('/chat/escribiendo', { method: 'POST' }).catch(() => {});
        },

        crecer(el) {
            el.style.height = 'auto';
            el.style.height = Math.min(el.scrollHeight, 160) + 'px';
        },

        teclear(e) {
            // En computadora, Enter manda y Shift+Enter hace otra línea. En el celular, el botón.
            if (e.key === 'Enter' && !e.shiftKey && window.matchMedia('(hover: hover)').matches) {
                e.preventDefault();
                this.enviar();
            }
        },

        async enviar() {
            if (this.grabando) return this.terminarNota();
            const texto = this.texto;
            if (this.editando) {
                if (!texto.trim()) return;
                this.error = '';
                return this.guardarEdicion(texto);
            }
            if ((!texto.trim() && !this.elegidos.length) || this.bloqueado) return;
            this.error = '';
            if (this.elegidos.length) return this.enviarConArchivos(texto);

            const p = {
                id: chatId(),
                texto,
                respondeA: this.respondiendo ? this.respondiendo.id : null,
                cita: this.respondiendo ? (this.respondiendo.texto || chatQueEs(this.respondiendo.adjuntos)) : null,
                // Hora de la tienda, como las del servidor. Solo sirve para ordenar mientras se
                // manda; la de verdad la pone el servidor.
                fecha: new Date(Date.now() - 5 * 3600000).toISOString().slice(0, 23),
            };
            chatGuardarPendientes([...chatLeerPendientes(), p]);
            this.mensajes.push(this.local(p, 'enviando'));
            this.texto = '';
            this.respondiendo = null;
            this.$nextTick(() => {
                this.crecer(this.$refs.caja);
                this.alFinal();
            });
            await this.mandarUno(p);
        },

        async mandarUno(p) {
            const m = this.mensajes.find((x) => x.id === p.id);
            try {
                const r = await fetch('/chat/mensaje', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ id: p.id, texto: p.texto, respondeA: p.respondeA, archivos: p.archivos || [] }),
                });
                if (r.ok) {
                    this.quitarPendiente(p.id);
                    if (m && m.estado === 'enviando') m.estado = 'enviado';
                } else if (r.status === 400) {
                    // No se va a poder (vacío, muy largo, bloqueado): no se reintenta.
                    const j = await r.json().catch(() => ({}));
                    this.quitarPendiente(p.id);
                    if (m) m.estado = 'error';
                    this.error = j.error || 'No se pudo mandar.';
                }
                // 401, 503 o sin red: se queda en "enviando…" y se reintenta.
            } catch { /* sin red */ }
        },

        reintentar() {
            if (!navigator.onLine) return;
            chatLeerPendientes().forEach((p) => {
                const m = this.mensajes.find((x) => x.id === p.id);
                if (m && m.estado === 'enviando') this.mandarUno(p);
            });
        },

        quitarPendiente(id) {
            chatGuardarPendientes(chatLeerPendientes().filter((p) => p.id !== id));
        },

        descartar(m) {
            chatSoltarUrls(m.adjuntos);
            chatSubidas.delete(m.id);
            this.mensajes = this.mensajes.filter((x) => x.id !== m.id);
            this.quitarPendiente(m.id);
            this.error = '';
        },

        // ── Archivos: elegir y subir ──

        elegir(e) {
            const errores = [];
            for (const file of e.target.files) {
                if (this.elegidos.length >= this.maxAdjuntos) {
                    errores.push(`Van ${this.maxAdjuntos} archivos como máximo por mensaje.`);
                    break;
                }
                const clase = chatClaseLocal(file);
                const tope = clase === 'video' ? this.bytesVideo : this.bytesDocumento;
                if (file.size > tope) {
                    errores.push(`"${file.name}" pasa de ${Math.round(tope / 1024 / 1024)} MB.`);
                    continue;
                }
                if (!file.size) {
                    errores.push(`"${file.name}" está vacío.`);
                    continue;
                }
                const id = chatId();
                chatSubidas.set('elegido:' + id, file);
                this.elegidos.push({
                    id, nombre: file.name, clase, bytes: file.size,
                    vista: clase === 'imagen' ? URL.createObjectURL(file) : null,
                });
            }
            this.error = errores.join(' ');
            e.target.value = '';  // que se pueda volver a elegir el mismo
        },

        quitarElegido(e) {
            chatSoltarUrls([e]);
            chatSubidas.delete('elegido:' + e.id);
            this.elegidos = this.elegidos.filter((x) => x.id !== e.id);
        },

        async enviarConArchivos(texto) {
            const p = {
                id: chatId(),
                texto,
                respondeA: this.respondiendo ? this.respondiendo.id : null,
                cita: this.respondiendo ? (this.respondiendo.texto || chatQueEs(this.respondiendo.adjuntos)) : null,
                fecha: new Date(Date.now() - 5 * 3600000).toISOString().slice(0, 23),
            };
            const items = this.elegidos.map((e) => {
                const file = chatSubidas.get('elegido:' + e.id);
                chatSubidas.delete('elegido:' + e.id);
                return { file, archivo: null, cargado: 0 };
            });
            // Lo que se ve mientras se sube: la vista previa del celular, sin links.
            const adjuntos = this.elegidos.map((e, n) => {
                const url = e.vista || (e.clase === 'video' || e.clase === 'audio' ? URL.createObjectURL(items[n].file) : null);
                return { nombre: e.nombre, clase: e.clase, bytes: e.bytes, original: url, mini: null, local: true };
            });
            chatSubidas.set(p.id, { p, items });
            const m = this.local(p, 'enviando');
            m.adjuntos = adjuntos;
            m.subiendo = { hechos: 0, total: items.length, pct: 0 };
            this.mensajes.push(m);
            this.elegidos = [];
            this.texto = '';
            this.respondiendo = null;
            this.$nextTick(() => {
                this.crecer(this.$refs.caja);
                this.alFinal();
            });
            await this.subirTodo(p.id);
        },

        // Sube lo que falte, 3 a la vez; cada uno se reintenta solo. Con todos arriba, manda el mensaje.
        async subirTodo(id) {
            const s = chatSubidas.get(id);
            if (!s || s.activo) return;
            s.activo = true;
            const m = () => this.mensajes.find((x) => x.id === id);
            const total = s.items.reduce((t, it) => t + it.file.size, 0) || 1;
            const avance = () => {
                const msj = m();
                if (!msj) return;
                const subido = s.items.reduce((t, it) => t + (it.archivo ? it.file.size : it.cargado), 0);
                msj.subiendo = {
                    hechos: s.items.filter((it) => it.archivo).length,
                    total: s.items.length,
                    pct: Math.min(100, Math.round((subido * 100) / total)),
                };
            };
            const faltan = s.items.filter((it) => !it.archivo);
            let siguiente = 0;
            let falla = null;
            const trabajar = async () => {
                while (siguiente < faltan.length && !falla && chatSubidas.has(id)) {
                    const it = faltan[siguiente++];
                    for (let intento = 1; ; intento++) {
                        try {
                            const r = await chatSubir(it.file, id, (n) => { it.cargado = n; avance(); });
                            it.archivo = r.archivo;
                            avance();
                            break;
                        } catch (e) {
                            it.cargado = 0;
                            avance();
                            // 400 y 413: el archivo no se va a poder (vacío, muy grande, bloqueado).
                            if (e.status === 400 || e.status === 413 || intento >= CHAT_INTENTOS) {
                                falla = e;
                                break;
                            }
                            await chatEsperar(1500 * intento);
                        }
                    }
                }
            };
            avance();
            await Promise.all(Array.from({ length: CHAT_SUBIDAS_A_LA_VEZ }, trabajar));
            s.activo = false;
            const msj = m();
            if (!msj || !chatSubidas.has(id)) return;  // lo quitaron mientras subía
            if (falla) {
                msj.estado = 'error';
                msj.subiendo = null;
                this.$nextTick(() => this.alFinal());
                this.error = falla.error || (navigator.onLine
                    ? 'No se pudo subir un archivo.'
                    : 'Sin conexión: no se subieron los archivos.');
                return;
            }
            // Todos arriba: ya se puede guardar como pendiente (sin los File) y mandar.
            const p = {
                ...s.p,
                archivos: s.items.map((it) => it.archivo),
                adjuntos: msj.adjuntos.map((a) => ({ nombre: a.nombre, clase: a.clase, bytes: a.bytes })),
            };
            chatGuardarPendientes([...chatLeerPendientes().filter((x) => x.id !== id), p]);
            msj.subiendo = null;
            await this.mandarUno(p);
        },

        reintentarSubida(m) {
            this.error = '';
            m.estado = 'enviando';
            this.subirTodo(m.id);
        },

        // ── Notas de voz ──

        // 🎤 en lugar de ➤ cuando no hay nada que mandar.
        get microfono() {
            return this.puedeGrabar && !this.grabando && !this.editando && !this.texto.trim() && !this.elegidos.length;
        },

        tiempoNota() {
            const s = this.segundos;
            return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
        },

        async grabar() {
            this.error = '';
            let flujo;
            try {
                flujo = await navigator.mediaDevices.getUserMedia({ audio: true });
            } catch (e) {
                this.error = e.name === 'NotAllowedError'
                    ? 'Para grabar, permite el micrófono en tu celular.'
                    : 'No se pudo usar el micrófono.';
                return;
            }
            const tipo = ['audio/webm;codecs=opus', 'audio/mp4', 'audio/webm'].find((t) => MediaRecorder.isTypeSupported(t));
            const g = new MediaRecorder(flujo, tipo ? { mimeType: tipo } : undefined);
            chatPedazos = [];
            g.ondataavailable = (e) => { if (e.data.size) chatPedazos.push(e.data); };
            g.onstop = () => {
                flujo.getTracks().forEach((t) => t.stop());
                clearInterval(this._reloj);
                this.grabando = false;
                if (!this._cancelada) this.mandarNota(g.mimeType || tipo || '');
                chatPedazos = [];
            };
            this._cancelada = false;
            this.segundos = 0;
            this.grabando = true;
            g.start(1000);
            chatGrabadora = g;
            const inicio = Date.now();
            this._reloj = setInterval(() => {
                this.segundos = Math.floor((Date.now() - inicio) / 1000);
                if (this.segundos >= CHAT_NOTA_MAX_S) this.terminarNota();  // a los 5 min se manda sola
            }, 250);
        },

        terminarNota(cancelar = false) {
            this._cancelada = cancelar;
            if (chatGrabadora && chatGrabadora.state !== 'inactive') chatGrabadora.stop();
            chatGrabadora = null;
        },

        mandarNota(mime) {
            const m4a = mime.includes('mp4');
            const blob = new Blob(chatPedazos, { type: m4a ? 'audio/mp4' : 'audio/webm' });
            if (this.segundos < 1 || !blob.size) return;  // un toque sin querer
            const file = new File([blob], m4a ? 'nota-de-voz.m4a' : 'nota-de-voz.weba', { type: blob.type });
            const id = chatId();
            chatSubidas.set('elegido:' + id, file);
            this.elegidos.push({ id, nombre: 'Nota de voz', clase: 'audio', bytes: file.size, vista: null });
            this.enviarConArchivos('');
        },

        // ── Menú de un mensaje ──

        tocar(m) {
            if (m.eliminado || this.aviso(m) || m.estado !== 'guardado') return;
            this.abierto = this.abierto === m.id ? null : m.id;
        },

        responder(m) {
            this.abierto = null;
            this.editando = null;
            this.respondiendo = m;
            this.$refs.caja.focus();
        },

        puedeEditar(m) {
            if (!this.mio(m) || m.eliminado || !m.texto || this.bloqueado) return false;
            return Date.now() - chatFecha(m.fecha).getTime() < this.minutosEditar * 60000;
        },

        editar(m) {
            this.abierto = null;
            this.respondiendo = null;
            this.editando = m;
            this.texto = m.texto;
            this.$nextTick(() => {
                this.crecer(this.$refs.caja);
                this.$refs.caja.focus();
            });
        },

        cancelar() {
            if (this.editando) this.texto = '';
            this.editando = null;
            this.respondiendo = null;
            this.$nextTick(() => this.crecer(this.$refs.caja));
        },

        async guardarEdicion(texto) {
            const m = this.editando;
            if (texto === m.texto) return this.cancelar();
            try {
                await postJson(`/chat/${m.id}/editar`, { texto });
                // El cambio llega por el SSE (evento `editado`).
                this.editando = null;
                this.texto = '';
                this.$nextTick(() => this.crecer(this.$refs.caja));
            } catch (e) {
                this.error = navigator.onLine ? e.message : 'Sin conexión: no se pudo editar.';
            }
        },

        async eliminar(m) {
            this.abierto = null;
            if (!confirm('¿Eliminar este mensaje? Se borra para ti y para la papelería.')) return;
            try {
                await postJson(`/chat/${m.id}/eliminar`, {});
            } catch (e) {
                this.error = navigator.onLine ? e.message : 'Sin conexión: no se pudo eliminar.';
            }
        },

        // 🔊 con la voz en español del propio celular.
        leer(m) {
            this.abierto = null;
            const texto = m.texto || chatQueEs(m.adjuntos) || chatQueTarjeta(m.tarjeta);
            if (!this.puedeLeer || !texto) return;
            speechSynthesis.cancel();
            const dicho = new SpeechSynthesisUtterance(texto);
            dicho.lang = 'es-MX';
            const voz = speechSynthesis.getVoices().find((v) => v.lang && v.lang.startsWith('es'));
            if (voz) dicho.voice = voz;
            speechSynthesis.speak(dicho);
        },
    }));
});
