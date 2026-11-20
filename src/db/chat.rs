//! Tabla `Mensajes` del chat (definida en el repo del POS; diseño en `CHAT.md`). Solo xplaya
//! escribe aquí, desde el stream `CHAT` (`chat/consumer.rs`). Fechas en la hora de la tienda.
//!
//! Solo ejecuta: lo que se guarda y cuándo lo decide `chat::reglas`. La app lee de aquí el
//! historial, el bloqueo y lo que no ha leído.

use chrono::NaiveDateTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::chat::contrato::{Adjunto, Lado};
use crate::chat::reglas::{Ajustes, Guardado, Nuevo};
use crate::chat::vista::{adjuntos_vista, Vista};

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

/// "Vaciar chat" en la app: el cliente deja de ver lo de `Fecha <= hasta` (hora de la tienda).
/// Solo de su lado: no se borra nada y la caja sigue viendo todo.
pub async fn vaciar(pool: &PgPool, cliente: Uuid, hasta: NaiveDateTime) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE clientes SET chatvaciado = $2 WHERE id = $1")
        .bind(cliente)
        .bind(hasta)
        .execute(pool)
        .await?;
    Ok(())
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

/// Los ajustes `CHAT_*` y `PAPELERIA_ABIERTA` de `Settings`. Si falta uno o no es número, queda el de siempre.
pub async fn ajustes(pool: &PgPool) -> Result<Ajustes, sqlx::Error> {
    let filas: Vec<(String, Option<String>)> =
        sqlx::query_as(
            "SELECT key, value FROM settings WHERE key IN ('CHAT_MINUTOS_EDITAR', 'CHAT_CLIENTES_PRUEBA', 'CHAT_MB_DOCUMENTO', 'CHAT_MB_VIDEO', 'PAPELERIA_ABIERTA')",
        )
            .fetch_all(pool)
            .await?;
    let mut a = Ajustes::default();
    let numero = |valor: Option<String>| valor.and_then(|v| v.trim().parse::<u64>().ok()).filter(|n| *n > 0);
    for (llave, valor) in filas {
        match llave.as_str() {
            "CHAT_MINUTOS_EDITAR" => {
                if let Some(m) = valor.and_then(|m| m.trim().parse().ok()) {
                    a.minutos_editar = m;
                }
            }
            "CHAT_MB_DOCUMENTO" => a.mb_documento = numero(valor).unwrap_or(a.mb_documento),
            "CHAT_MB_VIDEO" => a.mb_video = numero(valor).unwrap_or(a.mb_video),
            "PAPELERIA_ABIERTA" => a.papeleria_abierta = valor.is_none_or(|v| v.trim() != "0"),
            // Con la fila, aunque el valor sea NULL: vacía quiere decir todos (`chat_visible`).
            _ => a.clientes_prueba = Some(valor.unwrap_or_default()),
        }
    }
    Ok(a)
}

/// Cuándo se cerró la papelería, si el switch de la caja (`Settings.PAPELERIA_ABIERTA`) está en
/// `'0'`; `None` si está abierta o no hay fila. La hora es `LastUpdated`, que la caja pone al
/// mover el switch: separa un cierre del siguiente para el aviso automático
/// (`reglas::aviso_fuera_de_horario`). Sin `LastUpdated`, todos los cierres cuentan como uno.
pub async fn cierre_de_la_papeleria(pool: &PgPool) -> Result<Option<NaiveDateTime>, sqlx::Error> {
    let fila: Option<(Option<String>, Option<NaiveDateTime>)> =
        sqlx::query_as("SELECT value, lastupdated FROM settings WHERE key = 'PAPELERIA_ABIERTA'")
            .fetch_optional(pool)
            .await?;
    Ok(match fila {
        Some((Some(valor), cuando)) if valor.trim() == "0" => Some(cuando.unwrap_or_default()),
        _ => None,
    })
}

// ── Lo que lee la app ────────────────────────────────────────────────────────

/// Cuánto de la cita se manda: es solo para recordar a qué se responde.
const LARGO_CITA: i32 = 120;

