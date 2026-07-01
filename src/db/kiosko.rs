use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::kiosko::FamiliaTile;
use crate::models::producto::{Paginacion, ProductoCard};

const ITEMS_POR_PAGINA: i64 = 12; // grid de 3 columnas → 4 filas por página

#[derive(sqlx::FromRow)]
struct KioskoRow {
    nid: i32,
    nombre: String,
    categoria: String,
    unidadmedida: String,
    precioventa: Decimal,
    foto: Option<String>,
    total_items: i64,
}

/// Catálogo del kiosko: los productos con MENOS ventas en los últimos 30 días
/// aparecen primero — el objetivo estratégico es rotar inventario de baja venta.
/// No reemplaza a `productos::busqueda()`, que ordena por prioridad de galería.
pub async fn kiosko_lista(
    pool: &PgPool,
    familia_id: Option<Uuid>,
    busqueda: Option<&str>,
    pagina: i32,
    content_base_url: &str,
) -> Result<(Vec<ProductoCard>, Paginacion), sqlx::Error> {
    let pagina = pagina.clamp(1, 10_000);
    let offset = (i64::from(pagina) - 1) * ITEMS_POR_PAGINA;

    // Ventas por producto en subquery separado — sumar ap.cantidad tras agrupar
    // evita la trampa del JOIN multiplicador (que solo aplica a Ajustes.Pago).
    // Mismos filtros de visibilidad que v_galeria_principal: sin servicios,
    // con stock (o kit) y con precio.
    let rows = sqlx::query_as::<_, KioskoRow>(
        "WITH ventas AS (
             SELECT ap.productoid, SUM(ap.cantidad) AS cantidad
             FROM ajustesproductos ap
             JOIN ajustes a ON a.id = ap.ajusteid
             WHERE a.tipoajuste = 0
               AND a.fechaajuste >= NOW() - INTERVAL '30 days'
             GROUP BY ap.productoid
         )
         SELECT vi.nid, vi.nombre, vi.categoria, vi.unidadmedida,
                vi.ultimoprecioventa AS precioventa,
                (SELECT fp.filename FROM fotosproductos fp
                 WHERE fp.productoid = vi.id
                 ORDER BY fp.filename LIMIT 1) AS foto,
                COUNT(*) OVER () AS total_items
         FROM v_inventario vi
         LEFT JOIN ventas v ON v.productoid = vi.id
         WHERE vi.esservicio = false
           AND (vi.stock > 0 OR vi.escompuesto = true)
           AND vi.ultimoprecioventa > 0
           AND ($1::text IS NULL
                OR unaccent(vi.nombre) ILIKE unaccent('%' || $1 || '%')
                OR unaccent(vi.categoria) ILIKE unaccent('%' || $1 || '%'))
           AND ($4::uuid IS NULL OR EXISTS (
                SELECT 1 FROM productos p
                WHERE p.id = vi.id AND p.familiasemanticaid = $4))
         ORDER BY COALESCE(v.cantidad, 0) ASC, vi.nombre ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(busqueda)
    .bind(ITEMS_POR_PAGINA)
    .bind(offset)
    .bind(familia_id)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok((vec![], Paginacion::new(0, pagina, 0)));
    }

    let total_items = rows[0].total_items;
    let total_paginas = ((total_items + ITEMS_POR_PAGINA - 1) / ITEMS_POR_PAGINA) as i32;

    let productos = rows
        .into_iter()
        .map(|r| ProductoCard {
            nid: r.nid,
            nombre: r.nombre,
            categoria: r.categoria,
            precio_venta: format!("{:.2}", r.precioventa),
            unidad_medida: r.unidadmedida,
            foto_url: r.foto.map(|f| {
                format!("{}/papeleria-fotos-productos/{}", content_base_url, f)
            }),
            presentaciones: vec![],
        })
        .collect();

    Ok((productos, Paginacion::new(total_items, pagina, total_paginas)))
}

/// Tiles de navegación del kiosko: las 37 FamiliasSemanticas curadas a mano.
/// El conteo usa los mismos filtros de visibilidad que kiosko_lista() para que
/// el número del tile coincida con lo que muestra el grid al filtrar.
pub async fn familias_semanticas(pool: &PgPool) -> Result<Vec<FamiliaTile>, sqlx::Error> {
    sqlx::query_as::<_, FamiliaTile>(
        "SELECT fs.id, fs.nombre, COUNT(*) AS total_productos
         FROM familiassemanticas fs
         JOIN productos p ON p.familiasemanticaid = fs.id
         JOIN v_inventario vi ON vi.id = p.id
         WHERE vi.esservicio = false
           AND (vi.stock > 0 OR vi.escompuesto = true)
           AND vi.ultimoprecioventa > 0
         GROUP BY fs.id, fs.nombre
         ORDER BY total_productos DESC, fs.nombre ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn familia_nombre(pool: &PgPool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT nombre FROM familiassemanticas WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}
