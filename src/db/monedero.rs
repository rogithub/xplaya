use chrono::{Datelike, Duration, Local, Timelike};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::monedero::{
    AlertaVencimiento, ClienteMonedero, Cotizacion, HistorialEntry, ProductoCotizacion,
    ProductoRecibo, VentaRecibo,
};

// ── Helpers de formato ────────────────────────────────────────────────────────

fn fecha_es(dt: chrono::NaiveDateTime) -> String {
    const MESES: [&str; 12] = [
        "enero", "febrero", "marzo", "abril", "mayo", "junio",
        "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre",
    ];
    format!("{} de {}, {}", dt.day(), MESES[(dt.month() - 1) as usize], dt.year())
}

fn hora(dt: chrono::NaiveDateTime) -> String {
    format!("{:02}:{:02}", dt.hour(), dt.minute())
}

fn mxn(d: Decimal) -> String {
    format!("${:.2}", d)
}

// ── Structs internos (FromRow) ─────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct AjusteRow {
    id: Uuid,
    fechaajuste: chrono::NaiveDateTime,
    clienteid: Option<Uuid>,
    pago: Option<Decimal>,
    pagomonedero: Option<Decimal>,
    pagotarjeta: Option<Decimal>,
    pagotransferencia: Option<Decimal>,
    pagodolares: Option<Decimal>,
    tipocambiodolares: Option<Decimal>,
    cambio: Option<Decimal>,
}

#[derive(sqlx::FromRow)]
struct ProductoAjusteRow {
    nombre: String,
    cantidad: Decimal,
    preciounitarioventa: Option<Decimal>,
    esingresotrasladado: bool,
}

#[derive(sqlx::FromRow)]
struct PedidoRow {
    uid: Uuid,
    fechacreado: chrono::NaiveDateTime,
    clienteid: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct PedidoItemRow {
    nombre: String,
    cantidad: Decimal,
    precioventa: Option<Decimal>,
}

#[derive(sqlx::FromRow)]
struct ClienteRow {
    nombre: String,
    fechacreado: chrono::NaiveDateTime,
}

#[derive(sqlx::FromRow)]
struct HistorialRow {
    ajusteid: Uuid,
    fechaajuste: chrono::NaiveDateTime,
    dinerogenerado: Decimal,
    dinerogastado: Decimal,
    fechaexpiracion: chrono::NaiveDateTime,
    tienedevolucion: bool,
}

// ── Queries públicas ───────────────────────────────────────────────────────────

pub async fn recibo(pool: &PgPool, id: Uuid) -> Result<Option<VentaRecibo>, sqlx::Error> {
    let Some(venta) = sqlx::query_as::<_, AjusteRow>(
        "SELECT id, fechaajuste, clienteid,
                COALESCE(pago, 0) AS pago,
                COALESCE(pagomonedero, 0) AS pagomonedero,
                COALESCE(pagotarjeta, 0) AS pagotarjeta,
                COALESCE(pagotransferencia, 0) AS pagotransferencia,
                COALESCE(pagodolares, 0) AS pagodolares,
                COALESCE(tipocambiodolares, 0) AS tipocambiodolares,
                COALESCE(cambio, 0) AS cambio
         FROM ajustes
         WHERE id = $1 AND tipoajuste = 0",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let productos_rows = sqlx::query_as::<_, ProductoAjusteRow>(
        "SELECT p.nombre, ap.cantidad,
                COALESCE(ap.preciounitarioventa, 0) AS preciounitarioventa,
                EXISTS(
                    SELECT 1 FROM v_ingresos_trasladados v WHERE v.id = p.id
                ) AS esingresotrasladado
         FROM ajustesproductos ap
         JOIN productos p ON p.id = ap.productoid
         WHERE ap.ajusteid = $1
         ORDER BY ap.datestamp",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    // Solo mostramos datos de monedero si el cliente vinculado aceptó el programa.
    // Si no aceptó (o no hay cliente), el recibo aparece sin sección de monedero.
    let acepto_programa: bool = match venta.clienteid {
        Some(cid) => sqlx::query_scalar(
            "SELECT aceptoprograma FROM clientes WHERE id = $1",
        )
        .bind(cid)
        .fetch_optional(pool)
        .await?
        .unwrap_or(false),
        None => false,
    };

    let monedero_generado: Decimal = if acepto_programa {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(g.dinerodigital), 0)
             FROM monederogenerados g
             JOIN ajustesproductos ap ON g.ajusteproductoid = ap.id
             WHERE ap.ajusteid = $1 AND g.devolucionid IS NULL",
        )
        .bind(id)
        .fetch_one(pool)
        .await?
    } else {
        Decimal::ZERO
    };

