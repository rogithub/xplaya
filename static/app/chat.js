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
            };
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
            if (citado) return citado.eliminado ? 'Mensaje eliminado' : citado.texto;
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
                if (m) { m.texto = null; m.eliminado = true; }
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
            const texto = this.texto;
            if (!texto.trim() || this.bloqueado) return;
            this.error = '';
            if (this.editando) return this.guardarEdicion(texto);

            const p = {
                id: chatId(),
                texto,
                respondeA: this.respondiendo ? this.respondiendo.id : null,
                cita: this.respondiendo ? this.respondiendo.texto : null,
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
                    body: JSON.stringify({ id: p.id, texto: p.texto, respondeA: p.respondeA }),
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
            this.mensajes = this.mensajes.filter((x) => x.id !== m.id);
            this.quitarPendiente(m.id);
            this.error = '';
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
            if (!this.puedeLeer || !m.texto) return;
            speechSynthesis.cancel();
            const dicho = new SpeechSynthesisUtterance(m.texto);
            dicho.lang = 'es-MX';
            const voz = speechSynthesis.getVoices().find((v) => v.lang && v.lang.startsWith('es'));
            if (voz) dicho.voice = voz;
            speechSynthesis.speak(dicho);
        },
    }));
});
