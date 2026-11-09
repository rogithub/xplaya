//! Enlace de alta en el recibo (`/alta/recibo/{venta}` en la app).
//!
//! El enlace no tiene tabla de tokens: está vivo si la venta tiene menos de 24 horas y
//! no tiene un resultado en `AltasRecibo`. Se gasta con el primer resultado (`ligado`,
//! `abonado` o `bloqueado`), y ahí también se cuentan los intentos fallidos de los
//! últimos 4 dígitos del teléfono. Diseño completo en `APP_MONEDERO.md`.

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// Intentos fallidos de los 4 dígitos antes de bloquear el enlace.
pub const MAX_INTENTOS: i32 = 5;

/// Venta con el enlace de alta vivo.
#[derive(sqlx::FromRow)]
pub struct VentaAlta {
    pub cliente_id: Option<Uuid>,
    /// El cliente de la venta ya tiene un celular ligado (el enlace serviría para cambiarlo).
    pub tiene_celular: bool,
}

/// `Some` si el enlace de alta de esta venta sigue vivo.
pub async fn venta_disponible(pool: &PgPool, ajuste_id: Uuid) -> Result<Option<VentaAlta>, sqlx::Error> {
    // FechaAjuste está en la hora de la tienda (el POS corre en America/Cancun); la BD, en UTC.
    sqlx::query_as(
        "SELECT a.clienteid AS cliente_id,
                EXISTS (SELECT 1 FROM clientecelulares c
                         WHERE c.clienteid = a.clienteid AND c.fechadesligado IS NULL) AS tiene_celular
           FROM ajustes a
           LEFT JOIN altasrecibo ar ON ar.ajusteid = a.id
          WHERE a.id = $1 AND a.tipoajuste = 0
            AND a.fechaajuste > (NOW() AT TIME ZONE 'America/Cancun') - INTERVAL '24 hours'
            AND ar.resultado IS NULL",
    )
    .bind(ajuste_id)
    .fetch_optional(pool)
    .await
}

pub enum Intento {
    Correcto,
    Incorrecto { quedan: i32 },
    Bloqueado,
}

/// Compara los últimos 4 dígitos del teléfono del cliente. Cada fallo suma un intento;
/// al llegar a `MAX_INTENTOS` el enlace queda `bloqueado` para siempre.
pub async fn revisar_digitos(
    pool: &PgPool,
    ajuste_id: Uuid,
    cliente_id: Uuid,
    digitos: &str,
) -> Result<Intento, sqlx::Error> {
    let correcto: bool = sqlx::query_scalar(
        "SELECT right(telefono, 4) = $2 FROM clientes WHERE id = $1",
    )
    .bind(cliente_id)
    .bind(digitos)
    .fetch_optional(pool)
    .await?
    .unwrap_or(false);
    if correcto {
        return Ok(Intento::Correcto);
    }

    // Suma el intento solo si el enlace sigue sin resultado (si ya se gastó, no devuelve fila).
    let intentos: Option<i32> = sqlx::query_scalar(
        "INSERT INTO altasrecibo (ajusteid, intentos) VALUES ($1, 1)
         ON CONFLICT (ajusteid) DO UPDATE SET intentos = altasrecibo.intentos + 1
          WHERE altasrecibo.resultado IS NULL
         RETURNING intentos",
    )
    .bind(ajuste_id)
    .fetch_optional(pool)
    .await?;

    match intentos {
        Some(n) if n < MAX_INTENTOS => Ok(Intento::Incorrecto { quedan: MAX_INTENTOS - n }),
        Some(_) => {
            sqlx::query("UPDATE altasrecibo SET resultado = 'bloqueado' WHERE ajusteid = $1")
                .bind(ajuste_id)
                .execute(pool)
                .await?;
            Ok(Intento::Bloqueado)
        }
        None => Ok(Intento::Bloqueado),
    }
}

/// Gasta el enlace con `resultado`. `false` si alguien más lo gastó primero: el que
/// llama debe deshacer su transacción.
pub async fn gastar(
    tx: &mut PgConnection,
    ajuste_id: Uuid,
    resultado: &str,
    cliente_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let fila = sqlx::query(
        "INSERT INTO altasrecibo (ajusteid, resultado, clienteid) VALUES ($1, $2, $3)
         ON CONFLICT (ajusteid) DO UPDATE SET resultado = EXCLUDED.resultado, clienteid = EXCLUDED.clienteid
          WHERE altasrecibo.resultado IS NULL",
    )
    .bind(ajuste_id)
    .bind(resultado)
    .bind(cliente_id)
    .execute(&mut *tx)
    .await?;
    Ok(fila.rows_affected() == 1)
}

/// Pone el cliente a una venta sin cliente y le genera su monedero, con la misma función
/// que usa el POS (`fn_asignar_cliente_venta`, en `reportes.sql` del POS). `false` si la
/// venta ya tenía cliente (por ejemplo, la cajera lo asignó mientras tanto).
pub async fn abonar_venta(
    tx: &mut PgConnection,
    ajuste_id: Uuid,
    cliente_id: Uuid,
) -> Result<bool, sqlx::Error> {
    // Usuario de las operaciones de xplaya.com (el mismo de los pedidos en línea).
    let usuario: Option<Uuid> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'ID_XPLAYA.COM_ANONYMOUS_USER'",
    )
    .fetch_optional(&mut *tx)
    .await?
    .and_then(|v| Uuid::parse_str(&v).ok());

    sqlx::query_scalar("SELECT fn_asignar_cliente_venta($1, $2, $3)")
        .bind(ajuste_id)
        .bind(cliente_id)
        .bind(usuario)
        .fetch_one(&mut *tx)
        .await
}

/// Cliente con ese teléfono (últimos 10 dígitos), haya aceptado el programa o no.
pub async fn cliente_por_telefono(tx: &mut PgConnection, tel: &str) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM clientes WHERE right(telefono, 10) = $1 LIMIT 1")
        .bind(tel)
        .fetch_optional(&mut *tx)
        .await
}

pub async fn nombre_cliente(pool: &PgPool, cliente_id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT nombre FROM clientes WHERE id = $1")
        .bind(cliente_id)
        .fetch_optional(pool)
        .await
}

/// Cliente nuevo que ya aceptó el programa. Las fechas van en hora de la tienda, como
/// las escribe el POS.
pub async fn crear_cliente(tx: &mut PgConnection, nombre: &str, tel: &str) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO clientes (id, telefono, nombre, fechacreado, aceptoprograma, fechaaceptoprograma)
         VALUES (gen_random_uuid(), $1, $2, NOW() AT TIME ZONE 'America/Cancun', true,
                 NOW() AT TIME ZONE 'America/Cancun')
         RETURNING id",
    )
    .bind(tel)
    .bind(nombre)
    .fetch_one(&mut *tx)
    .await
}

/// El cliente aceptó los términos al ligar su celular: es el "quiero participar".
pub async fn aceptar_programa(tx: &mut PgConnection, cliente_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE clientes
            SET aceptoprograma = true,
                fechaaceptoprograma = COALESCE(fechaaceptoprograma, NOW() AT TIME ZONE 'America/Cancun')
          WHERE id = $1",
    )
    .bind(cliente_id)
    .execute(&mut *tx)
    .await?;
    Ok(())
}
