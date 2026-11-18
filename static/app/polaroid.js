// Fotos polaroid desde la app (fotos_polaroid.html, routes/fotos.rs). Se carga solo en esa
// pantalla. Es el kiosko polaroid de imagina pasado a Alpine (imagina/frontend/src/routes/kiosko/
// polaroid y $lib/kiosko/*): mismos formatos, mismo acomodo del texto y el mismo pedido, para que
// la operadora lo pueda abrir y corregir en el panel de imagina como siempre:
//   1. Sube a imagina los recortes (la tira: sus 3 fotos ya juntas) y el texto de cada polaroid
//      como PNG (`/api/v1/images`).
//   2. Crea el pedido `polaroid-grid` (`/api/v1/orders/print`) con el permiso de xplaya
//      (X-Permiso): imagina arma el PDF y lo liga al cliente.
//   3. Sube el paquete para corregir en tienda: originales, recortes y `proyecto.json`.
// El avance se guarda en el celular (IndexedDB) por si el navegador cierra la página mientras la
// cámara está abierta.

(() => {
    // ── Formatos (imagina $lib/kiosko/presets.ts) ────────────────────────────
    const FORMATOS = {
        strip:   { nombre: 'Tira',     w: 50,  h: 150, desc: '5 × 15 cm' },
        large:   { nombre: 'Grande',   w: 100, h: 150, desc: '10 × 15 cm' },
        classic: { nombre: 'Clásica',  w: 100, h: 90,  desc: '10 × 9 cm' },
        mini:    { nombre: 'Mini',     w: 50,  h: 70,  desc: '5 × 7 cm' },
    };
    // Marco blanco (mm); abajo, el espacio para escribir.
    const MARCO = {
        strip:   { top: 4, right: 4, bottom: 18, left: 4 },
        large:   { top: 5, right: 5, bottom: 5,  left: 5 },
        classic: { top: 6, right: 5, bottom: 19, left: 5 },
        mini:    { top: 5, right: 4, bottom: 14, left: 4 },
    };
    const CON_TEXTO = { strip: true, large: false, classic: true, mini: true };
    const MAX_LETRAS = { strip: 60, large: 0, classic: 60, mini: 40 };
    const POR_TIRA = 3;
    const SEPARACION = 4; // mm entre polaroids en la hoja y entre las fotos de la tira
    const CARTA = { w: 215.9, h: 279.4 };
    const DPI = 300;
    const PX_MM = DPI / 25.4;
    const px = (mm) => Math.round((mm / 25.4) * DPI);

    // Letras con TTF en imagina (`CaptionFont` en imagina/src/api/pdf/text.rs).
    const LETRAS = ['Caveat', 'Pacifico', 'Montserrat', 'Dancing Script', 'Great Vibes', 'Indie Flower',
        'Shadows Into Light', 'Kalam', 'Permanent Marker', 'Amatic SC', 'Roboto', 'Oswald',
        'Playfair Display', 'Merriweather', 'Lobster'];
    const COLORES = ['#0f172a', '#1d4ed8', '#0f766e', '#15803d', '#dc2626', '#9333ea', '#db2777', '#ea580c'];
    const TEXTO_INICIAL = { texto: '', letra: 'Caveat', color: '#0f172a' };

    function rejilla(f) {
        const p = FORMATOS[f];
        return {
            cols: Math.max(1, Math.floor((CARTA.w + SEPARACION) / (p.w + SEPARACION))),
            rows: Math.max(1, Math.floor((CARTA.h + SEPARACION) / (p.h + SEPARACION))),
        };
    }
    /** Fotos que caben en una hoja carta. */
    function fotosPorHoja(f) {
        const { cols, rows } = rejilla(f);
        return f === 'strip' ? cols * rows * POR_TIRA : cols * rows;
    }
    /** Área de una foto dentro del marco (en la tira, una de sus 3). */
    function areaFoto(f) {
        const p = FORMATOS[f], m = MARCO[f];
        const w = p.w - m.left - m.right;
        let h = p.h - m.top - m.bottom;
        if (f === 'strip') h = (h - SEPARACION * (POR_TIRA - 1)) / POR_TIRA;
        return { aspecto: w / h, w: px(w), h: px(h) };
    }
    /** Franja blanca de abajo, donde va el texto. */
    function franja(f) {
        const p = FORMATOS[f], m = MARCO[f];
        return { w: p.w - m.left - m.right, h: m.bottom };
    }

    // ── Texto (imagina $lib/kiosko/caption.ts): el mismo acomodo en la vista y en la hoja ──
    const INTERLINEA = 1.2, MARGEN_X = 1.2, MARGEN_Y = 1, MAX_RATIO = 0.5, MIN_MM = 2.5, GANANCIA_PARTIR = 1.6;
    const EMOJI = '"Apple Color Emoji", "Noto Color Emoji", "Segoe UI Emoji", sans-serif';
    const fuente = (letra, pxs) => `${pxs}px "${letra}", ${EMOJI}`;
    const listas = new Map();
    function asegurarLetra(letra) {
        if (!listas.has(letra)) listas.set(letra, document.fonts.load(fuente(letra, 40)).catch(() => {}));
        return listas.get(letra);
    }
    let lienzoMedir = null;
    function medidor(letra, mm) {
        lienzoMedir ??= document.createElement('canvas').getContext('2d');
        lienzoMedir.font = fuente(letra, mm * PX_MM);
        return (s) => lienzoMedir.measureText(s).width / PX_MM;
    }
    const segmentador = 'Segmenter' in Intl ? new Intl.Segmenter('es', { granularity: 'grapheme' }) : null;
    const grafemas = (s) => (segmentador ? Array.from(segmentador.segment(s), (g) => g.segment) : Array.from(s));
    function envolver(palabras, ancho, max, partir) {
        const lineas = [];
        let cur = '';
        for (const p of palabras) {
            const prueba = cur ? `${cur} ${p}` : p;
            if (ancho(prueba) <= max) { cur = prueba; continue; }
            if (cur) lineas.push(cur);
            cur = '';
            if (ancho(p) <= max) { cur = p; continue; }
            if (!partir) return null;
            for (const g of grafemas(p)) {
                if (cur && ancho(cur + g) > max) { lineas.push(cur); cur = g; } else cur += g;
            }
        }
        if (cur) lineas.push(cur);
        return lineas;
    }
    /** El tamaño más grande al que el texto cabe en la franja; `cabe` false si ni al mínimo. */
    function acomodarTexto(texto, letra, caja) {
        const palabras = texto.trim().split(/\s+/).filter(Boolean);
        if (!palabras.length) return { mm: MIN_MM, lineas: [], cabe: true };
        const anchoUtil = caja.w - 2 * MARGEN_X, altoUtil = caja.h - 2 * MARGEN_Y, max = caja.h * MAX_RATIO;
        const probar = (t, partir) => {
            const l = envolver(palabras, medidor(letra, t), anchoUtil, partir);
            return l && l.length * t * INTERLINEA <= altoUtil ? l : null;
        };
        const mayor = (partir) => {
            if (!probar(MIN_MM, partir)) return null;
            if (probar(max, partir)) return max;
            let lo = MIN_MM, hi = max;
            for (let i = 0; i < 14; i++) { const m = (lo + hi) / 2; if (probar(m, partir)) lo = m; else hi = m; }
            return lo;
        };
        const entero = mayor(false), partido = mayor(true);
        const partir = entero === null || (partido !== null && partido > entero * GANANCIA_PARTIR);
        const t = partir ? partido : entero;
        if (t === null) return { mm: MIN_MM, lineas: envolver(palabras, medidor(letra, MIN_MM), anchoUtil, true), cabe: false };
        // Líneas parejas: el ancho más angosto que conserva el número de líneas.
        let lineas = probar(t, partir);
        if (lineas.length > 1) {
            const ancho = medidor(letra, t);
            let lo = 0, hi = anchoUtil;
            for (let i = 0; i < 12; i++) {
                const m = (lo + hi) / 2, a = envolver(palabras, ancho, m, partir);
                if (a && a.length <= lineas.length) { lineas = a; hi = m; } else lo = m;
            }
        }
        return { mm: t, lineas, cabe: true };
    }
    function dibujarTexto(ctx, acomodo, letra, color, caja, pxPorMm) {
        if (!acomodo.lineas.length) return;
        const t = acomodo.mm * pxPorMm, alto = t * INTERLINEA;
        ctx.save();
        ctx.beginPath();
        ctx.rect(0, 0, caja.w * pxPorMm, caja.h * pxPorMm);
        ctx.clip();
        ctx.font = fuente(letra, t);
        ctx.fillStyle = color;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        const cx = (caja.w * pxPorMm) / 2;
        const y0 = (caja.h * pxPorMm) / 2 - ((acomodo.lineas.length - 1) * alto) / 2;
        acomodo.lineas.forEach((l, i) => ctx.fillText(l, cx, y0 + i * alto));
        ctx.restore();
    }
    async function textoPng(texto, letra, color, caja) {
        await asegurarLetra(letra);
        const c = document.createElement('canvas');
        c.width = Math.round(caja.w * PX_MM);
        c.height = Math.round(caja.h * PX_MM);
        const ctx = c.getContext('2d');
        ctx.fillStyle = '#ffffff';
        ctx.fillRect(0, 0, c.width, c.height);
        dibujarTexto(ctx, acomodarTexto(texto, letra, caja), letra, color, caja, PX_MM);
        return new Promise((ok, mal) => c.toBlob((b) => (b ? ok(b) : mal(new Error('PNG'))), 'image/png'));
    }

    // ── Recortes ─────────────────────────────────────────────────────────────
    function abrirImagen(blob) {
        const url = URL.createObjectURL(blob);
        const im = new Image();
        im.src = url;
        return im.decode().then(() => ({ im, url }), (e) => { URL.revokeObjectURL(url); throw e; });
    }
    /** Encuadre al centro con la proporción pedida (fracciones de la foto, como en imagina). */
    function cajaInicial(nw, nh, aspecto) {
        const r = aspecto * (nh / nw);
        let w = 1, h = 1;
        if (r >= 1) h = 1 / r; else w = r;
        return { x: (1 - w) / 2, y: (1 - h) / 2, w, h };
    }
    function hornear(im, caja, w, h) {
        const c = document.createElement('canvas');
        c.width = w;
        c.height = h;
        const ctx = c.getContext('2d');
        ctx.imageSmoothingQuality = 'high';
        const nw = im.naturalWidth, nh = im.naturalHeight;
        ctx.drawImage(im, caja.x * nw, caja.y * nh, Math.max(1, caja.w * nw), Math.max(1, caja.h * nh), 0, 0, w, h);
        return new Promise((ok, mal) => c.toBlob((b) => (b ? ok(b) : mal(new Error('JPEG'))), 'image/jpeg', 0.92));
    }
    async function recorteAutomatico(archivo, area) {
        const { im, url } = await abrirImagen(archivo);
        try {
            const caja = cajaInicial(im.naturalWidth, im.naturalHeight, area.aspecto);
            return { recorte: await hornear(im, caja, area.w, area.h), caja };
        } finally {
            URL.revokeObjectURL(url);
        }
    }
    /** Cuántas fotos preparar a la vez: decodificar una de 12 MP pesa ~48 MB. */
    function aLaVez() {
        const mem = navigator.deviceMemory, nucleos = navigator.hardwareConcurrency || 2;
        const porMem = mem === undefined ? 2 : mem <= 2 ? 1 : mem <= 4 ? 2 : 3;
        return Math.max(1, Math.min(porMem, nucleos - 1));
    }
    async function enParalelo(cosas, limite, fn) {
        const res = new Array(cosas.length);
        let sig = 0;
        const trabajador = async () => {
            while (sig < cosas.length) {
                const i = sig++;
                try { res[i] = { ok: true, valor: await fn(cosas[i], i) }; } catch (error) { res[i] = { ok: false, error }; }
            }
        };
        await Promise.all(Array.from({ length: Math.max(1, Math.min(limite, cosas.length)) }, trabajador));
        return res;
    }
    /** Las 3 fotos de una tira, una sobre otra con su separación blanca. */
    async function juntarTira(recortes, w, h, sep) {
        const c = document.createElement('canvas');
        c.width = w;
        c.height = recortes.length * h + (recortes.length - 1) * sep;
        const ctx = c.getContext('2d');
        ctx.fillStyle = '#ffffff';
        ctx.fillRect(0, 0, c.width, c.height);
        for (let i = 0; i < recortes.length; i++) {
            if (!recortes[i]) continue;
            const { im, url } = await abrirImagen(recortes[i]);
            ctx.drawImage(im, 0, i * (h + sep), w, h);
            URL.revokeObjectURL(url);
        }
        return new Promise((ok) => c.toBlob(ok, 'image/jpeg', 0.95));
    }

    // ── Borrador en el celular (imagina $lib/kiosko/draft.ts) ────────────────
    const BORRADOR_HORAS = 48;
    let bd = null;
    function abrirBd() {
        bd ??= new Promise((ok, mal) => {
            const r = indexedDB.open('xplaya-fotos', 1);
            r.onupgradeneeded = () => r.result.createObjectStore('borradores');
            r.onsuccess = () => ok(r.result);
            r.onerror = () => mal(r.error);
        });
        return bd;
    }
    async function enBd(modo, op) {
        const db = await abrirBd();
        return new Promise((ok, mal) => {
            const tx = db.transaction('borradores', modo);
            const r = op(tx.objectStore('borradores'));
            tx.oncomplete = () => ok(r.result);
            tx.onerror = tx.onabort = () => mal(tx.error);
        });
    }
    const borrador = {
        leer: (k) => enBd('readonly', (s) => s.get(['polaroid', k])).catch(() => undefined),
        guardar: (k, v) => enBd('readwrite', (s) => s.put(v, ['polaroid', k])).catch(() => {}),
        quitar: (k) => enBd('readwrite', (s) => s.delete(['polaroid', k])).catch(() => {}),
        vaciar: () => enBd('readwrite', (s) => s.delete(IDBKeyRange.bound(['polaroid'], ['polaroid', []]))).catch(() => {}),
    };

    // ── Mandar a imagina ─────────────────────────────────────────────────────
    async function permiso() {
        const r = await fetch('/fotos/permiso', { cache: 'no-store' });
        if (r.status === 401) throw new Error('Tu sesión se cerró. Vuelve a entrar a la app.');
        if (!r.ok) throw new Error('No pudimos preparar el envío. Intenta de nuevo.');
        return r.json();
    }
    async function respuesta(r, si) {
        const cuerpo = await r.json().catch(() => ({}));
        if (!r.ok) {
            const e = new Error(cuerpo.error || si);
            e.estado = r.status;
            throw e;
        }
        return cuerpo;
    }
    async function subirImagenes(imagina, blobs) {
        const datos = new FormData();
        blobs.forEach((b, i) => datos.append('files', b, `foto-${i}.${b.type === 'image/png' ? 'png' : 'jpg'}`));
        const r = await fetch(`${imagina}/api/v1/images`, { method: 'POST', body: datos });
        return (await respuesta(r, 'No pudimos subir tus fotos.')).images.map((i) => i.id);
    }
    /** El diseño `polaroid-grid` que imagina convierte en PDF (imagina checkout.ts). */
    async function diseno(imagina, f, fotos) {
        const p = FORMATOS[f], m = MARCO[f], { rows, cols } = rejilla(f), caja = franja(f);
        let imagenes, textos;
        if (f === 'strip') {
            const area = areaFoto(f), tiras = [];
            textos = [];
            for (let s = 0; s < fotos.length / POR_TIRA; s++) {
                const de = fotos.slice(s * POR_TIRA, (s + 1) * POR_TIRA);
                if (!de.some(Boolean)) continue;
                tiras.push(await juntarTira(de.map((x) => x?.recorte ?? null), area.w, area.h, px(SEPARACION)));
                textos.push(de.find(Boolean));
            }
            imagenes = await subirImagenes(imagina, tiras);
        } else {
            const llenas = fotos.filter(Boolean);
            imagenes = await subirImagenes(imagina, llenas.map((x) => x.recorte));
            textos = llenas;
        }
        // El texto viaja como PNG de la franja, dibujado igual que en la vista previa.
        const conTexto = textos.map((t, i) => (CON_TEXTO[f] && t.texto.trim() ? i : -1)).filter((i) => i >= 0);
        const pngs = await Promise.all(conTexto.map((i) => textoPng(textos[i].texto, textos[i].letra, textos[i].color, caja)));
        const idsTexto = new Map();
        if (pngs.length) (await subirImagenes(imagina, pngs)).forEach((id, n) => idsTexto.set(conTexto[n], id));
        const images = imagenes.map((id, i) => {
            const t = textos[i], idTexto = idsTexto.get(i);
            if (!idTexto) return { id, rotation: 0, align: 'top' };
            return {
                id, rotation: 0, align: 'top',
                caption: {
                    text: t.texto, fontFamily: t.letra,
                    fontSize: acomodarTexto(t.texto, t.letra, caja).mm * 2.835, // pt; informativo, el PNG manda
                    color: t.color, rotation: 0, imageId: idTexto, imageWidthMM: caja.w, imageHeightMM: caja.h,
                },
            };
        });
        return {
            images,
            page: { size: 'letter', orientation: 'portrait', marginMM: 0 },
            grid: { rows, cols, spacingMM: SEPARACION, cellWidthMM: p.w, cellHeightMM: p.h },
            padding: m,
            cropMarkLengthMM: 4,
        };
    }
    const EXT = { 'image/jpeg': 'jpg', 'image/png': 'png', 'image/webp': 'webp', 'image/gif': 'gif', 'image/heic': 'heic', 'image/heif': 'heif' };
    function extension(a) {
        if (EXT[a.type]) return EXT[a.type];
        const e = (a.name || '').split('.').pop().toLowerCase();
        return Object.values(EXT).includes(e) || e === 'jpeg' ? e : null;
    }
    /** Paquete para corregir en tienda (imagina project.ts): fotos y al final `proyecto.json`. */
    function archivosPaquete(f, fotos) {
        const archivos = [];
        const json = {
            version: 1, tool: 'kiosko-polaroid', preset: f, savedAt: new Date().toISOString(),
            photos: fotos.map((x, slot) => {
                if (!x) return null;
                const ext = extension(x.original);
                const original = ext ? `originales/${slot}.${ext}` : null;
                const recorte = `recortes/${slot}.jpg`;
                if (original) archivos.push({ ruta: original, blob: x.original });
                archivos.push({ ruta: recorte, blob: x.recorte });
                return { slot, original, originalName: x.original.name || `foto-${slot}.${ext || 'jpg'}`, recorte,
                    box: x.caja, caption: x.texto, font: x.letra, color: x.color };
            }),
        };
        archivos.push({ ruta: 'proyecto.json', blob: new Blob([JSON.stringify(json, null, 2)], { type: 'application/json' }) });
        return archivos;
    }
    function subirConAvance(url, blob, alSubir) {
        return new Promise((ok, mal) => {
            const x = new XMLHttpRequest();
            x.open('PUT', url);
            x.setRequestHeader('Content-Type', blob.type || 'application/octet-stream');
            x.upload.onprogress = (e) => alSubir(e.loaded);
            x.onload = () => (x.status >= 200 && x.status < 300 ? (alSubir(blob.size), ok()) : mal(new Error(`subida ${x.status}`)));
            x.onerror = () => mal(new Error('subida'));
            x.send(blob);
        });
    }
    async function subirPaquete(imagina, codigo, token, f, fotos, avance) {
        const archivos = archivosPaquete(f, fotos);
        const fotosPaq = archivos.filter((a) => a.ruta !== 'proyecto.json');
        const json = archivos.find((a) => a.ruta === 'proyecto.json');
        const total = archivos.reduce((s, a) => s + a.blob.size, 0) || 1;
        const subido = new Map();
        const reportar = (a) => (bytes) => {
            subido.set(a, Math.min(bytes, a.blob.size));
            let s = 0;
            for (const b of subido.values()) s += b;
            avance(s / total);
        };
        const url = (a) => `${imagina}/api/v1/orders/${codigo}/files/${a.ruta}?token=${encodeURIComponent(token)}`;
        const res = await enParalelo(fotosPaq, 4, (a) => subirConAvance(url(a), a.blob, reportar(a)));
        if (res.some((r) => !r.ok)) throw new Error('paquete incompleto');
        await subirConAvance(url(json), json.blob, reportar(json));
    }

    // ── La pantalla ──────────────────────────────────────────────────────────
    document.addEventListener('alpine:init', () => {
        Alpine.data('fotoPolaroid', () => ({
            FORMATOS, LETRAS, COLORES, POR_TIRA,
            paso: 'formato',        // formato → fotos → editar → encuadre · enviando
            formato: 'classic',
            elegido: 'classic',     // resaltado en "formato"; se aplica al continuar
            fotos: [],
            idx: 0,                 // foto que se edita
            listo: false,
            preparando: null,       // { hechas, total }
            aviso: '',
            avance: 0,
            error: '',
            versionLetra: 0,        // sube cuando carga una letra: se vuelve a medir el texto

            // Encuadre de la foto `idx`
            enc: { url: '', img: { w: 0, h: 0 }, marco: { w: 0, h: 0 }, zoom: 1, zoomPrevio: 1, pos: { x: 0, y: 0 } },
            arrastre: null,

            async init() {
                window.addEventListener('popstate', (e) => this.alVolver(e.state));
                LETRAS.forEach((l) => asegurarLetra(l).then(() => this.versionLetra++));
                await this.recuperar();
                history.replaceState({ paso: 'formato' }, '');
                if (this.paso !== 'formato') history.pushState({ paso: 'fotos' }, '');
                if (this.paso === 'editar') history.pushState({ paso: 'editar', idx: this.idx }, '');
                this.listo = true;
            },

            // ── Navegación: cada pantalla va al historial, así "atrás" del celular funciona ──
            ir(paso, idx = this.idx) {
                this.paso = paso;
                this.idx = idx;
                history.pushState({ paso, idx }, '');
                this.guardarMeta();
            },
            cambiarA(idx) {
                this.idx = idx;
                history.replaceState({ paso: this.paso, idx }, '');
                this.guardarMeta();
            },
            atras() { history.back(); },
            alVolver(estado) {
                if (this.paso === 'enviando') return;
                const paso = estado?.paso || 'formato';
                if ((paso === 'editar' || paso === 'encuadre') && !this.fotos[estado.idx]) { this.paso = 'fotos'; return; }
                if (paso === 'formato') this.elegido = this.formato;
                if (this.paso === 'encuadre') this.soltarEncuadre();
                this.paso = paso;
                if (estado?.idx !== undefined) this.idx = estado.idx;
                if (paso === 'encuadre') this.abrirEncuadre();
                this.guardarMeta();
            },

            // ── Borrador ──
            guardarMeta() {
                if (!this.listo) return;
                borrador.guardar('meta', {
                    guardado: Date.now(), formato: this.formato, paso: this.paso === 'encuadre' ? 'editar' : this.paso, idx: this.idx,
                    textos: this.fotos.map((x) => (x ? { texto: x.texto, letra: x.letra, color: x.color, caja: { ...x.caja } } : null)),
                });
            },
            async recuperar() {
                const meta = await borrador.leer('meta');
                if (!meta || Date.now() - meta.guardado > BORRADOR_HORAS * 3600e3 || !FORMATOS[meta.formato]) {
                    await borrador.vaciar();
                    this.fotos = Array(fotosPorHoja(this.formato)).fill(null);
                    return;
                }
                const fotos = Array(fotosPorHoja(meta.formato)).fill(null);
                for (let i = 0; i < fotos.length; i++) {
                    const r = await borrador.leer(`foto:${i}`);
                    if (!r) continue;
                    fotos[i] = { original: r.original, recorte: r.recorte, url: URL.createObjectURL(r.recorte),
                        ...TEXTO_INICIAL, caja: null, ...meta.textos[i] };
                }
                this.formato = this.elegido = meta.formato;
                this.fotos = fotos;
                this.paso = meta.paso === 'editar' && fotos[meta.idx] ? 'editar' : fotos.some(Boolean) ? 'fotos' : 'formato';
                this.idx = meta.idx || 0;
            },

            // ── Derivados ──
            get esTira() { return this.formato === 'strip'; },
            get aspectoArea() { return areaFoto(this.formato).aspecto; },
            get hayTexto() { return CON_TEXTO[this.formato]; },
            get maxLetras() { return MAX_LETRAS[this.formato]; },
            get llenas() { return this.fotos.filter(Boolean).length; },
            get hayHuecos() { return this.llenas < this.fotos.length; },
            /** Dibujito del formato a escala, con su marco. */
            muestra(f) {
                const p = FORMATOS[f], m = MARCO[f], e = 0.55;
                return `width:${p.w * e}px;height:${p.h * e}px;padding:${m.top * e}px ${m.right * e}px ${m.bottom * e}px ${m.left * e}px`;
            },
            desc(f) { return `${FORMATOS[f].desc} · hasta ${fotosPorHoja(f)} fotos${f === 'strip' ? ` (${fotosPorHoja(f) / POR_TIRA} tiras de 3)` : ''}`; },
            inicioUnidad(i) { return this.esTira ? Math.floor(i / POR_TIRA) * POR_TIRA : i; },
            unidad(i) { const s = this.inicioUnidad(i); return this.esTira ? [s, s + 1, s + 2] : [s]; },
            /** Cada polaroid (o tira) como lista de sus huecos. */
            get unidades() {
                const u = [];
                for (let s = 0; s < this.fotos.length; s += this.esTira ? POR_TIRA : 1) u.push(this.unidad(s));
                return u;
            },
            /** Las polaroids con fotos, por su primera foto. */
            get conFotos() { return this.unidades.map((u) => u.find((i) => this.fotos[i])).filter((i) => i !== undefined); },
            get posicion() { return this.conFotos.findIndex((i) => this.inicioUnidad(i) === this.inicioUnidad(this.idx)); },
            primera(u) { return u.map((i) => this.fotos[i]).find(Boolean) || null; },
            get activa() { return this.fotos[this.idx]; },
            /** El texto de la polaroid que se edita (en la tira, el de sus 3 fotos). */
            get fuente() { return this.primera(this.unidad(this.idx)) || TEXTO_INICIAL; },
            get cabe() {
                this.versionLetra;
                if (!this.hayTexto || !this.activa) return true;
                return acomodarTexto(this.fuente.texto, this.fuente.letra, franja(this.formato)).cabe;
            },

            // ── Vista de una polaroid: medidas en px para un ancho máximo ──
            /** En "Tus fotos": 2 por renglón (3 en tira y mini). */
            tarjetaFotos() {
                const cols = this.formato === 'strip' || this.formato === 'mini' ? 3 : 2;
                const ancho = Math.min(window.innerWidth, 560) - 32;
                return this.tarjeta((ancho - 12 * (cols - 1)) / cols, 9999);
            },
            get columnas() { return this.formato === 'strip' || this.formato === 'mini' ? 3 : 2; },
            tarjetaEditor() {
                return this.tarjeta(Math.max(140, Math.min(window.innerWidth - 120, 320)), this.esTira ? 380 : 300);
            },
            tarjeta(maxW, maxH) {
                const p = FORMATOS[this.formato], m = MARCO[this.formato];
                const e = Math.min(maxW / p.w, maxH / p.h);
                const foto = `left:${m.left * e}px;top:${m.top * e}px;width:${(p.w - m.left - m.right) * e}px;height:${(p.h - m.top - m.bottom) * e}px;gap:${SEPARACION * e}px`;
                return { e, caja: `width:${p.w * e}px;height:${p.h * e}px`, foto,
                    texto: `left:${m.left * e}px;top:${(p.h - m.bottom) * e}px;width:${(p.w - m.left - m.right) * e}px;height:${m.bottom * e}px` };
            },
            pintarTexto(lienzo, fuenteTexto, e) {
                this.versionLetra;
                if (!lienzo || !this.hayTexto) return;
                const caja = franja(this.formato), dpr = window.devicePixelRatio || 1;
                lienzo.width = Math.round(caja.w * e * dpr);
                lienzo.height = Math.round(caja.h * e * dpr);
                const ctx = lienzo.getContext('2d');
                ctx.clearRect(0, 0, lienzo.width, lienzo.height);
                if (!fuenteTexto) return;
                dibujarTexto(ctx, acomodarTexto(fuenteTexto.texto, fuenteTexto.letra, caja), fuenteTexto.letra, fuenteTexto.color, caja, e * dpr);
            },

            // ── Formato ──
            continuar() {
                if (this.elegido !== this.formato) {
                    this.fotos.forEach((x) => x && URL.revokeObjectURL(x.url));
                    this.formato = this.elegido;
                    this.fotos = Array(fotosPorHoja(this.formato)).fill(null);
                    borrador.vaciar();
                }
                this.ir('fotos');
            },
            get borraAlCambiar() { return this.elegido !== this.formato && this.llenas > 0; },

            // ── Agregar y cambiar fotos ──
            desde: null,
            reemplazar: null,
            abrirGaleria(desde = null) { this.desde = desde; this.$refs.galeria.click(); },
            abrirCamara() { this.desde = null; this.$refs.camara.click(); },
            abrirCambio(i) { this.reemplazar = i; this.$refs.cambio.click(); },
            huecos(desde) {
                const n = this.fotos.length, inicio = desde ?? 0;
                return [...Array(n).keys()].map((k) => (inicio + k) % n).filter((i) => !this.fotos[i]);
            },
            ponerFoto(i, original, recorte, caja) {
                const antes = this.fotos[i];
                if (antes) URL.revokeObjectURL(antes.url);
                // En la tira el texto es de las 3: se hereda de una hermana.
                const t = antes || this.unidad(i).map((k) => this.fotos[k]).find(Boolean) || TEXTO_INICIAL;
                this.fotos[i] = { original, recorte, url: URL.createObjectURL(recorte), caja, texto: t.texto, letra: t.letra, color: t.color };
                borrador.guardar(`foto:${i}`, { original, recorte });
            },
            avisar(msg) {
                this.aviso = msg;
                clearTimeout(this.tAviso);
                this.tAviso = setTimeout(() => (this.aviso = ''), 5000);
            },
            async elegidas(ev, modo) {
                const archivos = Array.from(ev.target.files || []).filter((a) => a.type.startsWith('image/') || !a.type);
                ev.target.value = '';
                if (!archivos.length) return;
                const objetivo = this.reemplazar ?? (this.paso === 'editar' ? this.idx : null);
                const destinos = modo === 'cambiar' && objetivo !== null ? [objetivo] : this.huecos(this.desde);
                this.reemplazar = null;
                const usar = archivos.slice(0, destinos.length);
                if (archivos.length > destinos.length) {
                    this.avisar(destinos.length === 0 ? 'Ya no caben más fotos en este formato.'
                        : `Solo cabían ${destinos.length} ${destinos.length === 1 ? 'foto más' : 'fotos más'}; agregamos esas.`);
                }
                if (!usar.length) return;
                const area = areaFoto(this.formato);
                this.preparando = { hechas: 0, total: usar.length };
                const res = await enParalelo(usar, aLaVez(), async (a) => {
                    const r = await recorteAutomatico(a, area);
                    this.preparando.hechas++;
                    return r;
                });
                let fallaron = 0;
                for (let n = 0; n < usar.length; n++) {
                    let r = res[n].ok ? res[n].valor : null;
                    // Las que fallaron (memoria al abrir varias a la vez) se reintentan una por una.
                    if (!r) { try { r = await recorteAutomatico(usar[n], area); } catch { fallaron++; continue; } }
                    this.ponerFoto(destinos[n], usar[n], r.recorte, r.caja);
                }
                this.preparando = null;
                if (fallaron) this.avisar(`No pudimos abrir ${fallaron === 1 ? 'una foto' : `${fallaron} fotos`}. Prueba con otra.`);
                this.guardarMeta();
                const primeraNueva = destinos.slice(0, usar.length).find((i) => this.fotos[i]);
                if (primeraNueva === undefined) return;
                // Recién agregadas: se abre la primera para ponerle texto.
                if (modo === 'llenar' && this.paso === 'fotos' && this.hayTexto) this.ir('editar', primeraNueva);
                else if (modo === 'cambiar' && this.paso === 'editar') this.cambiarA(primeraNueva);
            },

            // ── Editor ──
            moverUnidad(d) {
                const sig = this.conFotos[this.posicion + d];
                if (sig !== undefined) this.cambiarA(sig);
            },
            ponerTexto(campo, valor) {
                for (const i of this.unidad(this.idx)) {
                    if (this.fotos[i]) this.fotos[i] = { ...this.fotos[i], [campo]: valor };
                }
                if (campo === 'letra') asegurarLetra(valor).then(() => this.versionLetra++);
                this.guardarMeta();
            },
            quitarFoto() {
                const antes = this.fotos[this.idx];
                if (antes) URL.revokeObjectURL(antes.url);
                this.fotos[this.idx] = null;
                borrador.quitar(`foto:${this.idx}`);
                const resto = this.unidad(this.idx).find((i) => this.fotos[i]);
                if (resto !== undefined) this.cambiarA(resto); else this.atras();
                this.guardarMeta();
            },

            // ── Encuadre: arrastrar y acercar; se guarda como `box` (fracciones), igual que imagina ──
            irAEncuadre() {
                this.ir('encuadre');
                this.abrirEncuadre();
            },
            async abrirEncuadre() {
                const f = this.fotos[this.idx];
                if (!f) return;
                this.soltarEncuadre();
                this.enc.url = URL.createObjectURL(f.original);
                const im = new Image();
                im.src = this.enc.url;
                try { await im.decode(); } catch { this.avisar('No pudimos abrir esa foto.'); this.atras(); return; }
                this.enc.img = { w: im.naturalWidth, h: im.naturalHeight };
                await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
                const m = this.$refs.marcoEnc;
                this.enc.marco = { w: m.clientWidth, h: m.clientHeight };
                const base = this.escalaBase();
                const caja = f.caja || cajaInicial(im.naturalWidth, im.naturalHeight, areaFoto(this.formato).aspecto);
                const e = this.enc.marco.w / (caja.w * im.naturalWidth);
                this.enc.zoom = this.enc.zoomPrevio = Math.min(4, Math.max(1, e / base));
                this.enc.pos = this.acotar(-caja.x * im.naturalWidth * this.escalaEnc(), -caja.y * im.naturalHeight * this.escalaEnc());
            },
            soltarEncuadre() {
                if (this.enc.url) URL.revokeObjectURL(this.enc.url);
                this.enc.url = '';
            },
            escalaBase() {
                const { img, marco } = this.enc;
                return img.w && marco.w ? Math.max(marco.w / img.w, marco.h / img.h) : 0;
            },
            escalaEnc() { return this.escalaBase() * this.enc.zoom; },
            acotar(x, y) {
                const e = this.escalaEnc(), { img, marco } = this.enc;
                return { x: Math.min(0, Math.max(marco.w - img.w * e, x)), y: Math.min(0, Math.max(marco.h - img.h * e, y)) };
            },
            tomar(ev) { ev.currentTarget.setPointerCapture(ev.pointerId); this.arrastre = { x: ev.clientX, y: ev.clientY, pos: { ...this.enc.pos } }; },
            mover(ev) {
                if (!this.arrastre) return;
                const a = this.arrastre;
                this.enc.pos = this.acotar(a.pos.x + ev.clientX - a.x, a.pos.y + ev.clientY - a.y);
            },
            soltar() { this.arrastre = null; },
            acercar() {
                const cx = this.enc.marco.w / 2, cy = this.enc.marco.h / 2, f = this.enc.zoom / this.enc.zoomPrevio;
                this.enc.pos = this.acotar(cx - (cx - this.enc.pos.x) * f, cy - (cy - this.enc.pos.y) * f);
                this.enc.zoomPrevio = this.enc.zoom;
            },
            async aplicarEncuadre() {
                const f = this.fotos[this.idx], e = this.escalaEnc(), { img, marco, pos } = this.enc;
                const caja = { x: -pos.x / e / img.w, y: -pos.y / e / img.h, w: marco.w / e / img.w, h: marco.h / e / img.h };
                const { im, url } = await abrirImagen(f.original);
                const area = areaFoto(this.formato);
                const recorte = await hornear(im, caja, area.w, area.h);
                URL.revokeObjectURL(url);
                URL.revokeObjectURL(f.url);
                this.fotos[this.idx] = { ...f, recorte, url: URL.createObjectURL(recorte), caja };
                borrador.guardar(`foto:${this.idx}`, { original: f.original, recorte });
                this.atras();
            },

            // ── Mandar ──
            async mandar() {
                if (!this.llenas) return;
                this.error = '';
                this.avance = 0;
                this.paso = 'enviando';
                const preparar = setInterval(() => { if (this.avance < 9) this.avance++; }, 400);
                try {
                    let pedido, intento = 0, imagina;
                    for (;;) {
                        const p = await permiso();
                        imagina = p.imagina;
                        const r = await fetch(`${imagina}/api/v1/orders/print`, {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json', 'X-Permiso': p.permiso },
                            body: JSON.stringify({ tool: 'polaroid-grid', design: await diseno(imagina, this.formato, this.fotos) }),
                        });
                        try { pedido = await respuesta(r, 'No pudimos mandar tus fotos.'); break; } catch (e) {
                            // Permiso vencido (se tardó mucho): uno nuevo y otra vez, una sola vez.
                            if (e.estado === 400 && /permiso/i.test(e.message) && intento++ === 0) continue;
                            throw e;
                        }
                    }
                    clearInterval(preparar);
                    // El paquete es para corregir en tienda; si falla, el pedido igual ya está.
                    try {
                        await subirPaquete(imagina, pedido.orderCode, pedido.uploadToken, this.formato, this.fotos,
                            (f) => { this.avance = Math.max(this.avance, 10 + Math.round(f * 90)); });
                    } catch (e) { console.warn('paquete incompleto', e); }
                    await borrador.vaciar();
                    location.href = '/fotos?mandado=' + encodeURIComponent(pedido.orderCode);
                } catch (e) {
                    clearInterval(preparar);
                    this.error = e instanceof TypeError ? 'Sin conexión. Revisa tu internet e intenta de nuevo.' : e.message;
                    this.paso = 'fotos';
                    history.replaceState({ paso: 'fotos' }, '');
                }
            },
        }));
    });
})();
