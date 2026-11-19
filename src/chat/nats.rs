//! Lo que hace xplaya en NATS por el cliente (`CHAT.md`): publica sus mensajes en el stream
//! `CHAT` y su "escribiendo…" por NATS núcleo. También reparte los cambios en vivo a los
//! celulares que tienen el chat abierto (`routes/chat.rs`).
//!
//! Los cambios en vivo los manda el consumer después de guardar (`consumer.rs`), así la app solo
//! ve lo que ya está en `Mensajes`. El reparto es en memoria: sirve porque xplaya corre con **una
//! réplica**; con dos, cada una vería solo lo que leyó su consumer.

use std::sync::Arc;

use async_nats::{jetstream, HeaderMap};
use futures_util::StreamExt;
use serde::Serialize;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::contrato::{Evento, Lado, Tema};
use super::vista::EnVivo;
use crate::config::Config;

/// Un cambio en vivo y de qué cliente es.
pub type Cambio = (Uuid, Arc<EnVivo>);

#[derive(Clone)]
pub struct Chat {
    /// `None`: sin `NATS_USUARIO`, no se puede mandar nada.
    nats: Option<(async_nats::Client, jetstream::Context)>,
    cambios: broadcast::Sender<Cambio>,
}

impl Chat {
    /// Conexión propia. Con `retry_on_initial_connect` no espera a NATS: si no está al arrancar,
    /// conecta en cuanto pueda (como `avisos.rs`).
    pub async fn conectar(cfg: &Config) -> Self {
        let (cambios, _) = broadcast::channel(256);
        if cfg.nats_usuario.is_empty() {
            tracing::warn!("chat: sin NATS_USUARIO, la app no puede mandar mensajes");
            return Chat { nats: None, cambios };
        }
        let cliente = async_nats::ConnectOptions::with_user_and_password(cfg.nats_usuario.clone(), cfg.nats_password.clone())
            .name("xplaya-chat-app")
            .retry_on_initial_connect()
            .connect(&cfg.nats_url)
            .await;
        match cliente {
            Ok(c) => {
                let js = jetstream::new(c.clone());
                Chat { nats: Some((c, js)), cambios }
            }
            Err(e) => {
                tracing::warn!("chat: NATS: {e}; la app no puede mandar mensajes");
                Chat { nats: None, cambios }
            }
        }
    }

    /// Publica en el stream y espera a que NATS lo guarde (el ✓ de la app). `msg_id` va en
    /// `Nats-Msg-Id`: si el celular reintenta el mismo mensaje dentro de 2 min, el stream no lo
    /// guarda dos veces (después, el `ON CONFLICT` de la tabla).
    pub async fn publicar(&self, tema: &Tema, cuerpo: &impl Serialize, msg_id: Option<Uuid>) -> Result<(), String> {
        let Some((_, js)) = &self.nats else {
            return Err("sin NATS".into());
        };
        let json = serde_json::to_vec(cuerpo).map_err(|e| e.to_string())?;
        let mut headers = HeaderMap::new();
        if let Some(id) = msg_id {
            headers.insert("Nats-Msg-Id", id.to_string().as_str());
        }
        js.publish_with_headers(tema.armar(), headers, json.into())
            .await
            .map_err(|e| e.to_string())?
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// "Escribiendo…" del cliente, por NATS núcleo: es del momento, si se pierde no pasa nada.
    pub async fn escribiendo(&self, cliente: Uuid) {
        let Some((c, _)) = &self.nats else { return };
        let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Escribiendo };
        if let Err(e) = c.publish(tema.armar(), "{}".into()).await {
            tracing::debug!("chat: escribiendo: {e}");
        }
    }

    pub fn avisar(&self, cliente: Uuid, cambio: EnVivo) {
        // Sin nadie escuchando, `send` falla; no importa.
        let _ = self.cambios.send((cliente, Arc::new(cambio)));
    }

    pub fn suscribir(&self) -> broadcast::Receiver<Cambio> {
        self.cambios.subscribe()
    }

    /// Escucha el "escribiendo…" de la papelería y lo pasa a la app. El cliente de NATS vuelve a
    /// suscribirse solo si la conexión se cae.
    pub fn vigilar_escribiendo(&self) {
        let Some((c, _)) = self.nats.clone() else { return };
        let chat = self.clone();
        tokio::spawn(async move {
            let mut mensajes = match c.subscribe("chat.*.papeleria.escribiendo").await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("chat: no se pudo escuchar escribiendo: {e}");
                    return;
                }
            };
            while let Some(m) = mensajes.next().await {
                if let Some(t) = Tema::leer(&m.subject) {
                    chat.avisar(t.cliente, EnVivo::Escribiendo);
                }
            }
        });
    }
}
