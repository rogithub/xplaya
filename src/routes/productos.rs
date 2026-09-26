use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Html,
};
use minijinja::context;
use serde::Deserialize;

use crate::{AppState, db::productos as db};

#[derive(Deserialize, Default)]
pub struct CatalogoParams {
    pub pagina: Option<i32>,
    pub busqueda: Option<String>,
}

pub async fn lista(
    State(state): State<AppState>,
    Query(params): Query<CatalogoParams>,
    headers: HeaderMap,
) -> Result<Html<String>, StatusCode> {
    let pagina = params.pagina.unwrap_or(1);
    let busqueda = params.busqueda.as_deref().filter(|s| !s.is_empty());

    let (productos, paginacion) = db::busqueda(
        &state.pool,
        busqueda,
        pagina,
        &state.config.content_base_url,
    )
    .await
    .map_err(|e| {
        tracing::error!("Error en catálogo: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Si HTMX hace el request (búsqueda o paginación), devuelve solo el grid.
    // Si es una carga normal del navegador, devuelve la página completa.
    let es_htmx = headers.contains_key("hx-request");
    let template = if es_htmx { "productos/partials/grid.html" } else { "productos/lista.html" };

    let html = state
        .tmpl
        .get_template(template)
        .and_then(|t| {
            t.render(context! {
                productos,
                paginacion,
                busqueda => busqueda.unwrap_or(""),
            })
        })
        .map_err(|e| {
            tracing::error!("Error de template: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Html(html))
}

pub async fn detalle(
    State(state): State<AppState>,
    Path(nid): Path<i32>,
) -> Result<Html<String>, StatusCode> {
    let producto = db::detalle(&state.pool, nid, &state.config.content_base_url)
        .await
        .map_err(|e| {
            tracing::error!("Error en detalle producto {}: {}", nid, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let html = state
        .tmpl
        .get_template("productos/detalle.html")
        .and_then(|t| t.render(context! { producto }))
        .map_err(|e| {
            tracing::error!("Error de template detalle: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Html(html))
}
