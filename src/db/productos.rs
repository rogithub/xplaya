use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::producto::{ComponenteKit, Paginacion, Presentacion, ProductoCard, ProductoDetalle};

#[derive(sqlx::FromRow)]
struct SitemapRow {
    nid: i32,
    filename: Option<String>,
}

pub async fn sitemap_productos(pool: &PgPool) -> Result<Vec<(i32, Vec<String>)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, SitemapRow>(
        "SELECT vi.nid, fp.filename
         FROM v_inventario vi
         LEFT JOIN fotosproductos fp ON fp.productoid = vi.id
         WHERE vi.stock > 0 OR vi.escompuesto = true
         ORDER BY vi.nid, fp.filename",
    )
    .fetch_all(pool)
    .await?;

    let mut result: Vec<(i32, Vec<String>)> = Vec::new();
    for row in rows {
        if result.last().map(|(n, _)| *n) != Some(row.nid) {
            result.push((row.nid, Vec::new()));
        }
        if let Some(f) = row.filename {
            result.last_mut().unwrap().1.push(f);
        }
    }
    Ok(result)
}

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
    escompuesto: bool,
}

#[derive(sqlx::FromRow)]
struct ComponenteRow {
    nombre: String,
    cantidad: Decimal,
}

#[derive(sqlx::FromRow)]
struct PresentacionRow {
    id: Uuid,
    nombre: String,
    factor: Decimal,
    precioventa: Decimal,
}

// Para el catálogo necesitamos el nid para agrupar por producto
#[derive(sqlx::FromRow)]
struct PresentacionCatalogoRow {
    nid: i32,
    id: Uuid,
    nombre: String,
    factor: Decimal,
    precioventa: Decimal,
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
        return Ok((vec![], Paginacion::new(0, pagina, 0)));
    }

    let paginacion = Paginacion::new(
        rows[0].total_items,
        rows[0].pagina_actual,
        rows[0].total_paginas,
    );

    let mut productos: Vec<ProductoCard> = rows
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

    // Un solo query batch para todas las presentaciones de los productos de esta página.
    let nids: Vec<i32> = productos.iter().map(|p| p.nid).collect();
    let pres_rows = sqlx::query_as::<_, PresentacionCatalogoRow>(
        "SELECT vi.nid, pp.id, pp.nombre, pp.factor, pp.precioventa
         FROM productopresentaciones pp
         JOIN v_inventario vi ON vi.id = pp.productoid
         WHERE vi.nid = ANY($1)
         ORDER BY vi.nid, pp.precioventa",
    )
    .bind(&nids[..])
    .fetch_all(pool)
    .await?;

    if !pres_rows.is_empty() {
        let mut mapa: std::collections::HashMap<i32, Vec<Presentacion>> =
            std::collections::HashMap::new();
        for row in pres_rows {
            mapa.entry(row.nid).or_default().push(Presentacion {
                id: row.id,
                nombre: row.nombre,
                factor: format!("{:.2}", row.factor),
                precio_venta: format!("{:.2}", row.precioventa),
            });
        }
        for p in &mut productos {
            if let Some(pres) = mapa.remove(&p.nid) {
                p.presentaciones = pres;
            }
        }
    }

    Ok((productos, paginacion))
}

pub async fn detalle(
    pool: &PgPool,
    nid: i32,
    content_base_url: &str,
) -> Result<Option<ProductoDetalle>, sqlx::Error> {
    let row = sqlx::query_as::<_, InventarioRow>(
        "SELECT nid, id, nombre, categoria, unidadmedida, ultimoprecioventa, stock, escompuesto
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

    // Los kits no tienen stock propio: se listan sus componentes para que el
    // cliente vea qué incluye. El POS expande el kit al momento de la venta.
    let componentes: Vec<ComponenteKit> = if row.escompuesto {
        sqlx::query_as::<_, ComponenteRow>(
            "SELECT p.nombre, pc.cantidad
             FROM productocomponentes pc
             JOIN productos p ON p.id = pc.componenteid
             WHERE pc.productopadreid = $1
             ORDER BY p.nombre",
        )
        .bind(row.id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|c| ComponenteKit {
            nombre: c.nombre,
            cantidad: format!("{}", c.cantidad.normalize()),
        })
        .collect()
    } else {
        vec![]
    };

    let presentaciones = sqlx::query_as::<_, PresentacionRow>(
        "SELECT id, nombre, factor, precioventa
         FROM productopresentaciones
         WHERE productoid = $1
         ORDER BY precioventa",
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|p| Presentacion {
        id: p.id,
        nombre: p.nombre,
        factor: format!("{:.2}", p.factor),
        precio_venta: format!("{:.2}", p.precioventa),
    })
    .collect();

    Ok(Some(ProductoDetalle {
        nid: row.nid,
        id: row.id,
        nombre: row.nombre,
        categoria: row.categoria,
        precio_venta: format!("{:.2}", row.ultimoprecioventa),
        unidad_medida: row.unidadmedida,
        stock: format!("{:.2}", row.stock),
        fotos,
        videos,
        presentaciones,
        componentes,
    }))
}
