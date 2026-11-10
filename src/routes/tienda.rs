//! Tienda dentro de la app de clientes (app.xplaya.com): catálogo, detalle y carrito
//! con la apariencia de la app y la barra de abajo (Monedero · Tienda · Carrito).
//!
//! Reusa las consultas del sitio (`db::productos`). La diferencia está en el pedido: con
//! la sesión del celular ligado (passkey) ya se sabe quién es, así que no pide nombre ni
//! teléfono y el pedido entra a nombre de ese cliente, con su teléfono ya comprobado.
//! Sin sesión se puede ver todo, pero para mandar el pedido hay que entrar con la huella.
//!
//! El carrito vive en el `localStorage` del celular (`static/js/cart.js`, el mismo del
//! sitio; cada dominio guarda el suyo).

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use minijinja::{context, Value};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    db,
    models::pedido::PedidoItemRequest,
    routes::{alta::error_json, app::cliente_sesion, productos::CatalogoParams},
    AppState,
};

fn render(state: &AppState, plantilla: &str, ctx: Value) -> Result<Html<String>, StatusCode> {
    state
        .tmpl
        .get_template(plantilla)
        .and_then(|t| t.render(ctx))
        .map(Html)
        .map_err(|e| {
            tracing::error!("Render {plantilla}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

// ── GET /tienda ─────────────────────────────────────────────────────────────

pub async fn lista(
    State(state): State<AppState>,
    Query(params): Query<CatalogoParams>,
) -> Result<Html<String>, StatusCode> {
    let pagina = params.pagina.unwrap_or(1).max(1);
    let busqueda = params.busqueda.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let (productos, paginacion) =
        db::productos::busqueda(&state.pool, busqueda, pagina, &state.config.content_base_url)
            .await
            .map_err(|e| {
                tracing::error!("Catálogo de la app: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    render(&state, "app/tienda/lista.html", context! {
        en_app => true,
        nav => "tienda",
        productos,
        paginacion,
        busqueda => busqueda.unwrap_or(""),
    })
}

// ── GET /tienda/{nid} ───────────────────────────────────────────────────────

pub async fn detalle(State(state): State<AppState>, Path(nid): Path<i32>) -> Result<Html<String>, StatusCode> {
    let producto = db::productos::detalle(&state.pool, nid, &state.config.content_base_url)
        .await
        .map_err(|e| {
            tracing::error!("Detalle {nid} en la app: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    render(&state, "app/tienda/detalle.html", context! {
        en_app => true,
        nav => "tienda",
        producto,
    })
}

// ── GET /carrito ────────────────────────────────────────────────────────────

/// Con sesión muestra a nombre de quién va el pedido; sin sesión, el botón de huella.
pub async fn carrito(State(state): State<AppState>, headers: HeaderMap) -> Result<Html<String>, StatusCode> {
    let nombre = match cliente_sesion(&state, &headers).await {
        Some(id) => db::altas::nombre_cliente(&state.pool, id).await.unwrap_or_else(|e| {
            tracing::error!("Nombre del cliente {id}: {e}");
            None
        }),
        None => None,
    };
    let con_sesion = nombre.is_some();
    render(&state, "app/tienda/carrito.html", context! {
        en_app => true,
        nav => "carrito",
        con_sesion,
        nombre,
    })
}

// ── POST /tienda/pedido ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PedidoApp {
    pub items: Vec<PedidoItemRequest>,
}

/// Crea el pedido a nombre del cliente de la sesión. Responde la URL de la cotización en
/// xplaya.com; los errores van como `{ "error": "..." }`, como el resto de la app.
pub async fn crear_pedido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<PedidoApp>,
) -> Response {
    let Some(cliente_id) = cliente_sesion(&state, &headers).await else {
        return error_json(StatusCode::UNAUTHORIZED, "Entra con tu huella para mandar tu pedido.");
    };
    if req.items.is_empty() {
        return error_json(StatusCode::BAD_REQUEST, "Tu carrito está vacío.");
    }
    if req.items.iter().any(|i| i.cantidad <= Decimal::ZERO) {
        return error_json(StatusCode::BAD_REQUEST, "Revisa las cantidades de tu carrito.");
    }
    match db::pedidos::crear_para_cliente(&state.pool, cliente_id, &req.items).await {
        Ok(uid) => Json(serde_json::json!({
            "cotizacion": format!("{}/cotizacion/{uid}", state.config.site_url),
        }))
        .into_response(),
        Err(e) => {
            tracing::error!("Pedido de la app del cliente {cliente_id}: {e}");
            error_json(StatusCode::INTERNAL_SERVER_ERROR, "No pudimos mandar tu pedido. Intenta de nuevo.")
        }
    }
}
