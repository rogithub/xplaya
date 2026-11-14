//! Trabajos del autoservicio de impresión (`TrabajosImpresion`, definida en el repo del POS).
//!
//! Estados: `preparado` (el cliente lo armó; la caja no lo ve) → `por_autorizar` (lo mandó desde
//! la tienda) → `autorizado` (la caja eligió impresora, en el POS) → `imprimiendo` (xplaya lo tomó)
//! → `impreso`; o `rechazado`, `error` y `vencido` (a los 7 días).
//! Las fechas van en hora de la tienda, como las del POS.

use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::impresion::Cuenta;

/// Días que vive un trabajo (y su PDF) desde que se subió.
pub const DIAS_VIDA: i32 = 7;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Trabajo {
    pub id: Uuid,
    pub archivo: String,
    pub objeto: String,
    pub paginas_pdf: i32,
    pub paginas: String,
    pub caras: i32,
    pub copias: i32,
    pub doble_cara: bool,
    pub hojas: i32,
    pub precio_cara: Decimal,
    pub estado: String,
    pub error: Option<String>,
}

impl Trabajo {
    pub fn total(&self) -> Decimal {
        Decimal::from(self.caras * self.copias) * self.precio_cara
    }
}

const COLUMNAS: &str = "id, archivo, objeto, paginaspdf AS paginas_pdf, paginas, caras, copias,
     doblecara AS doble_cara, hojas, preciocara AS precio_cara, estado, error";

/// Precio de una cara: el vigente del producto b/n de la primera impresora del autoservicio
/// (`Settings.AUTOSERVICIO_IMPRESORAS`). Todas cobran $1; el pedido usa el de la que imprima.
pub async fn precio_cara(pool: &PgPool) -> Result<Option<Decimal>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT pp.precioventa
           FROM settings s
           CROSS JOIN LATERAL (SELECT (s.value::jsonb -> 0 ->> 'productoId')::uuid AS id) p
           JOIN preciosproductos pp ON pp.productoid = p.id
          WHERE s.key = 'AUTOSERVICIO_IMPRESORAS'
          ORDER BY pp.fechacreado DESC
          LIMIT 1",
    )
    .fetch_optional(pool)
    .await
}

pub struct Nuevo<'a> {
    pub id: Uuid,
    pub cliente_id: Uuid,
    pub archivo: &'a str,
    pub objeto: &'a str,
    pub paginas_pdf: u32,
    pub cuenta: &'a Cuenta,
    pub copias: u32,
    pub doble_cara: bool,
    pub precio_cara: Decimal,
}

pub async fn crear(pool: &PgPool, t: &Nuevo<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO trabajosimpresion
            (id, clienteid, archivo, objeto, paginaspdf, paginas, caras, copias, doblecara, hojas,
             preciocara, fechacreado)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW() AT TIME ZONE 'America/Cancun')",
    )
    .bind(t.id)
    .bind(t.cliente_id)
    .bind(t.archivo)
    .bind(t.objeto)
    .bind(t.paginas_pdf as i32)
    .bind(&t.cuenta.paginas)
    .bind(t.cuenta.caras as i32)
    .bind(t.copias as i32)
    .bind(t.doble_cara)
    .bind(t.cuenta.hojas as i32)
    .bind(t.precio_cara)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Los trabajos vivos del cliente, el más nuevo primero.
pub async fn de_cliente(pool: &PgPool, cliente_id: Uuid) -> Result<Vec<Trabajo>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNAS} FROM trabajosimpresion
          WHERE clienteid = $1 AND estado <> 'vencido'
            AND fechacreado > (NOW() AT TIME ZONE 'America/Cancun') - make_interval(days => $2)
          ORDER BY fechacreado DESC"
    ))
    .bind(cliente_id)
    .bind(DIAS_VIDA)
    .fetch_all(pool)
    .await
}

pub async fn uno(pool: &PgPool, id: Uuid, cliente_id: Uuid) -> Result<Option<Trabajo>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNAS} FROM trabajosimpresion WHERE id = $1 AND clienteid = $2 AND estado <> 'vencido'"
    ))
    .bind(id)
    .bind(cliente_id)
    .fetch_optional(pool)
    .await
}

/// Cambia páginas, copias y caras. Solo mientras está `preparado`: `false` si ya no.
pub async fn cambiar_opciones(
    pool: &PgPool,
    id: Uuid,
    cliente_id: Uuid,
    cuenta: &Cuenta,
    copias: u32,
    doble_cara: bool,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE trabajosimpresion
            SET paginas = $3, caras = $4, copias = $5, doblecara = $6, hojas = $7
          WHERE id = $1 AND clienteid = $2 AND estado = 'preparado'",
    )
    .bind(id)
    .bind(cliente_id)
    .bind(&cuenta.paginas)
    .bind(cuenta.caras as i32)
    .bind(copias as i32)
    .bind(doble_cara)
    .bind(cuenta.hojas as i32)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Quita un trabajo que no está en la caja ni imprimiéndose. Devuelve su objeto de MinIO.
pub async fn quitar(pool: &PgPool, id: Uuid, cliente_id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "DELETE FROM trabajosimpresion
          WHERE id = $1 AND clienteid = $2 AND estado IN ('preparado', 'rechazado', 'error')
          RETURNING objeto",
    )
    .bind(id)
    .bind(cliente_id)
    .fetch_optional(pool)
    .await
}

