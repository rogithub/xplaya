use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
};
use minijinja::context;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    AppState,
    db::{kiosko as db, pedidos as db_pedidos, productos as db_productos},
    models::pedido::{KioskoPedidoRequest, KioskoPedidoResponse},
    routes::productos::CatalogoParams,
};

const COOKIE_TOKEN: &str = "kiosko_token";

/// Lee el token de la cookie del request. El token nunca aparece en el HTML de
/// las páginas (son públicas) — solo viaja en esta cookie HttpOnly, sembrada
/// por GET /kiosko/activar en el navegador del kiosko físico.
fn token_de_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|c| c.strip_prefix(COOKIE_TOKEN).and_then(|r| r.strip_prefix('=')))
}

#[derive(Deserialize)]
pub struct ActivarParams {
    t: Option<String>,
}

/// Habilita el navegador del kiosko físico: siembra la cookie HttpOnly con el
/// token y redirige al catálogo. La URL con `?t=` solo vive en la configuración
/// de autostart de la Raspberry — nunca en páginas ni en el repo.
pub async fn activar(
    State(state): State<AppState>,
    Query(params): Query<ActivarParams>,
) -> Response {
    let esperado = &state.config.kiosko_token;

    // Fail-closed: sin token configurado no se habilita nada.
    if esperado.is_empty() || params.t.as_deref() != Some(esperado.as_str()) {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Max-Age 1 año; Path=/kiosko limita la cookie a las rutas del kiosko.
    let cookie = format!(
        "{}={}; Path=/kiosko; HttpOnly; SameSite=Lax; Max-Age=31536000",
        COOKIE_TOKEN, esperado
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to("/kiosko")).into_response()
}

/// Crea el pedido del kiosko. Sin datos del cliente: llega al POS a nombre del
/// cliente de sistema "Kiosko en tienda" (ID_CLIENTE_KIOSKO) con Origen=0 y el
/// vendedor captura o verifica al cliente real al cobrar.
pub async fn crear_pedido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<KioskoPedidoRequest>,
) -> Result<Json<KioskoPedidoResponse>, (StatusCode, String)> {
    let esperado = &state.config.kiosko_token;
    let autorizado =
        !esperado.is_empty() && token_de_cookie(&headers) == Some(esperado.as_str());

    if !autorizado {
        return Err((
            StatusCode::FORBIDDEN,
            "Este navegador no está habilitado como kiosko.".into(),
        ));
    }
    if req.items.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "El carrito está vacío.".into()));
    }

    match db_pedidos::crear_kiosko(&state.pool, &req.items).await {
        Ok(Some(pedido_uid)) => Ok(Json(KioskoPedidoResponse { pedido_uid })),
        Ok(None) => {
            tracing::error!("Falta la setting ID_CLIENTE_KIOSKO en la BD");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "El kiosko no está configurado en el sistema.".into(),
            ))
        }
        Err(e) => {
            tracing::error!("Error creando pedido de kiosko: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error interno del servidor.".into(),
            ))
        }
    }
}

pub async fn lista(
    State(state): State<AppState>,
    Query(params): Query<CatalogoParams>,
    headers: HeaderMap,
) -> Result<Html<String>, StatusCode> {
    let pagina = params.pagina.unwrap_or(1);
    let busqueda = params.busqueda.as_deref().filter(|s| !s.is_empty());

    let (productos, paginacion) = db::kiosko_lista(
        &state.pool,
        None,
        busqueda,
        pagina,
        &state.config.content_base_url,
    )
    .await
    .map_err(|e| {
        tracing::error!("Error en catálogo kiosko: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Igual que en /: HTMX (búsqueda o paginación) recibe solo el grid,
    // una carga normal del navegador recibe la página completa.
    let es_htmx = headers.contains_key("hx-request");
    let template = if es_htmx { "kiosko/partials/grid.html" } else { "kiosko/lista.html" };

    // Los tiles viven fuera de #catalogo — solo hacen falta en la carga completa.
    let familias = if es_htmx {
        vec![]
    } else {
        db::familias_semanticas(&state.pool).await.map_err(|e| {
            tracing::error!("Error en familias del kiosko: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let html = state
        .tmpl
        .get_template(template)
        .and_then(|t| {
            t.render(context! {
                productos,
                paginacion,
                familias,
                busqueda => busqueda.unwrap_or(""),
                base_url => "/kiosko",
            })
        })
        .map_err(|e| {
            tracing::error!("Error de template kiosko: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Html(html))
}

/// Detalle táctil — misma query que /productos/{nid}, template propio del kiosko
/// (sin SEO/OG/compartir; botones y tipografía a escala táctil).
pub async fn detalle(
    State(state): State<AppState>,
    Path(nid): Path<i32>,
) -> Result<Html<String>, StatusCode> {
    let producto = db_productos::detalle(&state.pool, nid, &state.config.content_base_url)
        .await
        .map_err(|e| {
            tracing::error!("Error en detalle kiosko {}: {}", nid, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let html = state
        .tmpl
        .get_template("kiosko/detalle.html")
        .and_then(|t| t.render(context! { producto }))
        .map_err(|e| {
            tracing::error!("Error de template detalle kiosko: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Html(html))
}

/// Carrito táctil — el estado vive en localStorage (store de Alpine);
/// el servidor solo entrega el template.
pub async fn carrito(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let html = state
        .tmpl
        .get_template("kiosko/carrito.html")
        .and_then(|t| t.render(context! {}))
        .map_err(|e| {
            tracing::error!("Error de template carrito kiosko: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Html(html))
}

pub async fn categoria(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(params): Query<CatalogoParams>,
    headers: HeaderMap,
) -> Result<Html<String>, StatusCode> {
    let pagina = params.pagina.unwrap_or(1);
    let busqueda = params.busqueda.as_deref().filter(|s| !s.is_empty());

    let nombre = db::familia_nombre(&state.pool, id)
        .await
        .map_err(|e| {
            tracing::error!("Error buscando familia {}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let (productos, paginacion) = db::kiosko_lista(
        &state.pool,
        Some(id),
        busqueda,
        pagina,
        &state.config.content_base_url,
    )
    .await
    .map_err(|e| {
        tracing::error!("Error en categoría kiosko {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let es_htmx = headers.contains_key("hx-request");
    let template = if es_htmx { "kiosko/partials/grid.html" } else { "kiosko/lista.html" };

    let html = state
        .tmpl
        .get_template(template)
        .and_then(|t| {
            t.render(context! {
                productos,
                paginacion,
                familia => context! { nombre },
                busqueda => busqueda.unwrap_or(""),
                base_url => format!("/kiosko/categoria/{}", id),
            })
        })
        .map_err(|e| {
            tracing::error!("Error de template categoría kiosko: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Html(html))
}
