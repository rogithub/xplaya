//! Notificaciones de la app (APP_MONEDERO.md, sección Notificaciones): el celular nos da su
//! suscripción Web Push y la guardamos en `ClienteCelulares`. Mandar avisos es otro paso.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64ct::{Base64UrlUnpadded, Encoding};
use serde::Deserialize;

use crate::{db, routes::alta::error_json, routes::app::token_sesion, AppState};

/// Lo que da `PushSubscription.toJSON()` en el navegador.
#[derive(Deserialize)]
pub struct Suscripcion {
    endpoint: String,
    keys: Llaves,
}

#[derive(Deserialize)]
pub struct Llaves {
    p256dh: String,
    auth: String,
}

/// POST /notificaciones/suscripcion — solo con sesión: la suscripción es del celular ligado.
pub async fn suscribir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(s): Json<Suscripcion>,
) -> Response {
    if state.vapid.is_none() {
        return error_json(StatusCode::NOT_FOUND, "Las notificaciones no están activas.");
    }
    let Some(token) = token_sesion(&headers) else {
        return error_json(StatusCode::UNAUTHORIZED, "Entra con tu huella para activar los avisos.");
    };
    let celular = match db::celulares::celular_de_sesion(&state.pool, token).await {
        Ok(Some(id)) => id,
        Ok(None) => return error_json(StatusCode::UNAUTHORIZED, "Entra con tu huella para activar los avisos."),
        Err(e) => {
            tracing::error!("celular_de_sesion: {e}");
            return error_json(StatusCode::INTERNAL_SERVER_ERROR, "Algo falló. Intenta de nuevo.");
        }
    };
    if !valida(&s) {
        return error_json(StatusCode::BAD_REQUEST, "El celular mandó una suscripción que no entendemos.");
    }
    match db::celulares::guardar_push(&state.pool, celular, &s.endpoint, &s.keys.p256dh, &s.keys.auth).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => {
            tracing::error!("guardar_push: {e}");
            error_json(StatusCode::INTERNAL_SERVER_ERROR, "Algo falló. Intenta de nuevo.")
        }
    }
}

/// Que la podamos usar después: endpoint HTTPS, llave pública P-256 (65 bytes) y secreto de
/// 16 bytes, los dos en base64url. Así no se guarda nada con lo que luego no se pueda cifrar.
fn valida(s: &Suscripcion) -> bool {
    let largo = |b64: &str| Base64UrlUnpadded::decode_vec(b64.trim_end_matches('=')).map(|v| v.len()).ok();
    s.endpoint.starts_with("https://")
        && s.endpoint.len() <= 1000
        && largo(&s.keys.p256dh) == Some(65)
        && largo(&s.keys.auth) == Some(16)
}
