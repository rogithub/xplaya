//! Tabla `Mensajes` del chat (definida en el repo del POS; diseño en `CHAT.md`). Solo xplaya
//! escribe aquí, desde el stream `CHAT` (`chat/consumer.rs`). Fechas en la hora de la tienda.
//!
//! Solo ejecuta: lo que se guarda y cuándo lo decide `chat::reglas`.

use chrono::NaiveDateTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::chat::contrato::{Adjunto, Lado};
use crate::chat::reglas::{Ajustes, Guardado, Nuevo};

/// Cliente, autor, fecha, si se eliminó, adjuntos y secuencia.
type FilaGuardado = (Uuid, String, NaiveDateTime, bool, Option<serde_json::Value>, Option<i64>);

/// Lo que hay guardado de un mensaje, para que las reglas decidan.
pub async fn guardado(pool: &PgPool, id: Uuid) -> Result<Option<Guardado>, sqlx::Error> {
    let fila: Option<FilaGuardado> = sqlx::query_as(
        "SELECT clienteid, autor, fecha, fechaeliminado IS NOT NULL, adjuntos, secuencia
         FROM mensajes WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.and_then(|(cliente, autor, fecha, eliminado, adjuntos, secuencia)| {
        Some(Guardado {
            cliente,
            // El CHECK de la tabla solo deja los cuatro lados.
            autor: Lado::leer(&autor)?,
            fecha,
            eliminado,
            adjuntos: adjuntos.and_then(|a| serde_json::from_value::<Vec<Adjunto>>(a).ok()).unwrap_or_default(),
            secuencia: secuencia.map(|s| s as u64),
        })
    }))
}

/// Guarda un mensaje nuevo. Devuelve `false` si no se guardó: ya estaba (NATS lo dio dos veces) o
/// el cliente no existe. Si cita un mensaje que no es de este cliente, se guarda sin la cita.
pub async fn guardar(pool: &PgPool, n: &Nuevo) -> Result<bool, sqlx::Error> {
    let adjuntos = (!n.adjuntos.is_empty()).then(|| serde_json::to_value(&n.adjuntos)).transpose().map_err(|e| sqlx::Error::Encode(Box::new(e)))?;
    let hecho = sqlx::query(
        "INSERT INTO mensajes (id, clienteid, autor, tipo, texto, adjuntos, ref, respondea, secuencia, fecha)
         SELECT $1, c.id, $3, $4, $5, $6, $7,
                (SELECT m.id FROM mensajes m WHERE m.id = $8 AND m.clienteid = c.id),
                $9, $10
         FROM clientes c WHERE c.id = $2
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(n.id)
    .bind(n.cliente)
    .bind(n.autor.texto())
    .bind(n.tipo.texto())
    .bind(&n.texto)
    .bind(adjuntos)
    .bind(&n.referencia)
    .bind(n.responde_a)
    .bind(n.secuencia as i64)
    .bind(n.fecha)
    .execute(pool)
    .await?;
    Ok(hecho.rows_affected() == 1)
}

/// Marca leídos los mensajes del cliente (`del_cliente`) o los que le escribieron, hasta `hasta`.
/// Devuelve cuántos.
pub async fn marcar_leidos(pool: &PgPool, cliente: Uuid, del_cliente: bool, hasta: NaiveDateTime) -> Result<u64, sqlx::Error> {
    let hecho = sqlx::query(
        "UPDATE mensajes SET fechaleido = $3
         WHERE clienteid = $1 AND (autor = 'cliente') = $2
           AND fecha <= $3 AND fechaleido IS NULL AND fechaeliminado IS NULL",
    )
    .bind(cliente)
    .bind(del_cliente)
    .bind(hasta)
    .execute(pool)
    .await?;
    Ok(hecho.rows_affected())
}

pub async fn editar(pool: &PgPool, id: Uuid, texto: &str, fecha: NaiveDateTime) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE mensajes SET texto = $2, fechaeditado = $3 WHERE id = $1 AND fechaeliminado IS NULL")
        .bind(id)
        .bind(texto)
        .bind(fecha)
        .execute(pool)
        .await?;
    Ok(())
}

