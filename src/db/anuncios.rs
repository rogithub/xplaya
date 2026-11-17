//! Tabla `Anuncios` (definida en el repo del POS): un anuncio del kiosko por cliente. xplaya lo
//! crea, lo cambia y lo quita (el cliente); la caja lo aprueba o no en el POS. Fechas en la hora
//! de la tienda, como `TrabajosImpresion`.

use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// Ahora en la tienda.
const AHORA: &str = "(NOW() AT TIME ZONE 'America/Cancun')";

/// Rechazados y vencidos se guardan este tiempo (por si el cliente lo vuelve a mandar); luego se
/// borran con su imagen.
pub const DIAS_GUARDADO: i32 = 30;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Anuncio {
    pub id: Uuid,
    pub tipo: String,
    pub objeto: Option<String>,
    pub texto: Option<String>,
    pub color: Option<String>,
    pub estado: String,
    pub motivo: Option<String>,
    pub fechaenviado: NaiveDateTime,
    pub fechavence: Option<NaiveDateTime>,
}

pub async fn de_cliente(pool: &PgPool, cliente_id: Uuid) -> Result<Option<Anuncio>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, tipo, objeto, texto, color, estado, motivo, fechaenviado, fechavence
           FROM anuncios WHERE clienteid = $1",
    )
    .bind(cliente_id)
    .fetch_optional(pool)
    .await
}

/// Lo que el cliente manda: imagen (llave en MinIO) o texto con su color.
pub enum Contenido<'a> {
    Imagen { objeto: &'a str },
    Texto { texto: &'a str, color: &'a str },
}

/// Pone o reemplaza su anuncio; queda por aprobar. Devuelve la imagen anterior, si había, para
/// borrarla de MinIO.
pub async fn guardar(pool: &PgPool, cliente_id: Uuid, contenido: &Contenido<'_>) -> Result<Option<String>, sqlx::Error> {
    let (tipo, objeto, texto, color) = match contenido {
        Contenido::Imagen { objeto } => ("imagen", Some(*objeto), None, None),
        Contenido::Texto { texto, color } => ("texto", None, Some(*texto), Some(*color)),
    };
    let mut tx = pool.begin().await?;
    let anterior: Option<Option<String>> =
        sqlx::query_scalar("SELECT objeto FROM anuncios WHERE clienteid = $1 FOR UPDATE")
            .bind(cliente_id)
            .fetch_optional(&mut *tx)
            .await?;
    sqlx::query(&format!(
        "INSERT INTO anuncios (clienteid, tipo, objeto, texto, color, estado, fechaenviado)
         VALUES ($1, $2, $3, $4, $5, 'por_aprobar', {AHORA})
         ON CONFLICT (clienteid) DO UPDATE
            SET tipo = EXCLUDED.tipo, objeto = EXCLUDED.objeto, texto = EXCLUDED.texto,
                color = EXCLUDED.color, estado = 'por_aprobar', motivo = NULL, aproboid = NULL,
                fechaenviado = EXCLUDED.fechaenviado, fechaaprobado = NULL, fechavence = NULL"
    ))
    .bind(cliente_id)
    .bind(tipo)
    .bind(objeto)
    .bind(texto)
    .bind(color)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    // Si mandó otra imagen, la anterior sobra; si es la misma llave (no pasa: cada subida es
    // nueva), no se borra.
    Ok(anterior.flatten().filter(|o| Some(o.as_str()) != objeto))
}

/// Ya pasaron sus 7 días y lo quiere otra vez, igual: vuelve a esperar a la caja.
pub async fn volver_a_mandar(pool: &PgPool, cliente_id: Uuid) -> Result<u64, sqlx::Error> {
    sqlx::query(&format!(
        "UPDATE anuncios
            SET estado = 'por_aprobar', motivo = NULL, aproboid = NULL, fechaenviado = {AHORA},
                fechaaprobado = NULL, fechavence = NULL
          WHERE clienteid = $1 AND estado = 'aprobado' AND fechavence <= {AHORA}"
    ))
    .bind(cliente_id)
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
}

/// El cliente lo quita (ya rentó, se acabó la promoción). Devuelve su imagen para borrarla.
pub async fn quitar(pool: &PgPool, cliente_id: Uuid) -> Result<Option<Option<String>>, sqlx::Error> {
    sqlx::query_scalar("DELETE FROM anuncios WHERE clienteid = $1 RETURNING objeto")
        .bind(cliente_id)
        .fetch_optional(pool)
        .await
}

/// Un anuncio como lo pasa el kiosko. Nada del cliente: la pantalla es pública.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EnKiosko {
    pub id: Uuid,
    pub tipo: String,
    pub texto: Option<String>,
    pub color: Option<String>,
    /// Cambia con cada anuncio nuevo del cliente: va en la URL de la imagen para la caché.
    pub version: i64,
}

/// Aprobados y vigentes, en el orden en que se aprobaron.
pub async fn en_kiosko(pool: &PgPool) -> Result<Vec<EnKiosko>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT id, tipo, texto, color, extract(epoch FROM fechaenviado)::bigint AS version
           FROM anuncios
          WHERE estado = 'aprobado' AND fechavence > {AHORA}
          ORDER BY fechaaprobado"
    ))
    .fetch_all(pool)
    .await
}

/// La imagen de un anuncio que el kiosko puede pasar (aprobado y vigente).
pub async fn imagen_en_kiosko(pool: &PgPool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT objeto FROM anuncios
          WHERE id = $1 AND tipo = 'imagen' AND estado = 'aprobado' AND fechavence > {AHORA}"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map(Option::flatten)
}

/// Rechazados y vencidos de hace más de `DIAS_GUARDADO` días: (id, imagen).
pub async fn viejos(pool: &PgPool) -> Result<Vec<(Uuid, Option<String>)>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT id, objeto FROM anuncios
          WHERE (estado = 'aprobado' AND fechavence <= {AHORA} - make_interval(days => $1))
             OR (estado = 'rechazado' AND fechaenviado <= {AHORA} - make_interval(days => $1))"
    ))
    .bind(DIAS_GUARDADO)
    .fetch_all(pool)
    .await
}

pub async fn borrar(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM anuncios WHERE id = $1").bind(id).execute(pool).await.map(|_| ())
}

/// Para el aviso a la papelería.
pub async fn nombre_cliente(pool: &PgPool, cliente_id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT nombre FROM clientes WHERE id = $1")
        .bind(cliente_id)
        .fetch_optional(pool)
        .await
        .map(Option::flatten)
}
