//! Tarjetas del chat: un producto, recibo o cotización que se manda en un mensaje (`Tipo` +
//! `ref`). En `Mensajes` solo se guarda el `ref`; los datos se buscan al dar la vista, para que
//! el precio, la existencia o el estatus estén al día.

use std::collections::HashMap;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::vista::Vista;
use crate::db;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Tarjeta {
    Producto {
        nombre: String,
        precio: String,
        /// La primera foto, si tiene.
        foto: Option<String>,
        existencia: String,
        url: String,
    },
    Recibo {
        fecha: String,
        hora: String,
        total: String,
        url: String,
    },
    Cotizacion {
        /// `Pedidos.Id`, el número que ve el cliente.
        numero: i32,
        estatus: &'static str,
        total: String,
        url: String,
    },
}

/// `Pedidos.Estatus` como lo lee el cliente.
fn estatus(n: i32) -> &'static str {
    match n {
        1 => "Pagada",
        2 => "Entregada",
        _ => "Nueva",
    }
}

/// Dónde están el sitio (`SITE_URL`, los links de la tarjeta) y las fotos de los productos
/// (`CONTENT_BASE_URL`).
#[derive(Clone, Copy)]
pub struct Urls<'a> {
    pub sitio: &'a str,
    pub fotos: &'a str,
}

impl<'a> From<&'a crate::config::Config> for Urls<'a> {
    fn from(c: &'a crate::config::Config) -> Self {
        Urls { sitio: &c.site_url, fotos: &c.content_base_url }
    }
}

/// La tarjeta de un `tipo` + `ref`. `None` si el `ref` no se entiende o ya no existe.
pub async fn resolver(pool: &PgPool, urls: Urls<'_>, tipo: &str, referencia: &str) -> Result<Option<Tarjeta>, sqlx::Error> {
    let referencia = referencia.trim();
    let sitio = urls.sitio;
    Ok(match tipo {
        "producto" => {
            let Ok(nid) = referencia.parse::<i32>() else { return Ok(None) };
            db::productos::detalle(pool, nid, urls.fotos).await?.map(|p| Tarjeta::Producto {
                nombre: p.nombre,
                // Como el total del recibo y la cotización: con `$`.
                precio: format!("${}", p.precio_venta),
                foto: p.fotos.into_iter().next(),
                existencia: p.stock,
                url: format!("{sitio}/productos/{nid}"),
            })
        }
        "recibo" => {
            let Ok(id) = referencia.parse::<Uuid>() else { return Ok(None) };
            db::monedero::recibo(pool, id).await?.map(|r| Tarjeta::Recibo {
                fecha: r.fecha,
                hora: r.hora,
                total: r.total,
                url: format!("{sitio}/recibo/{id}"),
            })
        }
        "cotizacion" => {
            let Ok(uid) = referencia.parse::<Uuid>() else { return Ok(None) };
            db::monedero::cotizacion(pool, uid).await?.map(|c| Tarjeta::Cotizacion {
                numero: c.id,
                estatus: estatus(c.estatus),
                total: c.total,
                url: format!("{sitio}/cotizacion/{uid}"),
            })
        }
        _ => None,
    })
}

