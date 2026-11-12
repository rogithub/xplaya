//! Avisos a las personas de la papelería (no a los clientes): xplaya publica en NATS
//! `avisos.<topic>` el JSON de publicación de ntfy, y el puente `nats-ntfy`
//! (github.com/rogithub/nats-ntfy) lo reenvía a ntfy. xplaya nunca habla con ntfy ni tiene su token.
//!
//! Avisos de hoy:
//! - `alta_kiosko`: el código de un alta en el kiosko, con botón que abre WhatsApp con el mensaje
//!   ya escrito (topic `papeleria-kiosko`).
//! - `pedido_nuevo`: un pedido en línea (carrito de xplaya.com o tienda de la app), con botones a
//!   la cotización y al WhatsApp del cliente (topic `pedidos`).
//!
//! Son un camino más, por si nadie está viendo el POS. Si NATS no está (o no hay usuario), no pasa
//! nada: lo importante ya está en la BD y el POS lo muestra. Por eso se publica en una tarea aparte
//! y un error solo va al log.

use std::time::Duration;

use async_nats::jetstream;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{config::Config, db};

/// Topic de ntfy (`papeleria-kiosko`) al que se suscribe el celular de la papelería. `dayana`
/// puede leer `papeleria-*` y `ro` todo.
const SUBJECT_PAPELERIA: &str = "avisos.papeleria-kiosko";

/// Topic de ntfy `pedidos` (lo leen `dayana` y `ro`).
const SUBJECT_PEDIDOS: &str = "avisos.pedidos";

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

    /// Pedido en línea recién guardado. `desde`: "la app" o "xplaya.com", para saber por dónde
    /// llegó. Lee de la BD el número, el total y el cliente (en la misma tarea aparte, así no
    /// retrasa la respuesta al cliente).
    pub fn pedido_nuevo(&self, pool: &PgPool, site_url: &str, uid: Uuid, desde: &'static str) {
        if self.js.is_none() {
            return;
        }
        let (avisos, pool, site_url) = (self.clone(), pool.clone(), site_url.to_string());
        tokio::spawn(async move {
            let datos = async {
                let Some(cot) = db::monedero::cotizacion(&pool, uid).await? else { return Ok(None) };
                let cliente: Option<(Option<String>, Option<String>)> = match cot.cliente_id {
                    Some(id) => sqlx::query_as("SELECT nombre, telefono FROM clientes WHERE id = $1")
                        .bind(id)
                        .fetch_optional(&pool)
                        .await?,
                    None => None,
                };
                Ok::<_, sqlx::Error>(Some((cot, cliente.unwrap_or((None, None)))))
            }
            .await;
            let (cot, (nombre, telefono)) = match datos {
                Ok(Some(d)) => d,
                Ok(None) => return tracing::warn!("avisos: pedido {uid} no encontrado"),
                Err(e) => return tracing::warn!("avisos: pedido {uid}: {e}"),
            };

            let nombre = nombre.unwrap_or_default();
            let primer_nombre = nombre.split_whitespace().next().unwrap_or("cliente");
            let n = cot.productos.len();
            let productos = if n == 1 { "1 producto".to_string() } else { format!("{n} productos") };
            let cotizacion = format!("{site_url}/cotizacion/{uid}");
            let mut acciones = vec![json!({ "action": "view", "label": "Ver pedido", "url": cotizacion })];
            let mut mensaje = format!("Pedido No. {} · {productos} · {}\n", cot.id, cot.total);
            // Los teléfonos de Clientes pueden traer prefijos (+52, 01): se usan los últimos 10.
            let tel: String = telefono.unwrap_or_default().chars().filter(|c| c.is_ascii_digit()).collect();
            if tel.len() >= 10 {
                let tel = &tel[tel.len() - 10..];
                mensaje.push_str(&format!("{} · ", formato_telefono(tel)));
                acciones.push(json!({ "action": "view", "label": "WhatsApp", "url": format!("https://wa.me/52{tel}") }));
            }
            mensaje.push_str(&format!("desde {desde}"));

            let aviso = json!({
                "title": format!("Pedido en línea: {primer_nombre}"),
                "message": mensaje,
                "priority": 4,
                "click": cotizacion,
                "actions": acciones,
            });
            avisos.publicar(SUBJECT_PEDIDOS, aviso.to_string());
        });
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
