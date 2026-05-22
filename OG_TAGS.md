# Open Graph — Revisión de imágenes

| Página              | URL                        | `og:title`                                                        | `og:description`                                              | `og:image`                    |
|---------------------|----------------------------|-------------------------------------------------------------------|---------------------------------------------------------------|-------------------------------|
| Catálogo            | `/productos`               | Catálogo — xplaya.com                                            | Material escolar, de oficina y más...                        | `loguito.png`                 |
| Detalle producto    | `/productos/:id`           | `{nombre}` (foto del producto; fallback `loguito.png`)           | `${precio} \| {categoría}`                                    | foto del producto / `loguito.png` |
| Reseña              | `/resena`                  | ⭐ Déjanos tu reseña — Papelería xplaya.com                      | Tu opinión nos ayuda a seguir mejorando...                   | `resena.png`                  |
| Consulta de saldo   | `/saldo`                   | Monedero Electrónico — Papelería xplaya.com                      | Consulta tu saldo y cashback acumulado · Solo necesitas tu teléfono. | `saldo.jpeg`           |
| Monedero cliente    | `/monedero/:guid`          | 💳 Monedero de `{nombre}`                                        | Saldo disponible: $`{saldo}` · Miembro desde `{fecha}`        | `loguito.png`                 |
| Ticket de venta     | `/recibo/:guid`            | Ticket `{total}` · 🗓️ `{fecha}` · 🕐 `{hora}`                  | Toca aquí para ver los detalles de tu compra 👆              | `recibo.jpg`                  |
| Cotización          | `/cotizacion/:guid`        | Cotización `{total}` · 🗓️ `{fecha}` · 🕐 `{hora}`              | Toca para ver el detalle de tu cotización 👆                  | `loguito.png`                 |