/// Pone la tarjeta a los mensajes que la llevan (no a los eliminados). Si la misma sale varias
/// veces en la página, se busca una vez.
pub async fn poner(pool: &PgPool, urls: Urls<'_>, mensajes: &mut [Vista]) -> Result<(), sqlx::Error> {
    let mut ya: HashMap<(String, String), Option<Tarjeta>> = HashMap::new();
    for m in mensajes.iter_mut().filter(|m| !m.eliminado) {
        let Some(referencia) = m.referencia.clone() else { continue };
        let llave = (m.tipo.clone(), referencia);
        if !ya.contains_key(&llave) {
            let t = resolver(pool, urls, &llave.0, &llave.1).await?;
            ya.insert(llave.clone(), t);
        }
        m.tarjeta = ya[&llave].clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::chat::tests::{cliente, con_schema};

    const URLS: Urls = Urls { sitio: "https://xplaya.com", fotos: "https://cntnt.xplaya.com" };

    /// Las vistas (`v_inventario` y las del monedero) viven en `reportes.sql`, junto al schema.
    async fn con_vistas(pool: &PgPool) -> bool {
        if !con_schema(pool).await {
            return false;
        }
        let ruta = format!("{}/../inventario_papeleria/dbscripts/reportes.sql", env!("CARGO_MANIFEST_DIR"));
        let Ok(sql) = std::fs::read_to_string(&ruta) else {
            eprintln!("Sin {ruta}: se salta la prueba");
            return false;
        };
        sqlx::raw_sql(&sql).execute(pool).await.expect("cargar reportes.sql");
        true
    }

    /// Un producto con lo que pide `v_productos`: precio, categoría y unidad. Devuelve su nid.
    async fn producto(pool: &PgPool) -> i32 {
        let (p, cat, um) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        sqlx::raw_sql(&format!(
            "INSERT INTO categorias (id, nombre) VALUES ('{cat}', 'Papel');
             INSERT INTO unidadesmedida (id, nombre) VALUES ('{um}', 'Pieza');
             INSERT INTO productos (id, nombre, unidadmedidaid) VALUES ('{p}', 'Cartulina', '{um}');
             INSERT INTO categoriasproductos (id, categoriaid, productoid) VALUES ('{}', '{cat}', '{p}');
             INSERT INTO preciosproductos (id, productoid, fechacreado, precioventa) VALUES ('{}', '{p}', NOW(), 12.5);",
            Uuid::new_v4(),
            Uuid::new_v4()
        ))
        .execute(pool)
        .await
        .expect("crear el producto");
        sqlx::query_scalar("SELECT nid FROM productos WHERE id = $1").bind(p).fetch_one(pool).await.unwrap()
    }

    #[test]
    fn el_estatus_como_lo_lee_el_cliente() {
        assert_eq!([estatus(0), estatus(1), estatus(2)], ["Nueva", "Pagada", "Entregada"]);
    }

    #[sqlx::test(migrations = false)]
    async fn las_tres_tarjetas_con_datos_al_dia(pool: PgPool) {
        if !con_vistas(&pool).await {
            return;
        }
        let c = cliente(&pool, "9840000001").await;
        let nid = producto(&pool).await;
        let venta = Uuid::new_v4();
        sqlx::query("INSERT INTO ajustes (id, pago, cambio, fechaajuste, tipoajuste, clienteid) VALUES ($1, 50, 0, '2026-11-19 17:42:00', 0, $2)")
            .bind(venta)
            .bind(c)
            .execute(&pool)
            .await
            .unwrap();
        let pedido = Uuid::new_v4();
        let numero: i32 = sqlx::query_scalar(
            "INSERT INTO pedidos (uid, fechacreado, clienteid, estatus) VALUES ($1, '2026-11-19 17:42:00', $2, 1) RETURNING id",
        )
        .bind(pedido)
        .bind(c)
        .fetch_one(&pool)
        .await
        .unwrap();

        match resolver(&pool, URLS, "producto", &nid.to_string()).await.unwrap() {
            Some(Tarjeta::Producto { nombre, precio, url, foto, .. }) => {
                assert_eq!((nombre.as_str(), precio.as_str(), foto), ("Cartulina", "$12.50", None));
                assert_eq!(url, format!("https://xplaya.com/productos/{nid}"));
            }
            otra => panic!("producto: {otra:?}"),
        }
        match resolver(&pool, URLS, "recibo", &venta.to_string()).await.unwrap() {
            Some(Tarjeta::Recibo { total, url, .. }) => {
                assert_eq!(total, "$50.00");
                assert_eq!(url, format!("https://xplaya.com/recibo/{venta}"));
            }
            otra => panic!("recibo: {otra:?}"),
        }
        match resolver(&pool, URLS, "cotizacion", &pedido.to_string()).await.unwrap() {
            Some(Tarjeta::Cotizacion { numero: n, estatus, url, .. }) => {
                assert_eq!((n, estatus), (numero, "Pagada"));
                assert_eq!(url, format!("https://xplaya.com/cotizacion/{pedido}"));
            }
            otra => panic!("cotización: {otra:?}"),
        }

        // Lo que no existe o no se entiende: sin tarjeta.
        assert_eq!(resolver(&pool, URLS, "producto", "999999").await.unwrap(), None);
        assert_eq!(resolver(&pool, URLS, "producto", "abc").await.unwrap(), None);
        assert_eq!(resolver(&pool, URLS, "recibo", &Uuid::new_v4().to_string()).await.unwrap(), None);
        assert_eq!(resolver(&pool, URLS, "cotizacion", "no-es-guid").await.unwrap(), None);
        assert_eq!(resolver(&pool, URLS, "texto", "1").await.unwrap(), None);
    }

    #[sqlx::test(migrations = false)]
    async fn poner_no_toca_eliminados_ni_mensajes_sin_ref(pool: PgPool) {
        if !con_vistas(&pool).await {
            return;
        }
        let nid = producto(&pool).await;
        let vista = |referencia: Option<String>, eliminado: bool| Vista {
            id: Uuid::new_v4(),
            autor: "papeleria".into(),
            tipo: "producto".into(),
            texto: None,
            responde_a: None,
            cita: None,
            fecha: chrono::NaiveDateTime::default(),
            leido: false,
            editado: false,
            eliminado,
            adjuntos: vec![],
            zip: None,
            referencia,
            tarjeta: None,
        };
        let mut mensajes = [
            vista(Some(nid.to_string()), false),
            vista(Some(nid.to_string()), false),
            vista(Some(nid.to_string()), true),
            vista(None, false),
        ];
        poner(&pool, URLS, &mut mensajes).await.unwrap();
        let con: Vec<bool> = mensajes.iter().map(|m| m.tarjeta.is_some()).collect();
        assert_eq!(con, [true, true, false, false]);
        assert_eq!(mensajes[0].tarjeta, mensajes[1].tarjeta);
    }
}
