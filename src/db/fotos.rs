//! Tabla `PedidosFotos` (definida en el repo del POS): los pedidos de fotos que el cliente hizo
//! desde la app y en qué van. El pedido de verdad vive en imagina; esto es lo que la app le
//! muestra al cliente, al día por NATS (`fotos.rs`). Fechas en la hora de la tienda.

use sqlx::PgPool;
use uuid::Uuid;

const AHORA: &str = "(NOW() AT TIME ZONE 'America/Cancun')";

/// Guarda el nuevo estado del pedido (lo crea si no estaba). Devuelve el estado anterior.
pub async fn guardar(pool: &PgPool, codigo: &str, cliente_id: Uuid, tipo: &str, estado: &str) -> Result<Option<String>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let anterior: Option<String> = sqlx::query_scalar("SELECT estado FROM pedidosfotos WHERE codigo = $1 FOR UPDATE")
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await?;
    sqlx::query(&format!(
        "INSERT INTO pedidosfotos (codigo, clienteid, tipo, estado, fechacreado, fechacambio)
         VALUES ($1, $2, $3, $4, {AHORA}, {AHORA})
         ON CONFLICT (codigo) DO UPDATE SET estado = EXCLUDED.estado, fechacambio = EXCLUDED.fechacambio"
    ))
    .bind(codigo)
    .bind(cliente_id)
    .bind(tipo)
    .bind(estado)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(anterior)
}

/// Nombre y teléfono del cliente, para el permiso que lleva a imagina.
pub async fn datos_cliente(pool: &PgPool, cliente_id: Uuid) -> Result<Option<(Option<String>, String)>, sqlx::Error> {
    sqlx::query_as("SELECT nombre, telefono FROM clientes WHERE id = $1")
        .bind(cliente_id)
        .fetch_optional(pool)
        .await
}

/// Un pedido de fotos como lo ve el cliente en su app.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct PedidoFoto {
    pub codigo: String,
    pub tipo: String,
    pub estado: String,
    pub fechacreado: chrono::NaiveDateTime,
    pub fechacambio: chrono::NaiveDateTime,
}

/// Los pedidos de fotos del cliente de los últimos 30 días, el más nuevo arriba.
pub async fn de_cliente(pool: &PgPool, cliente_id: Uuid) -> Result<Vec<PedidoFoto>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT codigo, tipo, estado, fechacreado, fechacambio FROM pedidosfotos
          WHERE clienteid = $1 AND fechacreado > {AHORA} - INTERVAL '30 days'
          ORDER BY fechacreado DESC"
    ))
    .bind(cliente_id)
    .fetch_all(pool)
    .await
}
