//! Avisos a las personas de la papelería (no a los clientes): xplaya publica en NATS
//! `avisos.<topic>` el JSON de publicación de ntfy, y el puente `nats-ntfy`
//! (github.com/rogithub/nats-ntfy) lo reenvía a ntfy. xplaya nunca habla con ntfy ni tiene su token.
//!
//! Hoy hay un solo aviso: el código de un alta en el kiosko, al celular de la papelería, con un
//! botón que abre WhatsApp con el mensaje ya escrito. El aviso del POS sigue igual; este es
//! un camino más, por si nadie está viendo la pantalla del POS.
//!
//! Si NATS no está (o no hay usuario), no pasa nada: la solicitud ya está en la BD y el POS la
//! muestra. Por eso se publica en una tarea aparte y un error solo va al log.

use std::time::Duration;

use async_nats::jetstream;
use serde_json::json;

use crate::config::Config;

/// Topic de ntfy al que está suscrito el celular de la papelería.
const SUBJECT_PAPELERIA: &str = "avisos.papeleria";

#[derive(Clone)]
pub struct Avisos {
    /// `None`: sin `NATS_USUARIO`, los avisos se apagan.
    js: Option<jetstream::Context>,
}

impl Avisos {
    /// Conexión propia, aparte de la que lee notificaciones en `push.rs`. Con
    /// `retry_on_initial_connect` no espera a NATS: si no está al arrancar, conecta en cuanto pueda.
    pub async fn conectar(cfg: &Config) -> Self {
        if cfg.nats_usuario.is_empty() {
            tracing::info!("avisos: sin NATS_USUARIO, apagados");
            return Avisos { js: None };
        }
        let cliente = async_nats::ConnectOptions::with_user_and_password(
            cfg.nats_usuario.clone(),
            cfg.nats_password.clone(),
        )
        .name("xplaya-avisos")
        .retry_on_initial_connect()
        .connect(&cfg.nats_url)
        .await;
        match cliente {
            Ok(c) => Avisos { js: Some(jetstream::new(c)) },
            Err(e) => {
                tracing::warn!("avisos: NATS: {e}; apagados");
                Avisos { js: None }
            }
        }
    }

    /// Código de un alta en el kiosko → celular de la papelería, con botón a WhatsApp.
    pub fn alta_kiosko(&self, nombre: &str, telefono: &str, codigo: &str) {
        let primer_nombre = nombre.split_whitespace().next().unwrap_or("");
        // El mismo texto que manda el POS (`kioskoMensaje.ts`). Los *asteriscos* son negritas en WhatsApp.
        let texto = format!(
            "Hola {primer_nombre}, tu código para el monedero de la Papelería es *{codigo}*.\n\
             Escríbelo en tu celular para terminar."
        );
        let whatsapp = format!("https://wa.me/52{telefono}?text={}", codificar_url(&texto));
        let aviso = json!({
            "title": format!("Alta en el kiosko: {primer_nombre}"),
            "message": format!("{nombre} · {} · código {codigo}", formato_telefono(telefono)),
            "priority": 4,
            // Tocar la notificación también abre WhatsApp.
            "click": whatsapp,
            "actions": [{ "action": "view", "label": "Enviar código por WhatsApp", "url": whatsapp }],
        });
        self.publicar(SUBJECT_PAPELERIA, aviso.to_string());
    }

    fn publicar(&self, subject: &'static str, cuerpo: String) {
        let Some(js) = self.js.clone() else { return };
        tokio::spawn(async move {
            // JetStream contesta con un acuse cuando el mensaje ya quedó guardado en el stream.
            let resultado = tokio::time::timeout(Duration::from_secs(5), async {
                js.publish(subject, cuerpo.into()).await?.await
            })
            .await;
            match resultado {
                Ok(Ok(_)) => tracing::info!("avisos: {subject} publicado"),
                Ok(Err(e)) => tracing::warn!("avisos: {subject}: {e}"),
                Err(_) => tracing::warn!("avisos: {subject}: NATS no contestó en 5 s"),
            }
        });
    }
}

/// "9841234567" → "984 123 4567".
fn formato_telefono(tel: &str) -> String {
    if tel.len() == 10 && tel.chars().all(|c| c.is_ascii_digit()) {
        format!("{} {} {}", &tel[..3], &tel[3..6], &tel[6..])
    } else {
        tel.to_string()
    }
}

/// Codifica para el `?text=` de wa.me (como `encodeURIComponent` en JS): deja letras, números
/// y `-_.~`, y todo lo demás va como `%XX` de sus bytes UTF-8.
fn codificar_url(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len() * 3);
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            salida.push(b as char);
        } else {
            salida.push_str(&format!("%{b:02X}"));
        }
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codifica_como_encode_uri_component() {
        assert_eq!(codificar_url("Hola Juan, *1234*.\nó"), "Hola%20Juan%2C%20%2A1234%2A.%0A%C3%B3");
    }

    #[test]
    fn telefono_con_espacios() {
        assert_eq!(formato_telefono("9841234567"), "984 123 4567");
        assert_eq!(formato_telefono("123"), "123");
    }
}