/// "Eliminar para todos": se queda la fila (para que la conversación tenga sentido) sin el texto
/// ni los archivos.
pub async fn eliminar(pool: &PgPool, id: Uuid, fecha: NaiveDateTime) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE mensajes SET texto = NULL, adjuntos = NULL, fechaeliminado = $2 WHERE id = $1")
        .bind(id)
        .bind(fecha)
        .execute(pool)
        .await?;
    Ok(())
}

/// Los ajustes `CHAT_*` de `Settings`. Si falta uno o no es número, queda el de siempre.
pub async fn ajustes(pool: &PgPool) -> Result<Ajustes, sqlx::Error> {
    let minutos: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'CHAT_MINUTOS_EDITAR'")
        .fetch_optional(pool)
        .await?
        .flatten();
    let mut a = Ajustes::default();
    if let Some(m) = minutos.and_then(|m| m.trim().parse().ok()) {
        a.minutos_editar = m;
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::contrato::Tipo;

    /// Carga el schema real del POS en la base temporal de la prueba, para no tener una copia que
    /// se desfase. `false` si no está el repo del POS (la prueba se salta).
    async fn con_schema(pool: &PgPool) -> bool {
        let ruta = std::env::var("POS_SCHEMA").unwrap_or_else(|_| {
            format!("{}/../inventario_papeleria/dbscripts/postgresql_inventario.sql", env!("CARGO_MANIFEST_DIR"))
        });
        let Ok(sql) = std::fs::read_to_string(&ruta) else {
            eprintln!("Sin el schema del POS en {ruta}: se salta la prueba (POS_SCHEMA cambia la ruta)");
            return false;
        };
        sqlx::raw_sql(&sql).execute(pool).await.expect("cargar el schema del POS");
        true
    }

    async fn cliente(pool: &PgPool, telefono: &str) -> Uuid {
        sqlx::query_scalar("INSERT INTO clientes (telefono, nombre, fechacreado) VALUES ($1, 'Prueba', NOW()) RETURNING id")
            .bind(telefono)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn hora(t: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(&format!("2026-11-19 {t}"), "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn nuevo(cliente: Uuid, autor: Lado, a_las: &str) -> Nuevo {
        Nuevo {
            id: Uuid::new_v4(),
            cliente,
            autor,
            tipo: Tipo::Texto,
            texto: Some("Hola".into()),
            adjuntos: vec![Adjunto {
                nombre: "a.pdf".into(),
                mime: "application/pdf".into(),
                bytes: 1,
                llave: format!("{cliente}/x/a.pdf"),
                paginas: Some(1),
                pdf: None,
                miniatura: None,
            }],
            referencia: None,
            responde_a: None,
            secuencia: 5,
            fecha: hora(a_las),
        }
    }

    #[sqlx::test(migrations = false)]
    async fn guardar_y_leer_lo_guardado(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        let c = cliente(&pool, "9840000001").await;
        let n = nuevo(c, Lado::Cliente, "17:42:00");
        assert!(guardar(&pool, &n).await.unwrap());
        assert!(!guardar(&pool, &n).await.unwrap(), "repetido: no se guarda dos veces");
        let g = guardado(&pool, n.id).await.unwrap().unwrap();
        assert_eq!((g.cliente, g.autor, g.fecha, g.eliminado, g.secuencia), (c, Lado::Cliente, n.fecha, false, Some(5)));
        assert_eq!(g.adjuntos, n.adjuntos);
        assert_eq!(guardado(&pool, Uuid::new_v4()).await.unwrap(), None);
    }

    #[sqlx::test(migrations = false)]
    async fn un_cliente_que_no_existe_no_rompe_nada(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        assert!(!guardar(&pool, &nuevo(Uuid::new_v4(), Lado::Cliente, "17:42:00")).await.unwrap());
    }

    #[sqlx::test(migrations = false)]
    async fn la_cita_solo_vale_dentro_de_la_conversacion(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        let (c, otro) = (cliente(&pool, "9840000001").await, cliente(&pool, "9840000002").await);
        let de_otro = nuevo(otro, Lado::Cliente, "17:00:00");
        let propio = nuevo(c, Lado::Cliente, "17:01:00");
        guardar(&pool, &de_otro).await.unwrap();
        guardar(&pool, &propio).await.unwrap();
        let mut cita_propia = nuevo(c, Lado::Papeleria, "17:02:00");
        cita_propia.responde_a = Some(propio.id);
        let mut cita_ajena = nuevo(c, Lado::Papeleria, "17:03:00");
        cita_ajena.responde_a = Some(de_otro.id);
        assert!(guardar(&pool, &cita_propia).await.unwrap());
        assert!(guardar(&pool, &cita_ajena).await.unwrap(), "se guarda, pero sin la cita");
        let cita = |id| sqlx::query_scalar::<_, Option<Uuid>>("SELECT respondea FROM mensajes WHERE id = $1").bind(id).fetch_one(&pool);
        assert_eq!(cita(cita_propia.id).await.unwrap(), Some(propio.id));
        assert_eq!(cita(cita_ajena.id).await.unwrap(), None);
    }

    #[sqlx::test(migrations = false)]
    async fn leidos_del_otro_lado_hasta_la_hora(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        let c = cliente(&pool, "9840000001").await;
        for (autor, a_las) in [(Lado::Cliente, "17:00:00"), (Lado::Cliente, "17:10:00"), (Lado::Cliente, "17:30:00"), (Lado::Papeleria, "17:05:00")] {
            guardar(&pool, &nuevo(c, autor, a_las)).await.unwrap();
        }
        // La papelería leyó a las 17:20: los dos primeros del cliente.
        assert_eq!(marcar_leidos(&pool, c, true, hora("17:20:00")).await.unwrap(), 2);
        assert_eq!(marcar_leidos(&pool, c, true, hora("17:20:00")).await.unwrap(), 0, "ya estaban leídos");
        // El cliente leyó lo de la papelería.
        assert_eq!(marcar_leidos(&pool, c, false, hora("17:20:00")).await.unwrap(), 1);
    }

    #[sqlx::test(migrations = false)]
    async fn editar_y_eliminar(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        let c = cliente(&pool, "9840000001").await;
        let n = nuevo(c, Lado::Cliente, "17:42:00");
        guardar(&pool, &n).await.unwrap();
        editar(&pool, n.id, "Son 2 juegos", hora("17:50:00")).await.unwrap();
        let (texto, editado): (Option<String>, Option<NaiveDateTime>) =
            sqlx::query_as("SELECT texto, fechaeditado FROM mensajes WHERE id = $1").bind(n.id).fetch_one(&pool).await.unwrap();
        assert_eq!((texto.as_deref(), editado), (Some("Son 2 juegos"), Some(hora("17:50:00"))));

        eliminar(&pool, n.id, hora("18:00:00")).await.unwrap();
        let (texto, adjuntos): (Option<String>, Option<serde_json::Value>) =
            sqlx::query_as("SELECT texto, adjuntos FROM mensajes WHERE id = $1").bind(n.id).fetch_one(&pool).await.unwrap();
        assert_eq!((texto, adjuntos), (None, None));
        assert!(guardado(&pool, n.id).await.unwrap().unwrap().eliminado);
    }

    #[sqlx::test(migrations = false)]
    async fn ajustes_de_settings(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        // El schema base no trae los Settings (van en inserts.sql): sin la fila, el de siempre.
        assert_eq!(ajustes(&pool).await.unwrap(), Ajustes::default());
        sqlx::query("INSERT INTO settings (key, value) VALUES ('CHAT_MINUTOS_EDITAR', '30')").execute(&pool).await.unwrap();
        assert_eq!(ajustes(&pool).await.unwrap().minutos_editar, 30);
        sqlx::query("UPDATE settings SET value = 'mucho' WHERE key = 'CHAT_MINUTOS_EDITAR'").execute(&pool).await.unwrap();
        assert_eq!(ajustes(&pool).await.unwrap(), Ajustes::default());
    }
}
