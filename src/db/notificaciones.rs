//! Tabla `Notificaciones` (bandeja de salida de avisos de la app; se define en el repo del POS).
//!
//! Quien quiere avisar (el POS) inserta una fila `pendiente` y publica su Id en NATS; `push.rs`
//! la toma, la manda por Web Push al celular ligado del cliente y la marca.

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// Lo que se manda al celular.
pub struct Pendiente {
    pub cliente_id: Uuid,
    pub titulo: String,
    pub texto: String,
    pub url: Option<String>,
}

/// Celular ligado del cliente con notificaciones activas.
pub struct Destino {
    pub celular_id: Uuid,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

/// Toma la notificación si sigue `pendiente`, bloqueando su fila hasta el fin de la
/// transacción. `SKIP LOCKED`: si otra tarea ya la está mandando (NATS y la revisión
/// periódica pueden coincidir), aquí sale `None` en lugar de esperar y mandarla dos veces.
pub async fn tomar(tx: &mut PgConnection, id: Uuid) -> Result<Option<Pendiente>, sqlx::Error> {
    let fila: Option<(Uuid, String, String, Option<String>)> = sqlx::query_as(
        "SELECT clienteid, titulo, texto, url FROM notificaciones
          WHERE id = $1 AND estado = 'pendiente'
          FOR UPDATE SKIP LOCKED",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    Ok(fila.map(|(cliente_id, titulo, texto, url)| Pendiente { cliente_id, titulo, texto, url }))
}

pub async fn destino(tx: &mut PgConnection, cliente_id: Uuid) -> Result<Option<Destino>, sqlx::Error> {
    let fila: Option<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT id, pushendpoint, pushp256dh, pushauth FROM clientecelulares
          WHERE clienteid = $1 AND fechadesligado IS NULL AND pushendpoint IS NOT NULL",
    )
    .bind(cliente_id)
    .fetch_optional(&mut *tx)
    .await?;
    Ok(fila.map(|(celular_id, endpoint, p256dh, auth)| Destino { celular_id, endpoint, p256dh, auth }))
}

/// Estado final: `enviada`, `sin_app` o `fallida` (con el error).
pub async fn marcar(
    tx: &mut PgConnection,
    id: Uuid,
    estado: &str,
    celular_id: Option<Uuid>,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE notificaciones
            SET estado = $2, celularid = $3, error = LEFT($4, 300),
                fechaenviado = CASE WHEN $2 = 'enviada' THEN NOW() END
          WHERE id = $1",
    )
    .bind(id)
    .bind(estado)
    .bind(celular_id)
    .bind(error)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// El servicio de push dijo que la suscripción ya no existe (desinstaló la app o quitó el
/// permiso): se borra para no volver a intentarlo. La app la rehace si vuelve a aceptar.
pub async fn quitar_push(tx: &mut PgConnection, celular_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE clientecelulares
            SET pushendpoint = NULL, pushp256dh = NULL, pushauth = NULL, fechapush = NULL
          WHERE id = $1",
    )
    .bind(celular_id)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Pendientes que NATS no entregó (estaba caído cuando el POS publicó): de más de un minuto,
/// para no adelantarse a NATS, y de menos de un día.
pub async fn rezagadas(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM notificaciones
          WHERE estado = 'pendiente'
            AND fechacreado < NOW() - INTERVAL '1 minute'
            AND fechacreado > NOW() - INTERVAL '1 day'
          ORDER BY fechacreado
          LIMIT 100",
    )
    .fetch_all(pool)
    .await
}

/// Lo que lleva más de un día pendiente ya no vale la pena mandarlo ("tu pedido está listo"
/// de antier confunde más de lo que ayuda).
pub async fn vencer(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE notificaciones SET estado = 'fallida', error = 'No se pudo entregar en un día'
          WHERE estado = 'pendiente' AND fechacreado <= NOW() - INTERVAL '1 day'",
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// El service worker avisa que el aviso llegó al celular. Solo la primera vez cuenta.
pub async fn marcar_recibida(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE notificaciones SET fecharecibido = COALESCE(fecharecibido, NOW()) WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// El cliente tocó el aviso. Si el aviso de "recibida" se perdió, abrirla también lo prueba.
pub async fn marcar_abierta(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE notificaciones
            SET fechaabierto = COALESCE(fechaabierto, NOW()),
                fecharecibido = COALESCE(fecharecibido, NOW())
          WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
