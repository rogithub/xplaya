document.addEventListener('alpine:init', () => {
    Alpine.store('carrito', {
        // Migración: items viejos en localStorage no tienen _key — los calculamos al cargar.
        items: JSON.parse(localStorage.getItem('xplaya_carrito') || '[]')
            .map(i => ({ ...i, _key: i._key || String(i.nid) })),

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
            // Mismo producto con diferente presentación = línea separada en el carrito.
            const key = producto.presentacion_id
                ? `${producto.nid}_${producto.presentacion_id}`
                : String(producto.nid);
            const idx = this.items.findIndex(i => i._key === key);
            if (idx >= 0) {
                this.items[idx].cantidad++;
            } else {
                this.items.push({ ...producto, _key: key, cantidad: 1 });
            }
            this._guardar();
        },

        quitar(key) {
            this.items = this.items.filter(i => i._key !== key);
            this._guardar();
        },

        cambiarCantidad(key, cantidad) {
            const n = parseInt(cantidad, 10);
            if (n <= 0) {
                this.quitar(key);
            } else {
                const idx = this.items.findIndex(i => i._key === key);
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
