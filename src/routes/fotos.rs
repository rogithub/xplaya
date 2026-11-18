//! Fotos de credencial y polaroid desde la app (`src/fotos.rs`). Las fotos se hacen en el
//! celular y se mandan **directo a imagina** con un permiso firmado que dice quién es el cliente;
//! aquí solo se sirven las pantallas, el permiso y la lista de pedidos (al día por NATS).

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use minijinja::context;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{db, routes::app::cliente_sesion, AppState};

/// Sin llave de fotos, la sección no existe; sin sesión, a la portada (ahí se entra).
#[allow(clippy::result_large_err)]
async fn requisitos(state: &AppState, headers: &HeaderMap) -> Result<Uuid, Response> {
    if state.fotos.is_none() {
        return Err(StatusCode::NOT_FOUND.into_response());
    }
    cliente_sesion(state, headers).await.ok_or_else(|| Redirect::to("/").into_response())
}

fn render(state: &AppState, plantilla: &str, ctx: minijinja::Value) -> Response {
    match state.tmpl.get_template(plantilla).and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("Render {plantilla}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Un pedido como lo ve la plantilla.
#[derive(Serialize)]
struct Vista {
    codigo: String,
    que: &'static str,
    /// "espera" | "listo" | "cancelado": el color de la etiqueta.
    clase: &'static str,
    etiqueta: &'static str,
    fecha: String,
}

fn vista(p: db::fotos::PedidoFoto) -> Vista {
    let que = match p.tipo.as_str() {
        "carnet" => "Foto de credencial",
        "polaroid-grid" => "Fotos polaroid",
        _ => "Fotos",
    };
    // "impreso" y "borrado" son lo mismo para el cliente: la operadora ya terminó.
    let (clase, etiqueta) = match p.estado.as_str() {
        "impreso" | "borrado" => ("listo", "✅ Terminado"),
        "cancelado" => ("cancelado", "Cancelado"),
        _ => ("espera", "📥 Lo recibimos"),
    };
    Vista { codigo: p.codigo, que, clase, etiqueta, fecha: p.fechacreado.format("%d/%m %H:%M").to_string() }
}

#[derive(Deserialize)]
pub struct PaginaParams {
    /// Código del pedido que se acaba de mandar.
    mandado: Option<String>,
}

/// GET /fotos — qué foto hacer y en qué van sus pedidos.
pub async fn pagina(State(state): State<AppState>, headers: HeaderMap, Query(p): Query<PaginaParams>) -> Response {
    let cliente_id = match requisitos(&state, &headers).await {
        Ok(id) => id,
        Err(r) => return r,
    };
    let pedidos = match db::fotos::de_cliente(&state.pool, cliente_id).await {
        Ok(p) => p.into_iter().map(vista).collect::<Vec<_>>(),
        Err(e) => {
            tracing::error!("BD fotos: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    // Recién mandado, NATS tarda un instante en traerlo: se recarga sola mientras no aparezca.
    let falta_el_mandado = p.mandado.as_ref().is_some_and(|c| !pedidos.iter().any(|v| &v.codigo == c));
    render(&state, "app/fotos.html", context! {
        en_app => true,
        nav => "fotos",
        pedidos,
        mandado => p.mandado,
        falta_el_mandado,
    })
}

/// GET /fotos/credencial — tamaño, cámara con guías, ajuste y mandar (`fotoCredencial` en app.js).
pub async fn credencial(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(r) = requisitos(&state, &headers).await {
        return r;
    }
    render(&state, "app/fotos_credencial.html", context! { en_app => true })
}

/// GET /fotos/polaroid — formato, fotos, texto y encuadre, y mandar (`fotoPolaroid` en
/// static/app/polaroid.js).
pub async fn polaroid(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(r) = requisitos(&state, &headers).await {
        return r;
    }
    render(&state, "app/fotos_polaroid.html", context! { en_app => true })
}

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
