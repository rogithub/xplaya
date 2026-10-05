use rust_decimal::{prelude::ToPrimitive, Decimal};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::producto::{ComponenteKit, FeedProducto, Paginacion, Presentacion, ProductoCard, ProductoDetalle};

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
         WHERE vi.stock > 0 OR vi.stockilimitado -- misma regla que la caja: un kit sin material no se lista
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
struct ProductoMetaRow {
    marca: Option<String>,
    modelo: Option<String>,
    descripcion: Option<String>,
    codigobarrasitem: Option<String>,
}

/// GTIN válido = solo dígitos, longitud de un formato de barcode real
/// (GTIN-8/12/13/14) y dígito verificador correcto. Si no cumple, se trata
/// como código interno (sku) en vez de arriesgar un "gtin" inválido en el
/// JSON-LD o en el feed (Merchant Center rechaza GTINs con checksum malo).
fn clasificar_codigo_barras(raw: Option<String>) -> (Option<String>, Option<String>) {
    match raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        Some(c) if es_gtin_valido(&c) => (Some(c), None),
        Some(c) => (None, Some(c)),
        None => (None, None),
    }
}

/// Dígito verificador GS1: de derecha a izquierda (sin contar el último dígito)
/// los pesos alternan 3, 1, 3, 1... y la suma más el verificador debe ser múltiplo de 10.
fn es_gtin_valido(codigo: &str) -> bool {
    if !matches!(codigo.len(), 8 | 12 | 13 | 14) || !codigo.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let digitos: Vec<u32> = codigo.chars().filter_map(|c| c.to_digit(10)).collect();
    let (verificador, resto) = digitos.split_last().unwrap();
    let suma: u32 = resto
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| if i % 2 == 0 { d * 3 } else { *d })
        .sum();
    (10 - suma % 10) % 10 == *verificador
}

#[derive(sqlx::FromRow)]
struct FeedRow {
    nid: i32,
    nombre: String,
    categoria: String,
    ultimoprecioventa: Decimal,
    stock: Decimal,
    stockilimitado: bool,
    marca: Option<String>,
    descripcion: Option<String>,
    codigobarrasitem: Option<String>,
    fotos: Vec<String>,
}

/// Productos para los feeds de Google Merchant Center.
/// Misma regla de visibilidad que el catálogo (`stock > 0 OR stockilimitado`,
/// así un kit sin material no aparece), más lo que Google exige:
/// sin servicios ni precio libre, con precio y con al menos una foto.
pub async fn feed_productos(pool: &PgPool) -> Result<Vec<FeedProducto>, sqlx::Error> {
    let rows = sqlx::query_as::<_, FeedRow>(
        // DISTINCT ON: v_inventario repite el producto si tiene más de una fila
        // en CategoriasProductos; Google rechaza ids duplicados en el feed.
        "SELECT DISTINCT ON (vi.nid)
                vi.nid, vi.nombre, vi.categoria, vi.ultimoprecioventa,
                vi.stock, coalesce(vi.stockilimitado, false) AS stockilimitado,
                p.marca, p.descripcion, p.codigobarrasitem,
                (SELECT array_agg(fp.filename::text ORDER BY fp.filename)
                 FROM fotosproductos fp WHERE fp.productoid = vi.id) AS fotos
         FROM v_inventario vi
         JOIN productos p ON p.id = vi.id
         WHERE (vi.stock > 0 OR vi.stockilimitado)
           AND vi.esservicio IS NOT TRUE
           AND vi.preciolibre IS NOT TRUE
           AND vi.ultimoprecioventa > 0
           AND EXISTS (SELECT 1 FROM fotosproductos fp WHERE fp.productoid = vi.id)
         ORDER BY vi.nid, vi.categoria",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            // Hay productos por metro (stock fraccionario): se reportan piezas enteras.
            let cantidad = if r.stockilimitado {
                None
            } else {
                r.stock.floor().to_i64().filter(|q| *q > 0)
            };
            FeedProducto {
                nid: r.nid,
                nombre: r.nombre,
                categoria: r.categoria,
                precio_venta: format!("{:.2}", r.ultimoprecioventa),
                cantidad,
                marca: r.marca.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()),
                descripcion: r.descripcion.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()),
                gtin: clasificar_codigo_barras(r.codigobarrasitem).0,
                fotos: r.fotos,
            }
        })
        .collect())
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

    cargar_presentaciones(pool, &mut productos).await?;

    Ok((productos, paginacion))
}

/// Un solo query batch para todas las presentaciones de los productos de una página.
async fn cargar_presentaciones(
    pool: &PgPool,
    productos: &mut [ProductoCard],
) -> Result<(), sqlx::Error> {
    if productos.is_empty() {
        return Ok(());
    }

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
        for p in productos.iter_mut() {
            if let Some(pres) = mapa.remove(&p.nid) {
                p.presentaciones = pres;
            }
        }
    }

    Ok(())
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

    let meta = sqlx::query_as::<_, ProductoMetaRow>(
        "SELECT marca, modelo, descripcion, codigobarrasitem FROM productos WHERE id = $1",
    )
    .bind(row.id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(ProductoMetaRow { marca: None, modelo: None, descripcion: None, codigobarrasitem: None });

    let (gtin, sku) = clasificar_codigo_barras(meta.codigobarrasitem);

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
        marca: meta.marca,
        modelo: meta.modelo,
        descripcion: meta.descripcion,
        gtin,
        sku,
    }))
}
