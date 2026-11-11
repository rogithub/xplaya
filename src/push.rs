//! Notificaciones Web Push de la app (APP_MONEDERO.md, sección Notificaciones).
//!
//! VAPID: un par de llaves P-256 que identifica a xplaya ante los servicios de push (Google,
//! Apple, Mozilla). El celular se suscribe con la **pública** y solo acepta mensajes firmados con
//! la **privada**, que vive en el servidor (`VAPID_PRIVATE_KEY`, SealedSecret en k3s). La pública
//! se deriva de la privada: un solo secreto que guardar.

use std::{sync::Arc, time::Duration};

use async_nats::jetstream::{consumer::PullConsumer, AckKind};
use base64ct::{Base64UrlUnpadded, Encoding};
use futures_util::StreamExt;
use uuid::Uuid;
use web_push_native::{
    jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair},
    p256::PublicKey,
    Auth, WebPushBuilder,
};

use crate::{db, db::notificaciones::Destino, AppState};

pub struct Vapid {
    /// Firma cada mensaje (JWT ES256 en el encabezado `Authorization`).
    pub par: ES256KeyPair,
    /// Llave pública en base64url (65 bytes, punto sin comprimir): el `applicationServerKey`
    /// que la app le pasa a `pushManager.subscribe()`.
    pub publica: String,
}

impl Vapid {
    /// `None` si la variable está vacía: la app no ofrece notificaciones.
    pub fn desde_base64(privada: &str) -> Option<Self> {
        let privada = privada.trim();
        if privada.is_empty() {
            return None;
        }
        let bytes = Base64UrlUnpadded::decode_vec(privada)
            .expect("VAPID_PRIVATE_KEY tiene que ser base64url sin relleno (32 bytes)");
        let par = ES256KeyPair::from_bytes(&bytes)
            .expect("VAPID_PRIVATE_KEY no es una llave P-256 válida");
        let publica = Base64UrlUnpadded::encode_string(&par.public_key().public_key().to_bytes_uncompressed());
        Some(Self { par, publica })
    }
}

// ── Entregar ────────────────────────────────────────────────────────────────

/// Fallo pasajero (el servicio de push no respondió, la BD falló…): la notificación sigue
/// `pendiente` y se reintenta (NATS la vuelve a dar; si no, la revisión periódica).
#[derive(Debug)]
pub struct Reintentar(pub String);

/// Cuánto guarda el servicio de push el aviso si el celular está apagado.
const TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Manda la notificación `id` al celular de su cliente y la marca. Si ya no está pendiente
/// (otra tarea la mandó, o el Id no existe), no hace nada: se puede llamar de más.
/// `origen` ("nats" o "revisión") solo es para el log: saber quién la entregó.
pub async fn entregar(state: &AppState, vapid: &Vapid, id: Uuid, origen: &str) -> Result<(), Reintentar> {
    let r = |e: sqlx::Error| Reintentar(format!("BD: {e}"));
    // La transacción mantiene la fila bloqueada mientras se manda (ver `tomar`).
    let mut tx = state.pool.begin().await.map_err(r)?;
    let Some(aviso) = db::notificaciones::tomar(&mut tx, id).await.map_err(r)? else {
        return Ok(());
    };
    let Some(destino) = db::notificaciones::destino(&mut tx, aviso.cliente_id).await.map_err(r)? else {
        db::notificaciones::marcar(&mut tx, id, "sin_app", None, None).await.map_err(r)?;
        tracing::info!("notificaciones: {id} sin_app ({origen})");
        return tx.commit().await.map_err(r);
    };
    let celular = Some(destino.celular_id);

    // Lo que lee el service worker (static/app/sw.js) para mostrar el aviso.
    let cuerpo = serde_json::json!({
        "id": id, "titulo": aviso.titulo, "texto": aviso.texto, "url": aviso.url,
    });
    let peticion = match peticion(vapid, &destino, &state.config.site_url, cuerpo.to_string()) {
        Ok(p) => p,
        Err(e) => {
            // Datos de suscripción que no sirven: no tiene caso reintentar.
            db::notificaciones::marcar(&mut tx, id, "fallida", celular, Some(&e)).await.map_err(r)?;
            return tx.commit().await.map_err(r);
        }
    };

    let respuesta = state.http.execute(peticion).await
        .map_err(|e| Reintentar(format!("servicio de push: {e}")))?;
    let status = respuesta.status();
    if status.is_success() {
        db::notificaciones::marcar(&mut tx, id, "enviada", celular, None).await.map_err(r)?;
        tracing::info!("notificaciones: {id} enviada ({origen})");
    } else if status.as_u16() == 404 || status.as_u16() == 410 {
        // La suscripción ya no existe: desinstaló la app o quitó el permiso.
        db::notificaciones::marcar(&mut tx, id, "fallida", celular,
            Some("La suscripción ya no existe (quitó el permiso o la app)")).await.map_err(r)?;
        db::notificaciones::quitar_push(&mut tx, destino.celular_id).await.map_err(r)?;
        tracing::info!("notificaciones: {id} fallida, suscripción vencida ({origen})");
    } else if status.as_u16() == 429 || status.is_server_error() {
        return Err(Reintentar(format!("servicio de push respondió {status}")));
    } else {
        // 400, 403 (firma VAPID rechazada), 413…: reintentar daría lo mismo.
        let detalle = respuesta.text().await.unwrap_or_default();
        let error = format!("{status}: {}", detalle.chars().take(200).collect::<String>());
        db::notificaciones::marcar(&mut tx, id, "fallida", celular, Some(&error)).await.map_err(r)?;
        tracing::warn!("notificaciones: {id} fallida: {error} ({origen})");
    }
    tx.commit().await.map_err(r)
}

