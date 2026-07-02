use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Html,
};
use minijinja::context;
use uuid::Uuid;

use crate::{
    AppState,
    db::{kiosko as db, productos as db_productos},
    routes::productos::CatalogoParams,
};

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
