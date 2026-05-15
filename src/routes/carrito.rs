use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::Html,
};
use minijinja::context;

use crate::{AppState, db::pedidos as db, models::pedido::{CrearPedidoRequest, PedidoCreadoResponse}};

pub async fn pagina(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let html = state
        .tmpl
        .get_template("carrito/index.html")
        .and_then(|t| t.render(context! {}))
        .map_err(|e| {
            tracing::error!("Error template carrito: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html))
}

pub async fn crear_pedido(
    State(state): State<AppState>,
    Json(req): Json<CrearPedidoRequest>,
) -> Result<Json<PedidoCreadoResponse>, (StatusCode, String)> {
    // Normalizar teléfono: solo dígitos, últimos 10
    let tel: String = req.telefono.chars().filter(|c| c.is_ascii_digit()).collect();
    let tel = if tel.len() >= 10 { tel[tel.len() - 10..].to_string() } else { tel };

    if tel.len() != 10 {
        return Err((StatusCode::BAD_REQUEST, "El teléfono debe tener 10 dígitos.".into()));
    }
    if req.nombre.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "El nombre es requerido.".into()));
    }
    if req.items.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "El carrito está vacío.".into()));
    }

    let (pedido_uid, cliente_id) = db::crear(&state.pool, req.nombre.trim(), &tel, &req.items)
        .await
        .map_err(|e| {
            tracing::error!("Error creando pedido: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error interno del servidor.".into())
        })?;

    Ok(Json(PedidoCreadoResponse { pedido_uid, cliente_id }))
}