/// Cifra el aviso para ese celular (aes128gcm con sus llaves) y lo firma con VAPID.
fn peticion(vapid: &Vapid, d: &Destino, contacto: &str, cuerpo: String) -> Result<reqwest::Request, String> {
    let p256dh = Base64UrlUnpadded::decode_vec(&d.p256dh).map_err(|e| format!("p256dh: {e}"))?;
    let auth = Base64UrlUnpadded::decode_vec(&d.auth).map_err(|e| format!("auth: {e}"))?;
    if auth.len() != 16 {
        return Err("auth no mide 16 bytes".into());
    }
    let http = WebPushBuilder::new(
        d.endpoint.parse().map_err(|e| format!("endpoint: {e}"))?,
        PublicKey::from_sec1_bytes(&p256dh).map_err(|e| format!("p256dh: {e}"))?,
        Auth::clone_from_slice(&auth),
    )
    .with_valid_duration(TTL)
    .with_vapid(&vapid.par, contacto)
    .build(cuerpo)
    .map_err(|e| format!("cifrar: {e}"))?;
    reqwest::Request::try_from(http).map_err(|e| format!("petición: {e}"))
}

// ── Vigilar: NATS y la revisión periódica ───────────────────────────────────

/// Arranca las dos tareas que entregan avisos. Sin llave VAPID no hace nada; sin usuario de
/// NATS, solo la revisión periódica (llegan con hasta dos minutos de retraso).
pub fn vigilar(state: AppState) {
    let Some(vapid) = state.vapid.clone() else { return };
    if state.config.nats_usuario.is_empty() {
        tracing::warn!("notificaciones: sin NATS_USUARIO, solo la revisión cada 2 minutos");
    } else {
        tokio::spawn(escuchar_nats(state.clone(), vapid.clone()));
    }
    tokio::spawn(revisar(state, vapid));
}

/// Lee el consumer `xplaya` del stream NOTIFICACIONES (los crea `ro` a mano; ver
/// k3s-manifests/CLAUDE.md, sección NATS). Si se cae la conexión, vuelve a empezar.
async fn escuchar_nats(state: AppState, vapid: Arc<Vapid>) {
    loop {
        if let Err(e) = escuchar_una_vez(&state, &vapid).await {
            tracing::warn!("notificaciones: NATS: {e}; reintento en 10 s");
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn escuchar_una_vez(state: &AppState, vapid: &Vapid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = &state.config;
    let cliente = async_nats::ConnectOptions::with_user_and_password(
        cfg.nats_usuario.clone(),
        cfg.nats_password.clone(),
    )
    .name("xplaya")
    .connect(&cfg.nats_url)
    .await?;
    let js = async_nats::jetstream::new(cliente);
    let consumer: PullConsumer = js.get_consumer_from_stream("xplaya", "NOTIFICACIONES").await?;
    // `messages()` pide mensajes al consumer en tandas y los va dando conforme llegan.
    let mut mensajes = consumer.messages().await?;
    tracing::info!("notificaciones: escuchando NATS (NOTIFICACIONES > xplaya)");

    while let Some(mensaje) = mensajes.next().await {
        let mensaje = mensaje?;
        // El mensaje es solo el Id de la fila; lo demás está en la tabla.
        let id = std::str::from_utf8(&mensaje.payload).ok().and_then(|t| t.trim().parse::<Uuid>().ok());
        let resultado = match id {
            Some(id) => entregar(state, vapid, id, "nats").await,
            None => {
                // Basura (o una prueba a mano): se confirma para que no vuelva.
                tracing::warn!("notificaciones: mensaje que no es un Id, se descarta: {:?}",
                    String::from_utf8_lossy(&mensaje.payload));
                Ok(())
            }
        };
        match resultado {
            Ok(()) => mensaje.ack().await?,
            Err(Reintentar(e)) => {
                tracing::warn!("notificaciones: {id:?}: {e}; NATS la vuelve a dar en 30 s");
                mensaje.ack_with(AckKind::Nak(Some(Duration::from_secs(30)))).await?;
            }
        }
    }
    Ok(())
}

/// Red de seguridad: cada 2 minutos manda las pendientes que NATS no trajo (el POS no pudo
/// publicar, o se acabaron los 5 intentos) y vence las de más de un día.
async fn revisar(state: AppState, vapid: Arc<Vapid>) {
    let mut cada = tokio::time::interval(Duration::from_secs(120));
    loop {
        cada.tick().await;
        match db::notificaciones::rezagadas(&state.pool).await {
            Ok(ids) => {
                for id in ids {
                    if let Err(Reintentar(e)) = entregar(&state, &vapid, id, "revisión").await {
                        tracing::warn!("notificaciones: rezagada {id}: {e}");
                    }
                }
            }
            Err(e) => tracing::error!("notificaciones: rezagadas: {e}"),
        }
        match db::notificaciones::vencer(&state.pool).await {
            Ok(0) => {}
            Ok(n) => tracing::warn!("notificaciones: {n} vencidas sin entregar"),
            Err(e) => tracing::error!("notificaciones: vencer: {e}"),
        }
    }
}
