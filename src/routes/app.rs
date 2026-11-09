//! App de clientes en app.xplaya.com (PWA del monedero).
//!
//! `main.rs` manda aquí las peticiones cuyo `Host` es el de `APP_URL`; el resto va
//! al router del sitio.
//!
//! La app sabe de quién es el monedero de dos formas:
//! - **Sesión** (`xp_sesion`): el celular está ligado con una passkey (alta desde el
//!   recibo en `routes/alta.rs`; para volver a entrar, `/passkey/entrar/*`). `/` muestra
//!   su monedero.
//! - **Sin sesión**: la URL `/monedero/{guid}` y la cookie `xp_monedero` que la recuerda,
//!   como en el paso 2. Se queda mientras haya clientes sin celular ligado.

use axum::{
    extract::{Form, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use minijinja::context;
use serde::Deserialize;
use tower::ServiceBuilder;
use tower_http::{services::ServeFile, set_header::SetResponseHeaderLayer};
use uuid::Uuid;
use webauthn_rs::prelude::PublicKeyCredential;

use crate::{
    db,
    routes::{alta, alta::error_json, monedero},
    AppState,
};

/// Cookie con el GUID del último monedero abierto en este celular.
const COOKIE_MONEDERO: &str = "xp_monedero";
/// Cookie con el token de sesión del celular ligado (en la BD solo está su hash).
const COOKIE_SESION: &str = "xp_sesion";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(inicio))
        .route("/entrar", post(entrar))
        .route("/salir", post(salir))
        .route("/manifest.webmanifest", get(manifest_sesion))
        .route("/monedero/{cliente_id}", get(monedero))
        .route("/monedero/{cliente_id}/manifest.webmanifest", get(manifest))
        .route("/alta/recibo/{ajuste_id}", get(alta::recibo))
        .route("/alta/recibo/{ajuste_id}/digitos", post(alta::digitos))
        .route("/alta/recibo/{ajuste_id}/datos", post(alta::datos))
        .route("/alta/recibo/{ajuste_id}/abonar", post(alta::abonar))
        .route("/alta/recibo/{ajuste_id}/solo-abonar", post(alta::solo_abonar))
        .route("/passkey/registro/inicio", post(alta::registro_inicio))
        .route("/passkey/registro/fin", post(alta::registro_fin))
        .route("/passkey/entrar/inicio", post(entrar_inicio))
        .route("/passkey/entrar/fin", post(entrar_fin))
        // El service worker tiene que servirse desde la raíz para controlar toda la
        // app (`scope: /`); por eso no va en `/static`. Se lee del disco como el resto
        // de estáticos, con `no-cache` para que cada despliegue se vea de inmediato.
        .route_service(
            "/sw.js",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache"),
                ))
                .service(ServeFile::new("static/app/sw.js")),
        )
        .route("/robots.txt", get(robots_txt))
        .fallback(|| async { Redirect::to("/") })
}

// ── / ───────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct InicioParams {
    pub error: Option<String>,
}

/// Con sesión, el monedero del celular ligado. Sin sesión: si el celular ya abrió un
/// monedero, va directo a él; si no, ofrece entrar con huella o buscar por teléfono.
async fn inicio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<InicioParams>,
) -> Result<Response, StatusCode> {
    if let Some(cliente_id) = cliente_sesion(&state, &headers).await {
        match monedero::render_monedero(&state, cliente_id, true, true).await {
            Ok(html) => return Ok(html.into_response()),
            // El cliente ya no existe o salió del programa: se cierra la sesión.
            Err(StatusCode::NOT_FOUND) => return Ok(salir(State(state), headers).await.into_response()),
            Err(e) => return Err(e),
        }
    }
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

/// Valor de una cookie de la petición.
fn leer_cookie<'a>(headers: &'a HeaderMap, nombre: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|par| par.trim().split_once('='))
        .find(|(n, _)| *n == nombre)
        .map(|(_, valor)| valor)
}

/// Lee el GUID de la cookie `xp_monedero`, si existe y es válido.
fn cookie_monedero(headers: &HeaderMap) -> Option<Uuid> {
    leer_cookie(headers, COOKIE_MONEDERO).and_then(|v| v.parse().ok())
}

/// Cliente de la sesión del celular, si tiene una válida. Un error de BD cuenta como
/// "sin sesión" (se registra en el log): la app sigue funcionando sin ella.
pub async fn cliente_sesion(state: &AppState, headers: &HeaderMap) -> Option<Uuid> {
    let token = leer_cookie(headers, COOKIE_SESION)?;
    db::celulares::cliente_de_sesion(&state.pool, token)
        .await
        .inspect_err(|e| tracing::error!("cliente_de_sesion: {e}"))
        .ok()
        .flatten()
}

/// `Secure` solo con HTTPS, para que las cookies funcionen en http://app.localhost.
fn secure(state: &AppState) -> &'static str {
    if state.config.app_url.starts_with("https://") { "; Secure" } else { "" }
}

