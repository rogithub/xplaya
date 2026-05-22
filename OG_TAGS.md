# Open Graph — Revisión de imágenes

| Página              | URL                        | `og:title`                                                        | `og:description`                                              | `og:image`                    |
|---------------------|----------------------------|-------------------------------------------------------------------|---------------------------------------------------------------|-------------------------------|
| Catálogo            | `/productos`               | Catálogo — xplaya.com                                            | Material escolar, de oficina y más...                        | `og_catalogo.jpeg`            |
| Detalle producto    | `/productos/:id`           | `{nombre}` (foto del producto; fallback `og_producto.jpeg`)      | `${precio} \| {categoría}`                                    | foto del producto / `og_producto.jpeg` |
| Reseña              | `/resena`                  | ⭐ Déjanos tu reseña — Papelería xplaya.com                      | Tu opinión nos ayuda a seguir mejorando...                   | `og_resena.jpeg`              |
| Consulta de saldo   | `/saldo`                   | Monedero Electrónico — Papelería xplaya.com                      | Consulta tu saldo y cashback acumulado · Solo necesitas tu teléfono. | `og_saldo.jpeg`        |
| Monedero cliente    | `/monedero/:guid`          | 💳 Monedero de `{nombre}`                                        | Saldo disponible: $`{saldo}` · Miembro desde `{fecha}`        | `og_monedero.jpeg`            |
| Ticket de venta     | `/recibo/:guid`            | Ticket `{total}` · 🗓️ `{fecha}` · 🕐 `{hora}`                  | Toca aquí para ver los detalles de tu compra 👆              | `og_recibo.jpeg`              |
| Cotización          | `/cotizacion/:guid`        | Cotización `{total}` · 🗓️ `{fecha}` · 🕐 `{hora}`              | Toca para ver el detalle de tu cotización 👆                  | `og_cotizacion.jpeg`          |
