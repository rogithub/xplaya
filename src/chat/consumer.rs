//! Lee el stream `CHAT` (consumer `xplaya`) y guarda en `Mensajes` todo lo que pasa, venga de
//! quien venga (`CHAT.md`). Misma forma que `fotos.rs`.
//!
//! Es un adaptador: lee el tema y el cuerpo, le pregunta a `reglas::aplicar` qué hacer y lo
//! ejecuta. No decide nada.

use std::time::Duration;

use async_nats::jetstream::{self, consumer::PullConsumer, AckKind};
use futures_util::StreamExt;

use super::contrato::{self, Entrante, Tema};
use super::reglas::{self, Cambio};
use crate::{db, AppState};

const STREAM: &str = "CHAT";
const CONSUMER: &str = "xplaya";
/// Bucket de los archivos del chat (90 días, sin copia a R2).
pub const BUCKET: &str = "papeleria-chat";

/// Sin usuario de NATS no hace nada: el chat no funciona sin el stream.
pub fn vigilar(state: AppState) {
    if state.config.nats_usuario.is_empty() {
        tracing::warn!("chat: sin NATS_USUARIO, no se guardan los mensajes");
        return;
    }
    tokio::spawn(async move {
        loop {
            if let Err(e) = escuchar_una_vez(&state).await {
                tracing::warn!("chat: NATS: {e}; reintento en 10 s");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

async fn escuchar_una_vez(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = &state.config;
    let cliente = async_nats::ConnectOptions::with_user_and_password(cfg.nats_usuario.clone(), cfg.nats_password.clone())
        .name("xplaya-chat")
        .connect(&cfg.nats_url)
        .await?;
    let js = jetstream::new(cliente);
    // Stream y consumer los crea `ro` a mano (CHAT.md, "Etapa 2 en el cluster").
    let consumer: PullConsumer = js.get_consumer_from_stream(CONSUMER, STREAM).await?;
    let mut mensajes = consumer.messages().await?;
    tracing::info!("chat: escuchando NATS ({STREAM} > {CONSUMER})");

    while let Some(mensaje) = mensajes.next().await {
        let mensaje = mensaje?;
        let secuencia = mensaje.info()?.stream_sequence;
        match procesar(state, &js, &mensaje.subject, &mensaje.payload, secuencia).await {
            Ok(()) => mensaje.ack().await?,
            Err(e) => {
                tracing::warn!("chat: {} #{secuencia}: {e}; NATS lo vuelve a dar en 30 s", mensaje.subject);
                mensaje.ack_with(AckKind::Nak(Some(Duration::from_secs(30)))).await?;
            }
        }
    }
    Ok(())
}

/// Un evento del stream. `Err` solo si falló la BD (se reintenta); lo que no se entiende o las
/// reglas rechazan se deja pasar con una nota en el log.
async fn procesar(state: &AppState, js: &jetstream::Context, tema: &str, cuerpo: &[u8], secuencia: u64) -> Result<(), sqlx::Error> {
    let Some(t) = Tema::leer(tema) else {
        tracing::warn!("chat: tema desconocido {tema:?}, se descarta");
        return Ok(());
    };
    let entrante = match contrato::leer(t.evento, cuerpo) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("chat: {tema} #{secuencia}: {e}; se descarta");
            return Ok(());
        }
    };
    let id = match &entrante {
        Entrante::Mensaje(m) => Some(m.id),
        Entrante::Editado(e) => Some(e.id),
        Entrante::Eliminado(e) => Some(e.id),
        Entrante::Leido(_) | Entrante::Sugerencia => None,
    };
    let guardado = match id {
        Some(id) => db::chat::guardado(&state.pool, id).await?,
        None => None,
    };
    let ajustes = db::chat::ajustes(&state.pool).await?;

    match reglas::aplicar(&t, entrante, guardado.as_ref(), secuencia, &ajustes) {
        Cambio::Guardar(n) => {
            if !db::chat::guardar(&state.pool, &n).await? {
                tracing::warn!("chat: {} no se guardó (repetido o el cliente no existe)", n.id);
            }
        }
        Cambio::MarcarLeidos { cliente, del_cliente, hasta } => {
            db::chat::marcar_leidos(&state.pool, cliente, del_cliente, hasta).await?;
        }
        Cambio::Editar { id, texto, fecha } => db::chat::editar(&state.pool, id, &texto, fecha).await?,
        Cambio::Eliminar { id, fecha, llaves, secuencia: del_mensaje } => {
            db::chat::eliminar(&state.pool, id, fecha).await?;
            // Lo que sigue es limpieza: si falla, el archivo se borra solo a los 90 días y el
            // mensaje sale del stream a los 30. No se reintenta el evento por eso.
            borrar_archivos(state, &llaves).await;
            if let Some(seq) = del_mensaje {
                quitar_del_stream(js, seq).await;
            }
        }
        Cambio::Ignorar(motivo) => tracing::info!("chat: {tema} #{secuencia} ignorado: {motivo}"),
    }
    Ok(())
}

async fn borrar_archivos(state: &AppState, llaves: &[String]) {
    if llaves.is_empty() {
        return;
    }
    let Some(minio) = &state.minio else {
        tracing::warn!("chat: sin MinIO, quedan {} archivos hasta que venzan", llaves.len());
        return;
    };
    let minio = minio.en(BUCKET);
    for llave in llaves {
        if let Err(e) = minio.borrar(llave).await {
            tracing::warn!("chat: {e}");
        }
    }
}

async fn quitar_del_stream(js: &jetstream::Context, secuencia: u64) {
    let resultado = match js.get_stream(STREAM).await {
        Ok(stream) => stream.delete_message(secuencia).await.map(|_| ()).map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    if let Err(e) = resultado {
        tracing::warn!("chat: no se pudo quitar #{secuencia} del stream: {e}");
    }
}
