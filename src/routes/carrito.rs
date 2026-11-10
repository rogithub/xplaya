use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Html,
};
use minijinja::context;

use crate::{
    AppState,
    db::{self, pedidos},
    models::pedido::{CrearPedidoRequest, PedidoCreadoResponse},
    routes::app::cliente_sesion,
};

/// Si el celular ya entró con su huella en la app, la cookie de sesión también llega aquí
/// (es de `xplaya.com`): el carrito muestra a nombre de quién va y no pide datos.
pub async fn pagina(State(state): State<AppState>, headers: HeaderMap) -> Result<Html<String>, StatusCode> {
    let nombre = match cliente_sesion(&state, &headers).await {
        Some(id) => db::altas::nombre_cliente(&state.pool, id).await.unwrap_or_else(|e| {
            tracing::error!("Nombre del cliente {id}: {e}");
            None
        }),
        None => None,
    };
    let html = state
        .tmpl
        .get_template("carrito/index.html")
        .and_then(|t| t.render(context! { nombre }))
        .map_err(|e| {
            tracing::error!("Error template carrito: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html))
}

pub async fn crear_pedido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CrearPedidoRequest>,
) -> Result<Json<PedidoCreadoResponse>, (StatusCode, String)> {
    if req.items.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "El carrito está vacío.".into()));
    }
    // Con sesión de la app: a nombre de ese cliente, sin nombre ni teléfono.
    if let Some(cliente_id) = cliente_sesion(&state, &headers).await {
        let pedido_uid = pedidos::crear_para_cliente(&state.pool, cliente_id, &req.items)
            .await
            .map_err(|e| {
                tracing::error!("Error creando pedido con sesión: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Error interno del servidor.".into())
            })?;
        return Ok(Json(PedidoCreadoResponse { pedido_uid, cliente_id }));
    }

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

    let (pedido_uid, cliente_id) = pedidos::crear(&state.pool, req.nombre.trim(), &tel, &req.items)
        .await
        .map_err(|e| {
            tracing::error!("Error creando pedido: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error interno del servidor.".into())
        })?;

    Ok(Json(PedidoCreadoResponse { pedido_uid, cliente_id }))
}
