document.addEventListener('alpine:init', () => {
    Alpine.store('carrito', {
        items: JSON.parse(localStorage.getItem('xplaya_carrito') || '[]'),

        // Número total de artículos (suma de cantidades) — para el badge
        get total() {
            return this.items.reduce((sum, i) => sum + i.cantidad, 0);
        },

        // Subtotal en pesos
        get subtotal() {
            return this.items
                .reduce((sum, i) => sum + parseFloat(i.precio) * i.cantidad, 0)
                .toFixed(2);
        },

        agregar(producto) {
            const idx = this.items.findIndex(i => i.nid === producto.nid);
            if (idx >= 0) {
                this.items[idx].cantidad++;
            } else {
                this.items.push({ ...producto, cantidad: 1 });
            }
            this._guardar();
        },

        quitar(nid) {
            this.items = this.items.filter(i => i.nid !== nid);
            this._guardar();
        },

        cambiarCantidad(nid, cantidad) {
            const n = parseInt(cantidad, 10);
            if (n <= 0) {
                this.quitar(nid);
            } else {
                const idx = this.items.findIndex(i => i.nid === nid);
                if (idx >= 0) {
                    this.items[idx].cantidad = n;
                    this._guardar();
                }
            }
        },

        vaciar() {
            this.items = [];
            this._guardar();
        },

        _guardar() {
            localStorage.setItem('xplaya_carrito', JSON.stringify(this.items));
        },
    });
});
