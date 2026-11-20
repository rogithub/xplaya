// Mi perfil (templates/app/perfil.html): acomodar la foto en el círculo antes de mandarla, como
// en WhatsApp. Un dedo la mueve, dos la acercan (o la barrita de abajo); "Listo" la recorta aquí
// mismo en un cuadro de 512 px y manda eso (~60 KB en vez de la foto completa). El servidor la
// revisa y la deja en 256 (`perfil::preparar_foto`).
//
// Si el celular no puede abrir la foto (formato raro), se manda completa como antes y el
// servidor la recorta al centro.

const PERFIL_LADO = 512;
const PERFIL_ZOOM_MAX = 4;   // veces el tamaño mínimo (el que apenas cubre el círculo)

document.addEventListener('alpine:init', () => {
    Alpine.data('recorteFoto', () => ({
        abierto: false,
        subiendo: false,
        error: '',
        imagen: null,        // ImageBitmap o <img>
        ancho: 0, alto: 0,   // de la foto, ya derecha (EXIF)
        lado: 0,             // del cuadro en pantalla, en px
        escala: 1, minima: 1,
        x: 0, y: 0,          // esquina de la foto dentro del cuadro
        zoom: 0,             // 0–100, la barrita
        _dedos: new Map(),
        _gesto: null,

        init() {
            window.addEventListener('popstate', () => { if (this.abierto) this.soltar(); });
            // El cuadro mide 0 hasta que el x-show lo muestra, y a veces eso pasa después del
            // $nextTick: se acomoda cuando de verdad tiene tamaño (y otra vez si giran el celular).
            new ResizeObserver(() => this.redimensionar()).observe(this.$refs.cuadro);
        },

        redimensionar() {
            const nuevo = this.$refs.cuadro.clientWidth;
            if (!this.imagen || !nuevo || nuevo === this.lado) return;
            if (!this.lado) { this.acomodar(); return; }
            // Mismo encuadre, a la nueva medida.
            const f = nuevo / this.lado;
            this.lado = nuevo;
            this.minima *= f;
            this.escala *= f;
            this.x *= f;
            this.y *= f;
            this.dibujar();
        },

        async elegir(evento) {
            const archivo = evento.target.files[0];
            if (!archivo) return;
            this.error = '';
            const imagen = await perfilAbrir(archivo);
            if (!imagen) {
                // No se pudo abrir aquí: que el servidor lo intente con la foto completa.
                this.subiendo = true;
                this.$refs.form.submit();
                return;
            }
            this.imagen = imagen;
            this.ancho = imagen.width;
            this.alto = imagen.height;
            history.pushState({ perfilRecorte: true }, '');
            this.abierto = true;
            this.$nextTick(() => this.acomodar());
        },

        // Tamaño del cuadro y la foto llenándolo, al centro.
        acomodar() {
            this.lado = this.$refs.cuadro.clientWidth;
            if (!this.lado) return;   // aún oculto: lo hace `redimensionar` al aparecer
            this.minima = this.lado / Math.min(this.ancho, this.alto);
            this.escala = this.minima;
            this.zoom = 0;
            this.x = (this.lado - this.ancho * this.escala) / 2;
            this.y = (this.lado - this.alto * this.escala) / 2;
            this.dibujar();
        },

        // Que la foto siempre cubra todo el círculo.
        limitar() {
            const w = this.ancho * this.escala;
            const h = this.alto * this.escala;
            this.x = Math.min(0, Math.max(this.lado - w, this.x));
            this.y = Math.min(0, Math.max(this.lado - h, this.y));
        },

        dibujar() {
            const c = this.$refs.vista;
            const dpr = window.devicePixelRatio || 1;
            if (c.width !== Math.round(this.lado * dpr)) {
                c.width = c.height = Math.round(this.lado * dpr);
            }
            const ctx = c.getContext('2d');
            ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
            ctx.clearRect(0, 0, this.lado, this.lado);
            ctx.drawImage(this.imagen, this.x, this.y, this.ancho * this.escala, this.alto * this.escala);
        },

        // Acercar o alejar dejando quieto el punto (cx, cy) del cuadro.
        escalar(nueva, cx = this.lado / 2, cy = this.lado / 2) {
            nueva = Math.min(this.minima * PERFIL_ZOOM_MAX, Math.max(this.minima, nueva));
            const f = nueva / this.escala;
            this.x = cx - (cx - this.x) * f;
            this.y = cy - (cy - this.y) * f;
            this.escala = nueva;
            this.zoom = Math.round(100 * Math.log(nueva / this.minima) / Math.log(PERFIL_ZOOM_MAX));
            this.limitar();
            this.dibujar();
        },

        conBarrita(valor) {
            this.escalar(this.minima * Math.pow(PERFIL_ZOOM_MAX, Number(valor) / 100));
        },

        // ── Dedos (pointer events: dedo, mouse o pluma) ──

        punto(e) {
            const r = this.$refs.cuadro.getBoundingClientRect();
            return { x: e.clientX - r.left, y: e.clientY - r.top };
        },

        bajar(e) {
            try { this.$refs.cuadro.setPointerCapture(e.pointerId); } catch { /* ya se fue ese dedo */ }
            this._dedos.set(e.pointerId, this.punto(e));
            this._gesto = null;
        },

        mover(e) {
            if (!this._dedos.has(e.pointerId)) return;
            const antes = [...this._dedos.values()];
            this._dedos.set(e.pointerId, this.punto(e));
            const ahora = [...this._dedos.values()];
            if (ahora.length === 1) {
                this.x += ahora[0].x - antes[0].x;
                this.y += ahora[0].y - antes[0].y;
                this.limitar();
                this.dibujar();
            } else if (ahora.length >= 2) {
                const [a, b] = ahora;
                const distancia = Math.hypot(a.x - b.x, a.y - b.y);
                const medio = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
                if (this._gesto) {
                    this.x += medio.x - this._gesto.medio.x;
                    this.y += medio.y - this._gesto.medio.y;
                    this.escalar(this.escala * distancia / this._gesto.distancia, medio.x, medio.y);
                }
                this._gesto = { distancia, medio };
            }
        },

        soltarDedo(e) {
            this._dedos.delete(e.pointerId);
            this._gesto = null;
        },

        rueda(e) {
            const p = this.punto(e);
            this.escalar(this.escala * (e.deltaY < 0 ? 1.1 : 1 / 1.1), p.x, p.y);
        },

        // ── Cerrar y mandar ──

        cancelar() {
            if (history.state && history.state.perfilRecorte) history.back();
            else this.soltar();
        },

        soltar() {
            this.abierto = false;
            if (this.imagen && this.imagen.close) this.imagen.close();
            this.imagen = null;
            this.$refs.archivo.value = '';
        },

        async listo() {
            const c = document.createElement('canvas');
            c.width = c.height = PERFIL_LADO;
            const k = PERFIL_LADO / this.lado;
            const ctx = c.getContext('2d');
            ctx.imageSmoothingQuality = 'high';
            ctx.drawImage(this.imagen, this.x * k, this.y * k, this.ancho * this.escala * k, this.alto * this.escala * k);
            const blob = await new Promise((r) => c.toBlob(r, 'image/jpeg', 0.9));
            if (!blob) { this.error = 'No pudimos preparar tu foto. Intenta con otra.'; return; }
            const datos = new FormData();
            datos.append('foto', blob, 'foto.jpg');
            this.subiendo = true;
            try {
                // El servidor contesta con un redirect a /perfil?ok=… o ?error=…; fetch lo sigue.
                const r = await fetch('/perfil/foto', { method: 'POST', body: datos });
                location.replace(r.url);
            } catch {
                this.subiendo = false;
                this.error = 'No se pudo mandar tu foto. Revisa tu internet e intenta de nuevo.';
            }
        },
    }));
});

// La foto ya derecha según su EXIF. `null` si este navegador no la puede abrir (HEIC en Android).
async function perfilAbrir(archivo) {
    if (window.createImageBitmap) {
        try { return await createImageBitmap(archivo, { imageOrientation: 'from-image' }); } catch { /* abajo */ }
    }
    const url = URL.createObjectURL(archivo);
    try {
        const img = new Image();
        img.src = url;
        await img.decode();
        return img;
    } catch {
        return null;
    } finally {
        URL.revokeObjectURL(url);
    }
}