#[derive(sqlx::FromRow)]
struct FilaVista {
    id: Uuid,
    autor: String,
    tipo: String,
    texto: Option<String>,
    respondea: Option<Uuid>,
    cita: Option<String>,
    fecha: NaiveDateTime,
    leido: bool,
    editado: bool,
    eliminado: bool,
    adjuntos: Option<serde_json::Value>,
    referencia: Option<String>,
}

/// Mensajes de la conversación, del más viejo al más nuevo: los últimos `limite`, o los
/// anteriores al mensaje `antes` (al subir en el chat). Se pagina por (fecha, id) para no
/// perder mensajes del mismo segundo. Sin tarjetas: esas las pone `chat::tarjetas::poner`.
pub async fn conversacion(pool: &PgPool, cliente: Uuid, antes: Option<Uuid>, limite: i64) -> Result<Vec<Vista>, sqlx::Error> {
    let filas: Vec<FilaVista> = sqlx::query_as(
        "SELECT m.id, m.autor, m.tipo, m.texto, m.respondea,
                CASE WHEN r.fechaeliminado IS NULL THEN LEFT(r.texto, $4) END AS cita,
                m.fecha, m.fechaleido IS NOT NULL AS leido, m.fechaeditado IS NOT NULL AS editado,
                m.fechaeliminado IS NOT NULL AS eliminado, m.adjuntos, m.ref AS referencia
         FROM mensajes m
         LEFT JOIN mensajes r ON r.id = m.respondea
         WHERE m.clienteid = $1
           AND m.fecha > COALESCE((SELECT chatvaciado FROM clientes WHERE id = $1), '-infinity')
           AND ($2::uuid IS NULL OR (m.fecha, m.id) < (SELECT a.fecha, a.id FROM mensajes a WHERE a.id = $2 AND a.clienteid = $1))
         ORDER BY m.fecha DESC, m.id DESC
         LIMIT $3",
    )
    .bind(cliente)
    .bind(antes)
    .bind(limite)
    .bind(LARGO_CITA)
    .fetch_all(pool)
    .await?;
    Ok(filas
        .into_iter()
        .rev()
        .map(|f| {
            let archivos: Vec<Adjunto> = f.adjuntos.and_then(|a| serde_json::from_value(a).ok()).unwrap_or_default();
            let (adjuntos, zip) = adjuntos_vista(f.id, &archivos);
            Vista {
                id: f.id,
                autor: f.autor,
                tipo: f.tipo,
                texto: f.texto,
                responde_a: f.respondea,
                cita: f.cita,
                fecha: f.fecha,
                leido: f.leido,
                editado: f.editado,
                eliminado: f.eliminado,
                adjuntos,
                zip,
                referencia: f.referencia,
                tarjeta: None,
            }
        })
        .collect())
}

/// Si la caja bloqueó el chat de este cliente (`Clientes.ChatBloqueado`).
pub async fn bloqueado(pool: &PgPool, cliente: Uuid) -> Result<bool, sqlx::Error> {
    let bloqueado: Option<bool> = sqlx::query_scalar("SELECT chatbloqueado IS NOT NULL FROM clientes WHERE id = $1")
        .bind(cliente)
        .fetch_optional(pool)
        .await?;
    Ok(bloqueado.unwrap_or(false))
}