/// Cookie de sesión: un año, como la sesión en la BD.
pub fn cookie_sesion(state: &AppState, token: &str) -> String {
    format!(
        "{COOKIE_SESION}={token}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax{}",
        secure(state)
    )
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
/// cookie de un año para que `/` lleve directo aquí. Si es el monedero de la sesión,
/// manda a `/` (pasa al abrir una app instalada antes de ligar el celular).
async fn monedero(
    State(state): State<AppState>,
    Path(cliente_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if cliente_sesion(&state, &headers).await == Some(cliente_id) {
        return Ok(Redirect::to("/").into_response());
    }
    let html = match monedero::render_monedero(&state, cliente_id, true, false).await {
        Ok(html) => html,
        // GUID inexistente (o el cliente salió del programa): se borra la cookie para
        // que `/` no vuelva a mandar aquí, y se muestra la portada.
        Err(StatusCode::NOT_FOUND) => {
            let borrar = format!("{COOKIE_MONEDERO}=; Path=/; Max-Age=0");
            return Ok(([(header::SET_COOKIE, borrar)], Redirect::to("/")).into_response());
        }
        Err(e) => return Err(e),
    };

    // HttpOnly: el JavaScript de la página no la necesita.
    let cookie = format!(
        "{COOKIE_MONEDERO}={cliente_id}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax{}",
        secure(&state)
    );
    Ok(([(header::SET_COOKIE, cookie)], html).into_response())
}

// ── /monedero/:cliente_id/manifest.webmanifest ──────────────────────────────

/// Manifest de la PWA. Es por cliente para que, al abrir la app instalada, entre
/// directo a su monedero (`start_url`), aun en iPhone, donde la app instalada no
/// comparte cookies con Safari. `id` es fijo: para el celular siempre es la misma app.
async fn manifest(Path(cliente_id): Path<Uuid>) -> Response {
    manifest_json(&format!("/monedero/{cliente_id}"))
}

/// Manifest para quien tiene el celular ligado: la app abre en `/`, que muestra su
/// monedero si hay sesión o le pide su huella si no (en iPhone, la primera vez que abre
/// la app instalada).
async fn manifest_sesion() -> Response {
    manifest_json("/")
}

fn manifest_json(start_url: &str) -> Response {
    let manifest = serde_json::json!({
        "id": "/",
        "name": "Monedero xplaya",
        "short_name": "xplaya",
        "description": "Tu monedero de la Papelería xplaya: saldo, vencimientos y código para caja.",
        "lang": "es-MX",
        "start_url": start_url,
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

// ── POST /salir ─────────────────────────────────────────────────────────────

/// Cierra la sesión de este celular (la passkey sigue ligada: puede volver a entrar con
/// su huella) y olvida el último monedero abierto.
async fn salir(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = leer_cookie(&headers, COOKIE_SESION)
        && let Err(e) = db::celulares::cerrar_sesion(&state.pool, token).await
    {
        tracing::error!("cerrar_sesion: {e}");
    }
    let mut resp = Redirect::to("/").into_response();
    for nombre in [COOKIE_SESION, COOKIE_MONEDERO] {
        if let Ok(v) = HeaderValue::from_str(&format!("{nombre}=; Path=/; Max-Age=0")) {
            resp.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    resp
}

// ── POST /passkey/entrar/* ──────────────────────────────────────────────────

/// Reto para entrar con la passkey. No dice de quién: el celular ofrece la que tenga
/// para este dominio (passkey descubrible).
async fn entrar_inicio(State(state): State<AppState>) -> Response {
    match state.webauthn.start_discoverable_authentication() {
        Ok((opciones, estado)) => {
            let id = state.entradas.guardar(estado);
            Json(serde_json::json!({ "id": id, "opciones": opciones })).into_response()
        }
        Err(e) => {
            tracing::error!("start_discoverable_authentication: {e:?}");
            error_json(StatusCode::INTERNAL_SERVER_ERROR, "No se pudo preparar la entrada. Intenta de nuevo.")
        }
    }
}

#[derive(Deserialize)]
struct EntrarFin {
    id: Uuid,
    credencial: PublicKeyCredential,
}

async fn entrar_fin(State(state): State<AppState>, Json(body): Json<EntrarFin>) -> Response {
    let Some(estado) = state.entradas.tomar(body.id) else {
        return error_json(StatusCode::GONE, "Pasó mucho tiempo. Toca el botón otra vez.");
    };
    let sin_monedero = || error_json(
        StatusCode::UNAUTHORIZED,
        "Ese acceso ya no está ligado a un monedero. Vuelve a ligar tu celular desde un recibo.",
    );

    let Ok((_, credential_id)) = state.webauthn.identify_discoverable_authentication(&body.credencial) else {
        return sin_monedero();
    };
    let (celular_id, mut passkey) = match db::celulares::por_credencial(&state.pool, credential_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return sin_monedero(),
        Err(e) => {
            tracing::error!("por_credencial: {e}");
            return error_json(StatusCode::INTERNAL_SERVER_ERROR, "Algo falló de nuestro lado. Intenta de nuevo.");
        }
    };

    let resultado = match state.webauthn.finish_discoverable_authentication_v2(&body.credencial, estado, &passkey) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("finish_discoverable_authentication: {e:?}");
            return error_json(StatusCode::UNAUTHORIZED, "No se pudo comprobar tu huella. Intenta de nuevo.");
        }
    };

    // El contador de la passkey se guarda solo si cambió.
    let cambio = passkey.update_credential(&resultado) == Some(true);
    let sesion = async {
        db::celulares::registrar_uso(&state.pool, celular_id, cambio.then_some(&passkey)).await?;
        let mut conn = state.pool.acquire().await?;
        db::celulares::crear_sesion(&mut conn, celular_id).await
    };
    match sesion.await {
        Ok(token) => {
            let cookie = cookie_sesion(&state, &token);
            ([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ir": "/" }))).into_response()
        }
        Err(e) => {
            tracing::error!("crear_sesion: {e}");
            error_json(StatusCode::INTERNAL_SERVER_ERROR, "Algo falló de nuestro lado. Intenta de nuevo.")
        }
    }
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