    let saldo_monedero: Decimal = if acepto_programa {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(dinerodigitaldisponible), 0)
             FROM v_ajuste_producto_monedero
             WHERE clienteid = $1",
        )
        .bind(venta.clienteid.expect("acepto_programa implica clienteid"))
        .fetch_one(pool)
        .await?
    } else {
        Decimal::ZERO
    };

    let pago = venta.pago.unwrap_or(Decimal::ZERO);
    let pago_monedero = venta.pagomonedero.unwrap_or(Decimal::ZERO);
    let pago_tarjeta = venta.pagotarjeta.unwrap_or(Decimal::ZERO);
    let pago_transferencia = venta.pagotransferencia.unwrap_or(Decimal::ZERO);
    let pago_dolares = venta.pagodolares.unwrap_or(Decimal::ZERO);
    let tipo_cambio = venta.tipocambiodolares.unwrap_or(Decimal::ZERO);
    let cambio = venta.cambio.unwrap_or(Decimal::ZERO);

    let total = pago + pago_monedero + pago_tarjeta + pago_transferencia
        + (pago_dolares * tipo_cambio)
        - cambio;

    let saldo_anterior = saldo_monedero - monedero_generado + pago_monedero;
    let hay_actividad_monedero = monedero_generado > Decimal::ZERO || pago_monedero > Decimal::ZERO;

    let productos = productos_rows
        .into_iter()
        .map(|p| {
            let precio = p.preciounitarioventa.unwrap_or(Decimal::ZERO);
            let subtotal = precio * p.cantidad;
            ProductoRecibo {
                nombre: p.nombre,
                cantidad: format!("{:.2}", p.cantidad),
                precio_unitario: mxn(precio),
                subtotal: mxn(subtotal),
                es_ingreso_trasladado: p.esingresotrasladado,
            }
        })
        .collect();

    Ok(Some(VentaRecibo {
        id: venta.id,
        fecha: fecha_es(venta.fechaajuste),
        hora: hora(venta.fechaajuste),
        cliente_id: venta.clienteid,
        total: mxn(total),
        pago: mxn(pago),
        pago_monedero: mxn(pago_monedero),
        pago_tarjeta: mxn(pago_tarjeta),
        pago_transferencia: mxn(pago_transferencia),
        pago_dolares: mxn(pago_dolares),
        tipo_cambio_dolares: format!("{:.2}", tipo_cambio),
        cambio: mxn(cambio),
        productos,
        monedero_generado: mxn(monedero_generado),
        saldo_monedero: mxn(saldo_monedero),
        saldo_anterior: mxn(saldo_anterior),
        tiene_monedero: acepto_programa,
        hay_actividad_monedero,
    }))
}

