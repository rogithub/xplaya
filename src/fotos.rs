//! Fotos de credencial y polaroid desde la app (plan: artifact "Fotos en la app"). El cliente
//! las hace en la app, pero el pedido vive en **imagina**: ahí la operadora lo corrige, lo
//! imprime y lo marca impreso o lo borra. Esta parte de xplaya hace dos cosas:
//!
//! 1. **Permiso** (`Firmador`): el celular del cliente manda sus fotos directo a imagina (no
//!    pasan por aquí: la polaroid son varias fotos pesadas). Para que imagina sepa quién es, xplaya
//!    le da un permiso firmado con su llave Ed25519 (`FOTOS_LLAVE_PRIVADA`, SealedSecret) con el
//!    cliente, su nombre y teléfono, que vence en minutos. imagina solo tiene la pública
//!    (`imagina/src/permiso.rs`, mismo formato).
//! 2. **Estado** (`vigilar`): imagina publica en NATS `imagina.pedidos` cada cambio de un pedido
//!    de la app (recibido, impreso, cancelado, borrado). xplaya lo lee del stream IMAGINA
//!    (consumer `xplaya`), lo guarda en `PedidosFotos` y le avisa al cliente.
//!
//! Formato del permiso: `base64url(json) . base64url(firma del texto base64)`, sin relleno;
//! JSON `{"c": cliente_id, "n": nombre, "t": teléfono, "e": vence (segundos unix)}`.

use std::time::Duration;

use async_nats::jetstream::{consumer::PullConsumer, AckKind};
use base64ct::{Base64UrlUnpadded, Encoding};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::StreamExt;
use serde::Deserialize;
use uuid::Uuid;

use crate::{db, push, AppState};

/// Lo que dura el permiso: tomar la foto, acomodarla y mandarla. La app pide otro si vence.
pub const VIGENCIA_PERMISO: Duration = Duration::from_secs(30 * 60);

pub struct Firmador {
    llave: SigningKey,
    /// La que se configura en imagina (`XPLAYA_LLAVE_PUBLICA`), base64url de 32 bytes.
    pub publica: String,
}

impl Firmador {
    /// `None` si la variable está vacía: no hay fotos en la app.
    pub fn desde_base64(privada: &str) -> Option<Self> {
        let privada = privada.trim();
        if privada.is_empty() {
            return None;
        }
        let bytes: [u8; 32] = Base64UrlUnpadded::decode_vec(privada)
            .ok()
            .and_then(|b| b.try_into().ok())
            .expect("FOTOS_LLAVE_PRIVADA tiene que ser base64url sin relleno de 32 bytes");
        let llave = SigningKey::from_bytes(&bytes);
        let publica = Base64UrlUnpadded::encode_string(llave.verifying_key().as_bytes());
        Some(Self { llave, publica })
    }

    /// Permiso para que `cliente` mande un pedido a imagina. `ahora` en segundos unix.
    pub fn permiso(&self, cliente: Uuid, nombre: &str, telefono: &str, ahora: i64) -> String {
        let json = serde_json::json!({
            "c": cliente,
            "n": nombre,
            "t": telefono,
            "e": ahora + VIGENCIA_PERMISO.as_secs() as i64,
        })
        .to_string();
        let datos = Base64UrlUnpadded::encode_string(json.as_bytes());
        // Se firma el texto base64 tal como viaja: imagina lo revisa sin volver a codificar.
        let firma = self.llave.sign(datos.as_bytes());
        format!("{datos}.{}", Base64UrlUnpadded::encode_string(&firma.to_bytes()))
    }
}

// ── Estado de los pedidos: NATS imagina.pedidos ─────────────────────────────

/// Lo que publica imagina (`imagina/src/avisos.rs`, `pedido_de_la_app`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evento {
    pub codigo: String,
    pub cliente_id: Uuid,
    pub tipo: String,
    pub estado: String,
}

