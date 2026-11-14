//! "Entrar con Google" en la app (`google.rs` habla con Keycloak).
//!
//! - `POST /google/alta`: desde el paso "Crear acceso" del alta (recibo o kiosko), cuando el
//!   celular no pudo crear la passkey. Lleva el ticket del alta pendiente.
//! - `GET /google/entrar`: volver a entrar a la app con Google (sesión perdida).
//! - `GET /google/regreso`: a donde Keycloak devuelve al celular. Termina el alta (como
//!   `alta::registro_fin`, pero con la cuenta de Google en lugar de la passkey) o abre la sesión.

use axum::{
    extract::{Form, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use minijinja::context;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    db,
    google::Destino,
    passkeys,
    routes::{alta, app},
    AppState,
};

// ── POST /google/alta ───────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AltaForm {
    ticket: Uuid,
    /// "si" si marcó la casilla de los términos (no llega si no la marcó).
    terminos: Option<String>,
}

pub async fn alta(State(state): State<AppState>, Form(form): Form<AltaForm>) -> Result<Response, StatusCode> {
    let Some(alta) = state.altas.ver(form.ticket) else {
        return Ok(app::inicio_con_error("Pasó mucho tiempo. Vuelve a empezar."));
    };
    if alta.quien.pide_terminos() && form.terminos.as_deref() != Some("si") {
        return pagina_error(&state, form.ticket, &alta, "Acepta los términos del monedero para continuar.");
    }
    Ok(ir_a_google(&state, Destino::Alta { ticket: form.ticket }))
}

// ── GET /google/entrar ──────────────────────────────────────────────────────

pub async fn entrar(State(state): State<AppState>) -> Response {
    ir_a_google(&state, Destino::Entrar)
}

/// Guarda la ida y manda al celular a Keycloak (que lo manda directo a Google).
fn ir_a_google(state: &AppState, destino: Destino) -> Response {
    let Some(google) = &state.google else {
        return Redirect::to("/").into_response();
    };
    let ida = google.ida(destino);
    let id = state.google_idas.guardar(ida.clone());
    Redirect::to(&google.url_ida(id, &ida)).into_response()
}

// ── GET /google/regreso ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct Regreso {
    state: Option<Uuid>,
    code: Option<String>,
    /// Keycloak manda `error` (p. ej. `access_denied`) si el cliente canceló en Google.
    error: Option<String>,
}

pub async fn regreso(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Regreso>,
) -> Result<Response, StatusCode> {
    let (Some(google), Some(id)) = (&state.google, q.state) else {
        return Ok(Redirect::to("/").into_response());
    };
    // Solo se puede usar una vez: si alguien recarga la página del regreso, ya no sirve.
    let Some(ida) = state.google_idas.tomar(id) else {
        return Ok(app::inicio_con_error("Pasó mucho tiempo. Intenta de nuevo."));
    };

    let sub = match (&q.code, &q.error) {
        (Some(code), None) => match google.cuenta(&state.http, code, &ida).await {
            Ok(sub) => Some(sub),
            Err(e) => {
                tracing::error!("google: {e}");
                None
            }
        },
        _ => {
            tracing::info!("google: regreso sin code ({:?})", q.error);
            None
        }
    };

    match ida.destino {
        Destino::Entrar => entrar_con(&state, sub).await,
        Destino::Alta { ticket } => alta_con(&state, &headers, ticket, sub).await,
    }
}

/// Abre la sesión del celular ligado a esa cuenta de Google.
async fn entrar_con(state: &AppState, sub: Option<String>) -> Result<Response, StatusCode> {
    let Some(sub) = sub else {
        return Ok(app::inicio_con_error("No se pudo entrar con Google. Intenta de nuevo."));
    };
    let mut conn = state.pool.acquire().await.map_err(alta::error_bd)?;
    let Some((celular_id, _)) = db::celulares::por_google(&mut conn, &sub).await.map_err(alta::error_bd)? else {
        return Ok(app::inicio_con_error("Esa cuenta de Google no está ligada a un monedero. Liga tu celular en la tienda o desde un recibo."));
    };
    db::celulares::registrar_uso(&state.pool, celular_id, None).await.map_err(alta::error_bd)?;
    let token = db::celulares::crear_sesion(&mut conn, celular_id).await.map_err(alta::error_bd)?;
    Ok(([(header::SET_COOKIE, app::cookie_sesion(state, &token))], Redirect::to("/")).into_response())
}

/// Termina el alta pendiente con la cuenta de Google, como `alta::registro_fin` con la passkey.
async fn alta_con(
    state: &AppState,
    headers: &HeaderMap,
    ticket: Uuid,
    sub: Option<String>,
) -> Result<Response, StatusCode> {
    let Some(alta) = state.altas.ver(ticket) else {
        return Ok(app::inicio_con_error("Pasó mucho tiempo. Vuelve a empezar."));
    };
    let Some(sub) = sub else {
        return pagina_error(state, ticket, &alta, "No se pudo entrar con Google. Intenta de nuevo.");
    };

    // Una cuenta de Google, un monedero: si ya está ligada a otro cliente, no se mueve.
    let mut conn = state.pool.acquire().await.map_err(alta::error_bd)?;
    if let Some((_, otro)) = db::celulares::por_google(&mut conn, &sub).await.map_err(alta::error_bd)? {
        let mismo = matches!(alta.quien, alta::Quien::Cliente { id, .. } if id == otro);
        if !mismo {
            return pagina_error(
                state,
                ticket,
                &alta,
                "Esa cuenta de Google ya está ligada a otro monedero. Usa otra cuenta.",
            );
        }
    }
    drop(conn);

    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok());
    let acceso = db::celulares::Acceso::Google(&sub);
    match alta::completar_alta(state, &alta, acceso, passkeys::dispositivo(ua)).await {
        Ok(Some(token)) => {
            state.altas.tomar(ticket);
            let cookie = app::cookie_sesion(state, &token);
            Ok(([(header::SET_COOKIE, cookie)], Redirect::to("/listo")).into_response())
        }
        Ok(None) => pagina_error(
            state,
            ticket,
            &alta,
            "Ya no se pudo completar. Si necesitas ayuda, pregunta en la tienda.",
        ),
        Err(e) => {
            tracing::error!("completar_alta google {:?}: {e}", alta.origen);
            pagina_error(state, ticket, &alta, "Algo falló de nuestro lado. Intenta de nuevo.")
        }
    }
}

/// Vuelve al paso "Crear acceso" del mismo alta, con el mensaje.
fn pagina_error(
    state: &AppState,
    ticket: Uuid,
    alta: &alta::AltaPendiente,
    error: &str,
) -> Result<Response, StatusCode> {
    alta::pagina(state, alta.origen, context! {
        paso => "acceso",
        ticket => ticket.to_string(),
        nombre => alta::primer_nombre(&alta.nombre),
        nuevo => matches!(alta.quien, alta::Quien::Nuevo { .. }),
        pide_terminos => alta.quien.pide_terminos(),
        error,
    })
}
