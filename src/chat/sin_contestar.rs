//! Aviso a la papelería cuando un cliente escribe por el chat y nadie lo lee en la caja (etapa 6b,
//! `CHAT.md`): cada minuto busca las rachas de mensajes sin leer que acaban de cumplir
//! `CHAT_MINUTOS_AVISO` y manda un aviso por ntfy al celular de la papelería (`avisos.rs`).
//!
//! Es un adaptador: la ventana de tiempo y el texto los decide `reglas`, la consulta está en
//! `db::chat::sin_contestar`. Con la papelería cerrada (el switch de la caja) no avisa: de noche
//! no despierta a nadie, y al abrir los mensajes ya están en la bandeja del POS.

use std::time::Duration;

use super::{contrato::hora_tienda, reglas};
use crate::{db, AppState};

const CADA: Duration = Duration::from_secs(60);

pub fn vigilar(state: AppState) {
    tokio::spawn(async move {
        let mut antes = hora_tienda(chrono::Utc::now().fixed_offset());
        let mut reloj = tokio::time::interval(CADA);
        reloj.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        reloj.tick().await; // el primero es inmediato: la primera revisión, en un minuto
        loop {
            reloj.tick().await;
            let ahora = hora_tienda(chrono::Utc::now().fixed_offset());
            match revisar(&state, antes, ahora).await {
                // Solo avanza si revisó: si falló la BD, la siguiente vuelta cubre también esta.
                Ok(()) => antes = ahora,
                Err(e) => tracing::warn!("chat: revisar mensajes sin contestar: {e}"),
            }
        }
    });
}

async fn revisar(state: &AppState, antes: chrono::NaiveDateTime, ahora: chrono::NaiveDateTime) -> Result<(), sqlx::Error> {
    let ajustes = db::chat::ajustes(&state.pool).await?;
    if !ajustes.papeleria_abierta {
        return Ok(());
    }
    let Some((desde, hasta)) = reglas::ventana_de_aviso(antes, ahora, ajustes.minutos_aviso) else {
        return Ok(());
    };
    for s in db::chat::sin_contestar(&state.pool, desde, hasta).await? {
        let (titulo, mensaje) = reglas::aviso_a_la_papeleria(&s, ajustes.minutos_aviso);
        tracing::info!("chat: {} lleva {} sin contestar, aviso a la papelería", s.cliente, s.cuantos);
        state.avisos.chat_sin_contestar(s.cliente, titulo, mensaje);
    }
    Ok(())
}
