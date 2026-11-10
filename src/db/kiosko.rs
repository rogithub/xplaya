//! Solicitudes del kiosko de la tienda (`KioskoSolicitudes`, definida en el repo del POS).
//!
//! El cliente escribe nombre y teléfono; el POS muestra el código de 4 dígitos y la caja se lo
//! manda por WhatsApp a ese teléfono. Si lo escribe bien, el teléfono es suyo. La solicitud vive
//! 15 minutos y se bloquea al 5.º código equivocado.

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub const MAX_INTENTOS: i32 = 5;

/// Condición de "sigue viva": la usan todas las consultas.
const VIGENTE: &str = "fechacreado > NOW() - INTERVAL '15 minutes'";

pub async fn crear(pool: &PgPool, nombre: &str, telefono: &str) -> Result<Uuid, sqlx::Error> {
    let codigo = format!("{:04}", rand::random_range(0..10_000u32));
    sqlx::query_scalar(
        "INSERT INTO kioskosolicitudes (nombre, telefono, codigo) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(nombre)
    .bind(telefono)
    .bind(codigo)
    .fetch_one(pool)
    .await
}

/// Teléfono de la solicitud si sigue esperando el código.
pub async fn pendiente(pool: &PgPool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT telefono FROM kioskosolicitudes WHERE id = $1 AND estado = 'pendiente' AND {VIGENTE}"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub enum Revision {
    /// Código correcto: nombre que escribió y teléfono comprobado.
    Correcta { nombre: String, telefono: String },
    Incorrecta { quedan: i32 },
    Bloqueada,
    /// Vencida, ya usada o no existe.
    NoDisponible,
}

pub async fn revisar_codigo(pool: &PgPool, id: Uuid, codigo: &str) -> Result<Revision, sqlx::Error> {
    let correcta: Option<(String, String)> = sqlx::query_as(&format!(
        "UPDATE kioskosolicitudes SET estado = 'verificada', fechaverificado = NOW()
          WHERE id = $1 AND estado = 'pendiente' AND {VIGENTE} AND codigo = $2
          RETURNING nombre, telefono"
    ))
    .bind(id)
    .bind(codigo)
    .fetch_optional(pool)
    .await?;
    if let Some((nombre, telefono)) = correcta {
        return Ok(Revision::Correcta { nombre, telefono });
    }

    let intentos: Option<i32> = sqlx::query_scalar(&format!(
        "UPDATE kioskosolicitudes
            SET intentos = intentos + 1,
                estado = CASE WHEN intentos + 1 >= {MAX_INTENTOS} THEN 'bloqueada' ELSE estado END
          WHERE id = $1 AND estado = 'pendiente' AND {VIGENTE}
          RETURNING intentos"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(match intentos {
        Some(n) if n < MAX_INTENTOS => Revision::Incorrecta { quedan: MAX_INTENTOS - n },
        Some(_) => Revision::Bloqueada,
        None => Revision::NoDisponible,
    })
}

/// Cierra la solicitud verificada con el cliente que quedó. `false` si ya se había cerrado:
/// quien llama deshace su transacción.
pub async fn completar(tx: &mut PgConnection, id: Uuid, cliente_id: Uuid) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE kioskosolicitudes SET estado = 'completada', clienteid = $2
          WHERE id = $1 AND estado = 'verificada'",
    )
    .bind(id)
    .bind(cliente_id)
    .execute(&mut *tx)
    .await?;
    Ok(r.rows_affected() == 1)
}
