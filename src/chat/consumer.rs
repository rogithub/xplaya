//! Lee el stream `CHAT` (consumer `xplaya`) y guarda en `Mensajes` todo lo que pasa, venga de
//! quien venga (`CHAT.md`). Misma forma que `fotos.rs`.
//!
//! Es un adaptador: lee el tema y el cuerpo, le pregunta a `reglas::aplicar` qué hacer y lo
//! ejecuta. No decide nada. Después de guardar, avisa a la app en vivo (`nats.rs`).

use std::time::Duration;

use async_nats::jetstream::{self, consumer::PullConsumer, AckKind};
use futures_util::StreamExt;

use super::contrato::{self, Entrante, Evento, Lado, Tema};
use super::reglas::{self, Cambio};
use super::tarjetas;
use super::vista::{EnVivo, Vista};
use crate::{db, AppState};

const STREAM: &str = "CHAT";
const CONSUMER: &str = "xplaya";

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
            if db::chat::guardar(&state.pool, &n).await? {
                let mut vista = Vista::from(&n);
                if let Some(referencia) = &n.referencia {
                    // El mensaje ya está guardado: si la tarjeta falla, se avisa sin ella.
                    match tarjetas::resolver(&state.pool, (&state.config).into(), &vista.tipo, referencia).await {
                        Ok(t) => vista.tarjeta = t,
                        Err(e) => tracing::warn!("chat: tarjeta de {}: {e}", n.id),
                    }
                }
                state.chat.avisar(n.cliente, EnVivo::Mensaje(Box::new(vista)));
                if n.autor == Lado::Cliente {
                    fuera_de_horario(state, n.cliente, n.fecha).await?;
                }
            } else {
                tracing::warn!("chat: {} no se guardó (repetido o el cliente no existe)", n.id);
            }
        }
        Cambio::MarcarLeidos { cliente, del_cliente, hasta } => {
            db::chat::marcar_leidos(&state.pool, cliente, del_cliente, hasta).await?;
            state.chat.avisar(cliente, EnVivo::Leido { del_cliente, hasta });
        }
        Cambio::Editar { id, texto, fecha } => {
            db::chat::editar(&state.pool, id, &texto, fecha).await?;
            state.chat.avisar(t.cliente, EnVivo::Editado { id, texto, fecha });
        }
        Cambio::Eliminar { id, fecha, llaves, secuencia: del_mensaje } => {
            db::chat::eliminar(&state.pool, id, fecha).await?;
            state.chat.avisar(t.cliente, EnVivo::Eliminado { id, fecha });
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

/// Si el cliente escribió con la tienda cerrada, la respuesta automática con el horario, una vez
/// por cierre (`reglas::aviso_fuera_de_horario`). Se publica en el stream como cualquier mensaje y
/// vuelve por aquí para guardarse. Si NATS falla, solo va al log: el mensaje del cliente ya se
/// guardó, y reintentarlo no traería el aviso (el repetido se ignora).
async fn fuera_de_horario(state: &AppState, cliente: uuid::Uuid, fecha: chrono::NaiveDateTime) -> Result<(), sqlx::Error> {
    let Some(aviso) = reglas::aviso_fuera_de_horario(cliente, contrato::con_zona(fecha)) else {
        return Ok(());
    };
    if db::chat::guardado(&state.pool, aviso.id).await?.is_some() {
        return Ok(());
    }
    let tema = Tema { cliente, lado: Lado::Automatico, evento: Evento::Mensaje };
    if let Err(e) = state.chat.publicar(&tema, &aviso, Some(aviso.id)).await {
        tracing::warn!("chat: no se pudo mandar el aviso de fuera de horario a {cliente}: {e}");
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
    let minio = minio.en(crate::minio::BUCKET_CHAT);
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