/// El cliente está en la tienda (escaneó el QR del kiosko): sus trabajos preparados pasan a la
/// caja. Devuelve cuántos.
pub async fn mandar(pool: &PgPool, cliente_id: Uuid) -> Result<u64, sqlx::Error> {
    sqlx::query(
        "UPDATE trabajosimpresion
            SET estado = 'por_autorizar', fechaenviado = NOW() AT TIME ZONE 'America/Cancun'
          WHERE clienteid = $1 AND estado = 'preparado'
            AND fechacreado > (NOW() AT TIME ZONE 'America/Cancun') - make_interval(days => $2)",
    )
    .bind(cliente_id)
    .bind(DIAS_VIDA)
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
}

/// Trabajos que ya cumplieron sus días y todavía no se marcan `vencido` (sus PDFs por borrar).
pub async fn vencidos(pool: &PgPool) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, objeto FROM trabajosimpresion
          WHERE estado <> 'vencido'
            AND fechacreado <= (NOW() AT TIME ZONE 'America/Cancun') - make_interval(days => $1)
          LIMIT 200",
    )
    .bind(DIAS_VIDA)
    .fetch_all(pool)
    .await
}

/// Ya se borró su PDF: queda `vencido`. Si se había impreso, `FechaImpreso` y `PedidoUid` lo
/// siguen diciendo.
pub async fn marcar_vencido(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE trabajosimpresion SET estado = 'vencido' WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

// ── Imprimir (autoservicio.rs) ──────────────────────────────────────────────

/// Impresora del autoservicio (`Settings.AUTOSERVICIO_IMPRESORAS`).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Impresora {
    pub clave: String,
    pub nombre: String,
    /// `ipp://…` o `ipps://…`
    pub uri: String,
    /// Producto b/n con el que se cobra lo que sale de esta impresora.
    #[serde(rename = "productoId")]
    pub producto_id: Uuid,
}

pub async fn impresoras(pool: &PgPool) -> Result<Vec<Impresora>, sqlx::Error> {
    let json: Option<String> =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'AUTOSERVICIO_IMPRESORAS'")
            .fetch_optional(pool)
            .await?;
    Ok(json
        .and_then(|j| {
            serde_json::from_str(&j)
                .inspect_err(|e| tracing::error!("AUTOSERVICIO_IMPRESORAS: {e}"))
                .ok()
        })
        .unwrap_or_default())
}

/// Un trabajo que xplaya ya tomó para imprimir.
#[derive(Debug, sqlx::FromRow)]
pub struct PorImprimir {
    pub id: Uuid,
    pub cliente_id: Uuid,
    pub archivo: String,
    pub objeto: String,
    pub paginas_pdf: i32,
    pub paginas: String,
    pub caras: i32,
    pub copias: i32,
    pub doble_cara: bool,
    pub impresora: String,
}

/// Los que la caja autorizó y nadie ha tomado.
pub async fn autorizados(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM trabajosimpresion WHERE estado = 'autorizado' ORDER BY fechaautorizado")
        .fetch_all(pool)
        .await
}

/// Lo toma para imprimir (`autorizado` → `imprimiendo`) en una sola sentencia: si llega dos veces
/// (NATS y la revisión periódica), solo una lo toma y no se imprime doble.
pub async fn tomar(pool: &PgPool, id: Uuid) -> Result<Option<PorImprimir>, sqlx::Error> {
    sqlx::query_as(
        "UPDATE trabajosimpresion SET estado = 'imprimiendo'
          WHERE id = $1 AND estado = 'autorizado' AND impresora IS NOT NULL
          RETURNING id, clienteid AS cliente_id, archivo, objeto, paginaspdf AS paginas_pdf, paginas,
                    caras, copias, doblecara AS doble_cara, impresora",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Salió: crea el pedido para cobrarlo (Origen 2, Autoservicio: producto b/n de la impresora ×
/// caras impresas) y lo marca `impreso`, todo junto. Devuelve el Uid del pedido.
pub async fn terminar(pool: &PgPool, t: &PorImprimir, producto_id: Uuid) -> Result<Uuid, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let item = crate::models::pedido::PedidoItemRequest {
        producto_id,
        cantidad: Decimal::from(t.caras * t.copias),
        presentacion_id: None,
    };
    let pedido = crate::db::pedidos::insertar_autoservicio(&mut tx, t.cliente_id, &item).await?;
    sqlx::query(
        "UPDATE trabajosimpresion
            SET estado = 'impreso', pedidouid = $2, error = NULL,
                fechaimpreso = NOW() AT TIME ZONE 'America/Cancun'
          WHERE id = $1",
    )
    .bind(t.id)
    .bind(pedido)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(pedido)
}

/// No salió: `error` con el motivo para la cajera (puede mandarlo a la otra impresora).
pub async fn fallar(pool: &PgPool, id: Uuid, motivo: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE trabajosimpresion SET estado = 'error', error = $2 WHERE id = $1 AND estado = 'imprimiendo'")
        .bind(id)
        .bind(motivo.chars().take(500).collect::<String>())
        .execute(pool)
        .await
        .map(|_| ())
}

/// Los que se quedaron `imprimiendo` (xplaya se reinició a medias) pasan a `error`, para que la
/// cajera revise si salió antes de mandarlo otra vez. Nunca se reimprimen solos.
pub async fn interrumpidos(pool: &PgPool) -> Result<u64, sqlx::Error> {
    sqlx::query(
        "UPDATE trabajosimpresion
            SET estado = 'error', error = 'Se interrumpió. Revisa si salió antes de mandarlo otra vez.'
          WHERE estado = 'imprimiendo'
            AND fechaautorizado < (NOW() AT TIME ZONE 'America/Cancun') - INTERVAL '20 minutes'",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
}

/// Nombre del cliente, para el registro en Loki.
pub async fn nombre_cliente(pool: &PgPool, cliente_id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT nombre FROM clientes WHERE id = $1")
        .bind(cliente_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}