/// Escucha los cambios de los pedidos de fotos. Sin usuario de NATS no hace nada: los pedidos
/// llegan igual a imagina, solo que la app no ve en qué van.
pub fn vigilar(state: AppState) {
    if state.config.nats_usuario.is_empty() {
        tracing::warn!("fotos: sin NATS_USUARIO, la app no ve en qué van los pedidos");
        return;
    }
    tokio::spawn(async move {
        loop {
            if let Err(e) = escuchar_una_vez(&state).await {
                tracing::warn!("fotos: NATS: {e}; reintento en 10 s");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

async fn escuchar_una_vez(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = &state.config;
    let cliente = async_nats::ConnectOptions::with_user_and_password(cfg.nats_usuario.clone(), cfg.nats_password.clone())
        .name("xplaya-fotos")
        .connect(&cfg.nats_url)
        .await?;
    let js = async_nats::jetstream::new(cliente);
    // Stream y consumer los crea `ro` a mano (k3s-manifests/CLAUDE.md, sección NATS).
    let consumer: PullConsumer = js.get_consumer_from_stream("xplaya", "IMAGINA").await?;
    let mut mensajes = consumer.messages().await?;
    tracing::info!("fotos: escuchando NATS (IMAGINA > xplaya)");

    while let Some(mensaje) = mensajes.next().await {
        let mensaje = mensaje?;
        let Ok(evento) = serde_json::from_slice::<Evento>(&mensaje.payload) else {
            tracing::warn!("fotos: mensaje ilegible, se descarta: {:?}", String::from_utf8_lossy(&mensaje.payload));
            mensaje.ack().await?;
            continue;
        };
        match guardar(state, &evento).await {
            Ok(()) => mensaje.ack().await?,
            Err(e) => {
                tracing::warn!("fotos: {}: {e}; NATS lo vuelve a dar en 30 s", evento.codigo);
                mensaje.ack_with(AckKind::Nak(Some(Duration::from_secs(30)))).await?;
            }
        }
    }
    Ok(())
}

/// Guarda el estado y, si el pedido terminó, le avisa al cliente.
async fn guardar(state: &AppState, e: &Evento) -> Result<(), sqlx::Error> {
    if !matches!(e.estado.as_str(), "recibido" | "impreso" | "cancelado" | "borrado") {
        tracing::warn!("fotos: estado desconocido {:?} en {}, se ignora", e.estado, e.codigo);
        return Ok(());
    }
    let anterior = db::fotos::guardar(&state.pool, &e.codigo, e.cliente_id, &e.tipo, &e.estado).await?;
    tracing::info!("fotos: {} {} → {}", e.codigo, anterior.as_deref().unwrap_or("(nuevo)"), e.estado);
    // Solo se avisa una vez, al terminar: si la operadora lo marca impreso y luego lo borra, el
    // segundo cambio ya no avisa.
    let ya_habia_terminado = matches!(anterior.as_deref(), Some("impreso" | "cancelado" | "borrado"));
    if let Some(texto) = aviso(&e.estado)
        && !ya_habia_terminado
        && let Some(vapid) = &state.vapid
    {
        let cuerpo = serde_json::json!({
            "titulo": "Papelería xplaya",
            "texto": texto,
            "url": "/fotos",
            "tag": format!("fotos-{}", e.codigo),
        });
        if let Err(err) = push::mandar_del_momento(state, vapid, e.cliente_id, cuerpo, Duration::from_secs(24 * 60 * 60)).await {
            tracing::warn!("fotos: aviso de {}: {err}", e.codigo);
        }
    }
    Ok(())
}

/// Texto del aviso al cliente según el nuevo estado; `None`: no se avisa.
fn aviso(estado: &str) -> Option<&'static str> {
    match estado {
        "impreso" | "borrado" => Some("✅ Terminamos tu pedido de fotos. ¡Gracias!"),
        "cancelado" => Some("Cancelamos tu pedido de fotos. Si fue un error, escríbenos."),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};

    #[test]
    fn el_permiso_se_puede_revisar_con_la_publica() {
        let semilla = Base64UrlUnpadded::encode_string(&[7u8; 32]);
        let f = Firmador::desde_base64(&semilla).unwrap();
        let p = f.permiso(Uuid::nil(), "Ana López", "9841234567", 1000);
        let (datos, firma) = p.split_once('.').unwrap();
        // Lo mismo que hace imagina: la firma es del texto base64.
        let publica = Base64UrlUnpadded::decode_vec(&f.publica).unwrap();
        let publica = ed25519_dalek::VerifyingKey::from_bytes(&publica.try_into().unwrap()).unwrap();
        let firma = Signature::from_slice(&Base64UrlUnpadded::decode_vec(firma).unwrap()).unwrap();
        assert!(publica.verify(datos.as_bytes(), &firma).is_ok());
        let json: serde_json::Value = serde_json::from_slice(&Base64UrlUnpadded::decode_vec(datos).unwrap()).unwrap();
        assert_eq!(json["n"], "Ana López");
        assert_eq!(json["e"], 1000 + VIGENCIA_PERMISO.as_secs() as i64);
    }

    #[test]
    fn sin_llave_no_hay_fotos() {
        assert!(Firmador::desde_base64("  ").is_none());
    }

    #[test]
    fn avisos_al_terminar() {
        assert!(aviso("recibido").is_none());
        assert!(aviso("impreso").is_some());
        assert!(aviso("borrado").is_some());
        assert!(aviso("cancelado").is_some());
    }
}