/// Mensajes que le escribieron al cliente (papelería, automáticos) y no ha leído: el globito del menú.
pub async fn sin_leer(pool: &PgPool, cliente: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM mensajes
         WHERE clienteid = $1 AND autor <> 'cliente' AND fechaleido IS NULL AND fechaeliminado IS NULL
           AND fecha > COALESCE((SELECT chatvaciado FROM clientes WHERE id = $1), '-infinity')",
    )
    .bind(cliente)
    .fetch_one(pool)
    .await
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::chat::contrato::Tipo;

    /// Carga el schema real del POS en la base temporal de la prueba, para no tener una copia que
    /// se desfase. `false` si no está el repo del POS (la prueba se salta).
    pub(crate) async fn con_schema(pool: &PgPool) -> bool {
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

    pub(crate) async fn cliente(pool: &PgPool, telefono: &str) -> Uuid {
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
        sqlx::query("INSERT INTO settings (key, value) VALUES ('CHAT_MB_DOCUMENTO', '40'), ('CHAT_MB_VIDEO', '0')")
            .execute(&pool)
            .await
            .unwrap();
        let a = ajustes(&pool).await.unwrap();
        assert_eq!((a.mb_documento, a.mb_video), (40, 100), "0 no es un límite: queda el de siempre");
    }

    #[sqlx::test(migrations = false)]
    async fn quien_ve_el_chat_sale_de_settings(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        assert_eq!(ajustes(&pool).await.unwrap().clientes_prueba, None, "sin la fila");
        sqlx::query("INSERT INTO settings (key, value) VALUES ('CHAT_CLIENTES_PRUEBA', NULL)").execute(&pool).await.unwrap();
        assert_eq!(ajustes(&pool).await.unwrap().clientes_prueba.as_deref(), Some(""), "NULL cuenta como vacía");
        sqlx::query("UPDATE settings SET value = 'a,b' WHERE key = 'CHAT_CLIENTES_PRUEBA'").execute(&pool).await.unwrap();
        assert_eq!(ajustes(&pool).await.unwrap().clientes_prueba.as_deref(), Some("a,b"));
    }

    #[sqlx::test(migrations = false)]
    async fn la_conversacion_por_paginas_y_con_su_cita(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        let (c, otro) = (cliente(&pool, "9840000001").await, cliente(&pool, "9840000002").await);
        guardar(&pool, &nuevo(otro, Lado::Cliente, "16:00:00")).await.unwrap();
        // Tres del mismo segundo y uno después: la página no debe perder ninguno.
        let mut ids = vec![];
        for _ in 0..3 {
            let n = nuevo(c, Lado::Cliente, "17:00:00");
            guardar(&pool, &n).await.unwrap();
            ids.push(n.id);
        }
        let mut respuesta = nuevo(c, Lado::Papeleria, "17:05:00");
        respuesta.texto = Some("Sí".into());
        respuesta.responde_a = Some(ids[0]);
        guardar(&pool, &respuesta).await.unwrap();

        let todo = conversacion(&pool, c, None, 50).await.unwrap();
        assert_eq!(todo.len(), 4, "solo los de este cliente");
        assert_eq!(todo.last().unwrap().id, respuesta.id, "del más viejo al más nuevo");
        assert_eq!(todo.last().unwrap().cita.as_deref(), Some("Hola"));

        let ultimos = conversacion(&pool, c, None, 2).await.unwrap();
        let anteriores = conversacion(&pool, c, Some(ultimos[0].id), 2).await.unwrap();
        let resto = conversacion(&pool, c, Some(anteriores[0].id), 2).await.unwrap();
        let mut vistos: Vec<Uuid> = [resto, anteriores, ultimos].concat().iter().map(|v| v.id).collect();
        assert_eq!(vistos.len(), 4);
        vistos.sort();
        vistos.dedup();
        assert_eq!(vistos.len(), 4, "sin repetidos ni perdidos");

        // Citado y eliminado: ya no se muestra su texto.
        eliminar(&pool, ids[0], hora("17:10:00")).await.unwrap();
        let todo = conversacion(&pool, c, None, 50).await.unwrap();
        assert_eq!(todo.last().unwrap().cita, None);
        assert!(todo.iter().any(|v| v.id == ids[0] && v.eliminado && v.texto.is_none()));

        // Vaciar: la app ya no ve ni cuenta lo de antes; lo que llega después, sí.
        assert_eq!(sin_leer(&pool, c).await.unwrap(), 1, "la respuesta de la papelería");
        vaciar(&pool, c, hora("17:05:00")).await.unwrap();
        assert!(conversacion(&pool, c, None, 50).await.unwrap().is_empty(), "lo de la misma hora también se va");
        assert_eq!(sin_leer(&pool, c).await.unwrap(), 0);
        let despues = nuevo(c, Lado::Papeleria, "17:20:00");
        guardar(&pool, &despues).await.unwrap();
        let todo = conversacion(&pool, c, None, 50).await.unwrap();
        assert_eq!(todo.iter().map(|v| v.id).collect::<Vec<_>>(), vec![despues.id]);
        assert_eq!(sin_leer(&pool, c).await.unwrap(), 1);
        let filas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mensajes WHERE clienteid = $1").bind(c).fetch_one(&pool).await.unwrap();
        assert_eq!(filas, 5, "no se borra nada: la caja ve todo");
    }

    #[sqlx::test(migrations = false)]
    async fn bloqueo_y_sin_leer(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        let c = cliente(&pool, "9840000001").await;
        assert!(!bloqueado(&pool, c).await.unwrap());
        assert!(!bloqueado(&pool, Uuid::new_v4()).await.unwrap());
        sqlx::query("UPDATE clientes SET chatbloqueado = NOW() WHERE id = $1").bind(c).execute(&pool).await.unwrap();
        assert!(bloqueado(&pool, c).await.unwrap());

        guardar(&pool, &nuevo(c, Lado::Cliente, "17:00:00")).await.unwrap();
        guardar(&pool, &nuevo(c, Lado::Papeleria, "17:01:00")).await.unwrap();
        guardar(&pool, &nuevo(c, Lado::Automatico, "17:02:00")).await.unwrap();
        let borrado = nuevo(c, Lado::Papeleria, "17:03:00");
        guardar(&pool, &borrado).await.unwrap();
        eliminar(&pool, borrado.id, hora("17:04:00")).await.unwrap();
        assert_eq!(sin_leer(&pool, c).await.unwrap(), 2, "los suyos y los eliminados no cuentan");
        marcar_leidos(&pool, c, false, hora("17:05:00")).await.unwrap();
        assert_eq!(sin_leer(&pool, c).await.unwrap(), 0);
    }

    #[sqlx::test(migrations = false)]
    async fn la_conversacion_trae_archivos_y_ref(pool: PgPool) {
        if !con_schema(&pool).await {
            return;
        }
        let c = cliente(&pool, "9840000001").await;
        let mut dos = nuevo(c, Lado::Cliente, "17:00:00");
        let mut foto = dos.adjuntos[0].clone();
        foto.nombre = "foto.jpg".into();
        foto.mime = "image/jpeg".into();
        foto.miniatura = Some(format!("{c}/x/foto.mini.jpg"));
        dos.adjuntos.push(foto);
        let uno = nuevo(c, Lado::Cliente, "17:01:00");
        let mut tarjeta = nuevo(c, Lado::Papeleria, "17:02:00");
        (tarjeta.tipo, tarjeta.texto, tarjeta.adjuntos, tarjeta.referencia) = (Tipo::Producto, None, vec![], Some("1234".into()));
        for n in [&dos, &uno, &tarjeta] {
            guardar(&pool, n).await.unwrap();
        }

        let todo = conversacion(&pool, c, None, 50).await.unwrap();
        let (a, b, t) = (&todo[0], &todo[1], &todo[2]);
        assert_eq!(a.adjuntos.len(), 2);
        assert_eq!(a.adjuntos[1].original, format!("/chat/archivo/{}/1/original", dos.id));
        assert_eq!(a.adjuntos[1].mini, Some(format!("/chat/archivo/{}/1/mini", dos.id)));
        assert_eq!((a.adjuntos[0].clase, a.adjuntos[1].clase), ("pdf", "imagen"));
        assert_eq!(a.zip, Some(format!("/chat/zip/{}", dos.id)));
        assert_eq!((b.adjuntos.len(), b.zip.as_deref()), (1, None), "con uno no hay \"Bajar todo\"");
        assert_eq!((t.tipo.as_str(), t.referencia.as_deref(), t.adjuntos.len()), ("producto", Some("1234"), 0));
        assert_eq!(t.tarjeta, None, "la pone chat::tarjetas");

        eliminar(&pool, dos.id, hora("17:10:00")).await.unwrap();
        let todo = conversacion(&pool, c, None, 50).await.unwrap();
        assert_eq!((todo[0].adjuntos.len(), todo[0].zip.as_deref()), (0, None), "eliminado: sin archivos");
    }
}
