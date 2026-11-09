//! Celulares ligados a un monedero (`ClienteCelulares`) y sus sesiones (`CelularSesiones`).
//!
//! Un cliente tiene a lo más un celular activo (`FechaDesligado IS NULL`, lo asegura un
//! índice único parcial). Ligar uno nuevo desliga el anterior con motivo `reemplazado`.
//! El POS podrá desligarlo a mano (motivo `pos`); las sesiones de un celular desligado
//! dejan de servir aunque no hayan vencido.

use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;
use webauthn_rs::prelude::Passkey;

/// Las sesiones duran un año: es una tarjeta de lealtad, no un banco, y volver a entrar
/// es tocar la huella.
const DIAS_SESION: i32 = 365;

/// Liga la passkey al cliente, reemplazando el celular que tuviera. Va dentro de la
/// transacción del alta.
pub async fn ligar(
    tx: &mut PgConnection,
    celular_id: Uuid,
    cliente_id: Uuid,
    passkey: &Passkey,
    origen: &str,
    dispositivo: &str,
) -> Result<(), sqlx::Error> {
    // Las sesiones del celular anterior se borran; la fila queda como historial.
    sqlx::query(
        "DELETE FROM celularsesiones WHERE celularid IN
           (SELECT id FROM clientecelulares WHERE clienteid = $1 AND fechadesligado IS NULL)",
    )
    .bind(cliente_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE clientecelulares SET fechadesligado = NOW(), motivodesligado = 'reemplazado'
          WHERE clienteid = $1 AND fechadesligado IS NULL",
    )
    .bind(cliente_id)
    .execute(&mut *tx)
    .await?;

    let json = serde_json::to_string(passkey).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
    sqlx::query(
        "INSERT INTO clientecelulares (id, clienteid, credentialid, passkey, origen, dispositivo)
         VALUES ($1, $2, $3, $4::jsonb, $5, $6)",
    )
    .bind(celular_id)
    .bind(cliente_id)
    .bind(AsRef::<[u8]>::as_ref(passkey.cred_id()))
    .bind(json)
    .bind(origen)
    .bind(dispositivo)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Celular activo dueño de esta passkey: `(celular_id, passkey)`.
pub async fn por_credencial(
    pool: &PgPool,
    credential_id: &[u8],
) -> Result<Option<(Uuid, Passkey)>, sqlx::Error> {
    let fila: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, passkey::text FROM clientecelulares
          WHERE credentialid = $1 AND fechadesligado IS NULL",
    )
    .bind(credential_id)
    .fetch_optional(pool)
    .await?;

    let Some((id, json)) = fila else { return Ok(None) };
    let passkey = serde_json::from_str(&json).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    Ok(Some((id, passkey)))
}

/// Después de entrar: guarda el contador de la passkey (si cambió) y la fecha de uso.
pub async fn registrar_uso(
    pool: &PgPool,
    celular_id: Uuid,
    passkey: Option<&Passkey>,
) -> Result<(), sqlx::Error> {
    let json = passkey
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
    sqlx::query(
        "UPDATE clientecelulares
            SET fechaultimouso = NOW(), passkey = COALESCE($2::jsonb, passkey)
          WHERE id = $1",
    )
    .bind(celular_id)
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Sesiones ────────────────────────────────────────────────────────────────

/// Crea una sesión y devuelve el token para la cookie. En la BD solo queda su hash:
/// quien lea la tabla no puede hacerse pasar por el cliente.
pub async fn crear_sesion(tx: &mut PgConnection, celular_id: Uuid) -> Result<String, sqlx::Error> {
    let token = hex(&rand::random::<[u8; 32]>());
    sqlx::query(
        "INSERT INTO celularsesiones (tokenhash, celularid, expira)
         VALUES ($1, $2, NOW() + make_interval(days => $3))",
    )
    .bind(hash(&token))
    .bind(celular_id)
    .bind(DIAS_SESION)
    .execute(&mut *tx)
    .await?;
    Ok(token)
}

/// Cliente de la sesión, si el token existe, no venció y su celular sigue ligado.
pub async fn cliente_de_sesion(pool: &PgPool, token: &str) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT c.clienteid
           FROM celularsesiones s JOIN clientecelulares c ON c.id = s.celularid
          WHERE s.tokenhash = $1 AND s.expira > NOW() AND c.fechadesligado IS NULL",
    )
    .bind(hash(token))
    .fetch_optional(pool)
    .await
}

pub async fn cerrar_sesion(pool: &PgPool, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM celularsesiones WHERE tokenhash = $1")
        .bind(hash(token))
        .execute(pool)
        .await?;
    Ok(())
}

fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
