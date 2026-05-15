use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::producto::{Paginacion, ProductoCard, ProductoDetalle};

// Filas internas que sqlx mapea directamente desde la BD.
// Solo se usan aquí para construir los modelos públicos.

#[derive(sqlx::FromRow)]
struct GaleriaRow {
    nid: i32,
    nombre: String,
    categoria: String,
    unidadmedida: String,
    precioventa: Decimal,
    foto: Option<String>,
    total_items: i64,
    pagina_actual: i32,
    total_paginas: i32,
}

#[derive(sqlx::FromRow)]
struct InventarioRow {
    nid: i32,
    id: Uuid,
    nombre: String,
    categoria: String,
    unidadmedida: String,
    ultimoprecioventa: Decimal,
    stock: Decimal,
}

pub async fn busqueda(
    pool: &PgPool,
    busqueda: Option<&str>,
    pagina: i32,
    content_base_url: &str,
) -> Result<(Vec<ProductoCard>, Paginacion), sqlx::Error> {
    let rows = sqlx::query_as::<_, GaleriaRow>(
        "SELECT nid, nombre, categoria, unidadmedida, precioventa, foto,
                total_items, pagina_actual, total_paginas
         FROM fn_galeria_busqueda_paginada($1::text, $2::int4, $3::int4)",
    )
    .bind(busqueda)
    .bind(pagina)
    .bind(20i32)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok((
            vec![],
            Paginacion { total_items: 0, pagina_actual: pagina, total_paginas: 0 },
        ));
    }

    let paginacion = Paginacion {
        total_items: rows[0].total_items,
        pagina_actual: rows[0].pagina_actual,
        total_paginas: rows[0].total_paginas,
    };

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
        })
        .collect();

    Ok((productos, paginacion))
}

pub async fn detalle(
    pool: &PgPool,
    nid: i32,
    content_base_url: &str,
) -> Result<Option<ProductoDetalle>, sqlx::Error> {
    let row = sqlx::query_as::<_, InventarioRow>(
        "SELECT nid, id, nombre, categoria, unidadmedida, ultimoprecioventa, stock
         FROM v_inventario WHERE nid = $1",
    )
    .bind(nid)
    .fetch_optional(pool)
    .await?;

    let row = match row {
        Some(r) => r,
        None => return Ok(None),
    };

    let fotos = sqlx::query_scalar::<_, String>(
        "SELECT filename FROM fotosproductos WHERE productoid = $1 ORDER BY filename",
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|f| format!("{}/papeleria-fotos-productos/{}", content_base_url, f))
    .collect();

    let videos = sqlx::query_scalar::<_, String>(
        "SELECT url FROM urlcontentproductos WHERE productoid = $1 ORDER BY fechacreado DESC",
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;

    Ok(Some(ProductoDetalle {
        nid: row.nid,
        nombre: row.nombre,
        categoria: row.categoria,
        precio_venta: format!("{:.2}", row.ultimoprecioventa),
        unidad_medida: row.unidadmedida,
        stock: format!("{:.2}", row.stock),
        fotos,
        videos,
    }))
}
