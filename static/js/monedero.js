// Pantalla del monedero (monedero/app.html). Va aparte de app.js porque la tarjeta también
// se sirve en xplaya.com/monedero/{id}, donde app.js no se carga (registra el service worker
// de la app). Se carga antes de Alpine: registra su componente en alpine:init.
document.addEventListener('alpine:init', () => {
    // El QR para la caja, la hoja de pago por
    // transferencia (copiar cada dato por separado) y el historial en su propia vista.
    Alpine.data('monederoPantalla', () => ({
        qr: false,
        pago: false,
        vista: 'inicio',
        copiado: '',

        verHistorial() {
            this.vista = 'historial';
            window.scrollTo(0, 0);
        },

        async copiar(que, texto) {
            try {
                await navigator.clipboard.writeText(texto);
            } catch {
                // Navegadores sin la API de portapapeles: el truco de siempre.
                const t = document.createElement('textarea');
                t.value = texto;
                t.setAttribute('readonly', '');
                t.style.position = 'fixed';
                t.style.opacity = '0';
                document.body.appendChild(t);
                t.select();
                document.execCommand('copy');
                t.remove();
            }
            this.copiado = que;
            setTimeout(() => { if (this.copiado === que) this.copiado = ''; }, 2000);
        },
    }));
});
