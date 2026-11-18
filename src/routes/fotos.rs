//! Fotos de credencial y polaroid desde la app (`src/fotos.rs`): el pedido se manda directo a
//! imagina con un permiso firmado que dice quién es el cliente.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{db, routes::app::cliente_sesion, AppState};

/// GET /fotos/permiso — `{ permiso, imagina }` para el cliente con sesión. La app lo pide antes
/// de mandar sus fotos (y otra vez si imagina dice que venció).
pub async fn permiso(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(firmador) = &state.fotos else { return StatusCode::NOT_FOUND.into_response() };
    let Some(cliente_id) = cliente_sesion(&state, &headers).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let (nombre, telefono) = match db::fotos::datos_cliente(&state.pool, cliente_id).await {
        Ok(Some(d)) => d,
        Ok(None) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(e) => {
            tracing::error!("fotos permiso: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let permiso = firmador.permiso(
        cliente_id,
        nombre.as_deref().unwrap_or("Cliente de la app"),
        &telefono,
        chrono::Utc::now().timestamp(),
    );
    (
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({ "permiso": permiso, "imagina": state.config.imagina_url })),
    )
        .into_response()
}
