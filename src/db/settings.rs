use chrono::Datelike;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::monedero::Terminos;

#[derive(sqlx::FromRow)]
struct SettingRow {
    key: String,
    value: Option<String>,
    lastupdated: Option<chrono::NaiveDateTime>,
}

fn fmt_fecha_es(dt: chrono::NaiveDateTime) -> String {
    const MESES: [&str; 12] = [
        "enero", "febrero", "marzo", "abril", "mayo", "junio",
        "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre",
    ];
    format!("{} de {}, {}", dt.day(), MESES[(dt.month() - 1) as usize], dt.year())
}

pub async fn terminos(pool: &PgPool) -> Result<Terminos, sqlx::Error> {
    let rows = sqlx::query_as::<_, SettingRow>(
        "SELECT key, value, lastupdated FROM settings \
         WHERE key IN ('DIAS_VIGENCIA_MONEDERO', 'TIPO_CAMBIO_MONEDERO')",
    )
    .fetch_all(pool)
    .await?;

    let mut dias = 90i32;
    let mut cashback = Decimal::new(2, 2); // 0.02
    let mut last_updated: Option<chrono::NaiveDateTime> = None;

    for row in rows {
        match row.key.as_str() {
            "DIAS_VIGENCIA_MONEDERO" => {
                if let Some(v) = &row.value {
                    if let Ok(d) = v.parse::<i32>() {
                        dias = d;
                    }
                }
            }
            "TIPO_CAMBIO_MONEDERO" => {
                if let Some(v) = &row.value {
                    if let Ok(d) = v.parse::<Decimal>() {
                        cashback = d;
                    }
                }
            }
            _ => {}
        }
        if let Some(lu) = row.lastupdated {
            last_updated = Some(last_updated.map(|prev| prev.max(lu)).unwrap_or(lu));
        }
    }

    let pct = (cashback * Decimal::new(100, 0)).round();
    let porcentaje = format!("{}%", pct);
    let fecha_actualizacion = last_updated
        .map(fmt_fecha_es)
        .unwrap_or_else(|| "—".to_string());

    Ok(Terminos { dias_vigencia: dias, porcentaje, fecha_actualizacion })
}
