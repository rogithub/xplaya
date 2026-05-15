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
