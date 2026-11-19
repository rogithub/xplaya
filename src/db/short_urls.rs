use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ShortUrlRow {
    tipo: String,
    targetid: Uuid,
}

pub async fn buscar(pool: &PgPool, code: &str) -> Result<Option<(String, Uuid)>, sqlx::Error> {
    let row = sqlx::query_as::<_, ShortUrlRow>(
        "SELECT tipo, targetid FROM shorturls WHERE code = $1",
    )
    .bind(code)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| (r.tipo, r.targetid)))
}

/// La ruta a la que lleva una URL corta (`/r/{code}`). La usan el redirect y el chat, que
/// convierte un link corto en tarjeta.
pub fn ruta(tipo: &str, id: Uuid) -> Option<String> {
    match tipo {
        "recibo" => Some(format!("/recibo/{id}")),
        "cotizacion" => Some(format!("/cotizacion/{id}")),
        "monedero" => Some(format!("/monedero/{id}")),
        _ => None,
    }
}