pub async fn cotizacion(pool: &PgPool, uid: Uuid) -> Result<Option<Cotizacion>, sqlx::Error> {
    let Some(pedido) = sqlx::query_as::<_, PedidoRow>(
        "SELECT uid, fechacreado, clienteid FROM pedidos WHERE uid = $1",
    )
    .bind(uid)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let items = sqlx::query_as::<_, PedidoItemRow>(
        "SELECT p.nombre, pi.cantidad,
                COALESCE(pp.precioventa, 0) AS precioventa
         FROM pedidoitems pi
         JOIN productos p ON p.id = pi.productoid
         LEFT JOIN LATERAL (
             SELECT precioventa FROM preciosproductos
             WHERE productoid = pi.productoid
             ORDER BY fechacreado DESC LIMIT 1
         ) pp ON true
         WHERE pi.pedidoid = $1
         ORDER BY pi.id",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;

    let mut total = Decimal::ZERO;
    let productos: Vec<ProductoCotizacion> = items
        .into_iter()
        .map(|it| {
            let precio = it.precioventa.unwrap_or(Decimal::ZERO);
            let subtotal = precio * it.cantidad;
            total += subtotal;
            ProductoCotizacion {
                nombre: it.nombre,
                cantidad: format!("{:.2}", it.cantidad),
                precio: mxn(precio),
                subtotal: mxn(subtotal),
            }
        })
        .collect();

    let tiene_monedero: bool = match pedido.clienteid {
        Some(cid) => sqlx::query_scalar(
            "SELECT aceptoprograma FROM clientes WHERE id = $1",
        )
        .bind(cid)
        .fetch_optional(pool)
        .await?
        .unwrap_or(false),
        None => false,
    };

    Ok(Some(Cotizacion {
        uid: pedido.uid,
        fecha: fecha_es(pedido.fechacreado),
        hora: hora(pedido.fechacreado),
        cliente_id: pedido.clienteid,
        tiene_monedero,
        productos,
        total: mxn(total),
    }))
}

pub async fn monedero(pool: &PgPool, cliente_id: Uuid) -> Result<Option<ClienteMonedero>, sqlx::Error> {
    // Solo se muestra el monedero si el cliente aceptó explícitamente el programa.
    let Some(cliente) = sqlx::query_as::<_, ClienteRow>(
        "SELECT nombre, fechacreado FROM clientes WHERE id = $1 AND aceptoprograma = true",
    )
    .bind(cliente_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let saldo: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(dinerodigitaldisponible), 0)
         FROM v_ajuste_producto_monedero
         WHERE clienteid = $1",
    )
    .bind(cliente_id)
    .fetch_one(pool)
    .await?;

    let historial_rows = sqlx::query_as::<_, HistorialRow>(
        "SELECT
             a.id AS ajusteid,
             a.fechaajuste,
             SUM(g.dinerodigital) AS dinerogenerado,
             COALESCE(SUM(gasto.dinerodigitalgastado), 0) AS dinerogastado,
             MIN(g.fechaexpiracion) AS fechaexpiracion,
             BOOL_OR(g.devolucionid IS NOT NULL) AS tienedevolucion
         FROM ajustes a
         JOIN ajustesproductos ap ON a.id = ap.ajusteid
         JOIN monederogenerados g ON g.ajusteproductoid = ap.id
         LEFT JOIN (
             SELECT monederogeneradosid, SUM(dinerodigital) AS dinerodigitalgastado
             FROM monederoredimidos
             GROUP BY monederogeneradosid
         ) gasto ON gasto.monederogeneradosid = g.id
         WHERE a.clienteid = $1 AND a.tipoajuste = 0
         GROUP BY a.id, a.fechaajuste
         ORDER BY a.fechaajuste DESC",
    )
    .bind(cliente_id)
    .fetch_all(pool)
    .await?;

    let now = Local::now().naive_local();
    let threshold = now + Duration::days(30);

    let mut alerta_monto = Decimal::ZERO;
    let mut alerta_fecha: Option<chrono::NaiveDateTime> = None;

    let historial: Vec<HistorialEntry> = historial_rows
        .iter()
        .map(|row| {
            let disponible_neto = row.dinerogenerado - row.dinerogastado;

            if !row.tienedevolucion
                && disponible_neto > Decimal::ZERO
                && row.fechaexpiracion > now
                && row.fechaexpiracion <= threshold
            {
                alerta_monto += disponible_neto;
                alerta_fecha = Some(match alerta_fecha {
                    None => row.fechaexpiracion,
                    Some(prev) => prev.min(row.fechaexpiracion),
                });
            }

            let estado = if row.tienedevolucion {
                "Devuelto"
            } else if disponible_neto > Decimal::ZERO {
                "Disponible"
            } else {
                "Agotado"
            };

            HistorialEntry {
                ajuste_id: row.ajusteid,
                fecha: fecha_es(row.fechaajuste),
                dinero_generado: format!("{:.2}", row.dinerogenerado),
                dinero_gastado: format!("{:.2}", row.dinerogastado),
                disponible_neto: format!("{:.2}", disponible_neto),
                fecha_expiracion: fecha_es(row.fechaexpiracion),
                estado: estado.to_string(),
            }
        })
        .collect();

    let alerta = if alerta_monto > Decimal::ZERO {
        Some(AlertaVencimiento {
            monto: format!("{:.2}", alerta_monto),
            fecha: alerta_fecha.map(fecha_es).unwrap_or_default(),
        })
    } else {
        None
    };

    Ok(Some(ClienteMonedero {
        nombre: cliente.nombre,
        miembro_desde: fecha_es(cliente.fechacreado),
        saldo: format!("{:.2}", saldo),
        historial,
        alerta,
    }))
}

pub async fn buscar_cliente(pool: &PgPool, tel: &str) -> Result<Option<Uuid>, sqlx::Error> {
    // Solo busca clientes que aceptaron participar en el programa de monedero.
    sqlx::query_scalar(
        "SELECT id FROM clientes WHERE right(telefono, 10) = $1 AND aceptoprograma = true LIMIT 1",
    )
    .bind(tel)
    .fetch_optional(pool)
    .await
}
