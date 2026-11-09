//! App de clientes en app.xplaya.com (PWA del monedero).
//!
//! `main.rs` manda aquí las peticiones cuyo `Host` es el de `APP_URL`; el resto va
//! al router del sitio. Hasta que existan las passkeys (paso 3), la app sabe de quién
//! es el monedero por la URL `/monedero/{guid}` y una cookie que la recuerda.

use axum::{
    extract::{Form, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use minijinja::context;
use serde::Deserialize;
use uuid::Uuid;

use crate::{db, routes::monedero, AppState};

/// Cookie con el GUID del último monedero abierto en este celular.
const COOKIE_MONEDERO: &str = "xp_monedero";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(inicio))
        .route("/entrar", post(entrar))
        .route("/monedero/{cliente_id}", get(monedero))
        .route("/monedero/{cliente_id}/manifest.webmanifest", get(manifest))
        .route("/sw.js", get(service_worker))
        .route("/robots.txt", get(robots_txt))
        .fallback(|| async { Redirect::to("/") })
}

// ── / ───────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct InicioParams {
    pub error: Option<String>,
}

/// Si el celular ya abrió un monedero, va directo a él; si no, pide el teléfono.
async fn inicio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<InicioParams>,
) -> Result<Response, StatusCode> {
    if let Some(id) = cookie_monedero(&headers) {
        return Ok(Redirect::to(&format!("/monedero/{id}")).into_response());
    }

    let tmpl = state.tmpl.get_template("app/inicio.html").map_err(|e| {
        tracing::error!("Template app/inicio: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! { error => params.error, en_app => true })
        .map_err(|e| {
            tracing::error!("Render app/inicio: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html).into_response())
}

/// Lee el GUID de la cookie `xp_monedero`, si existe y es válido.
fn cookie_monedero(headers: &HeaderMap) -> Option<Uuid> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|par| par.trim().split_once('='))
        .find(|(nombre, _)| *nombre == COOKIE_MONEDERO)
        .and_then(|(_, valor)| valor.parse().ok())
}

// ── POST /entrar ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EntrarForm {
    pub telefono: String,
}

/// Igual que `/saldo` en xplaya.com, pero los errores vuelven a la portada de la app.
async fn entrar(
    State(state): State<AppState>,
    Form(form): Form<EntrarForm>,
) -> Result<Redirect, StatusCode> {
    let Some(tel) = monedero::normalizar_telefono(&form.telefono) else {
        return Ok(Redirect::to("/?error=Escribe+tu+teléfono+a+10+dígitos."));
    };

    let cliente_id = db::monedero::buscar_cliente(&state.pool, &tel)
        .await
        .map_err(|e| {
            tracing::error!("buscar_cliente (app): {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match cliente_id {
        Some(id) => Ok(Redirect::to(&format!("/monedero/{id}"))),
        None => Ok(Redirect::to(
            "/?error=Ese+teléfono+no+tiene+monedero.+Pídelo+en+caja+en+tu+próxima+compra.",
        )),
    }
}

// ── /monedero/:cliente_id ───────────────────────────────────────────────────

/// La misma tarjeta que en xplaya.com, con la PWA activada. Guarda el GUID en una
/// cookie de un año para que `/` lleve directo aquí.
async fn monedero(
    State(state): State<AppState>,
    Path(cliente_id): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let html = match monedero::render_monedero(&state, cliente_id, true).await {
        Ok(html) => html,
        // GUID inexistente (o el cliente salió del programa): se borra la cookie para
        // que `/` no vuelva a mandar aquí, y se muestra la portada.
        Err(StatusCode::NOT_FOUND) => {
            let borrar = format!("{COOKIE_MONEDERO}=; Path=/; Max-Age=0");
            return Ok(([(header::SET_COOKIE, borrar)], Redirect::to("/")).into_response());
        }
        Err(e) => return Err(e),
    };

    // HttpOnly: el JavaScript de la página no la necesita. Secure solo con HTTPS,
    // para que funcione en http://app.localhost durante el desarrollo.
    let secure = if state.config.app_url.starts_with("https://") { "; Secure" } else { "" };
    let cookie = format!(
        "{COOKIE_MONEDERO}={cliente_id}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax{secure}"
    );
    Ok(([(header::SET_COOKIE, cookie)], html).into_response())
}

// ── /monedero/:cliente_id/manifest.webmanifest ──────────────────────────────

/// Manifest de la PWA. Es por cliente para que, al abrir la app instalada, entre
/// directo a su monedero (`start_url`), aun en iPhone, donde la app instalada no
/// comparte cookies con Safari. `id` es fijo: para el celular siempre es la misma app.
async fn manifest(Path(cliente_id): Path<Uuid>) -> Response {
    let manifest = serde_json::json!({
        "id": "/",
        "name": "Monedero xplaya",
        "short_name": "xplaya",
        "description": "Tu monedero de la Papelería xplaya: saldo, vencimientos y código para caja.",
        "lang": "es-MX",
        "start_url": format!("/monedero/{cliente_id}"),
        "scope": "/",
        "display": "standalone",
        "orientation": "portrait",
        "background_color": "#231916",
        "theme_color": "#231916",
        "icons": [
            { "src": "/static/img/favicon/web-app-manifest-192x192.png", "sizes": "192x192", "type": "image/png", "purpose": "any" },
            { "src": "/static/img/favicon/web-app-manifest-512x512.png", "sizes": "512x512", "type": "image/png", "purpose": "any" },
            { "src": "/static/img/favicon/web-app-manifest-512x512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }
        ]
    });
    (
        [
            (header::CONTENT_TYPE, "application/manifest+json"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        manifest.to_string(),
    )
        .into_response()
}

// ── /sw.js ──────────────────────────────────────────────────────────────────

/// El service worker tiene que servirse desde la raíz para controlar todo el sitio
/// (`scope: /`); por eso no va en `/static`. `no-cache` para que el navegador vea
/// cada versión nueva en cuanto se despliega.
async fn service_worker() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        include_str!("../../static/app/sw.js"),
    )
        .into_response()
}

// ── /robots.txt ─────────────────────────────────────────────────────────────

/// La app solo tiene datos personales: nada que indexar.
async fn robots_txt() -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "User-agent: *\nDisallow: /\n",
    )
        .into_response()
}
