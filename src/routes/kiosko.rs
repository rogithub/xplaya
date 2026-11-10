//! Kiosko de la tienda: una pantalla del lado de los clientes (Chromium en modo kiosko)
//! con un QR que cambia cada 30 s. El cliente lo escanea con su celular y liga su monedero.
//!
//! | Teléfono que escribe | Qué pasa |
//! |---|---|
//! | Nuevo | crea su passkey → se crea su monedero con el celular ligado |
//! | Ya tiene monedero | aviso de ntfy a la caja con Aprobar / Rechazar; al aprobar, crea su passkey |
//!
//! La pantalla se habilita una vez con `/kiosko/activar?t=TOKEN`, que deja el token en una
//! cookie HttpOnly (la URL con el token solo vive en el autostart de la Raspberry). El mismo
//! token firma los QR (`kiosko.rs`), así que nadie fuera de la tienda genera uno válido.
//!
//! Todo lo intermedio vive en memoria, como el alta del recibo: si el pod se reinicia, el
//! cliente vuelve a escanear.

use axum::{
    extract::{Form, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use minijinja::context;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    db, kiosko,
    routes::{
        alta::{self, AltaPendiente, Origen, Quien},
        monedero::{normalizar_telefono, qr_svg},
    },
    AppState,
};

const COOKIE_KIOSKO: &str = "kiosko_token";
const TEMA_NTFY: &str = "papeleria-altas";

/// Teléfono que ya tiene monedero, esperando a que la caja confirme que es esa persona.
#[derive(Clone)]
pub struct Espera {
    pub visita: Uuid,
    pub cliente_id: Uuid,
    pub estado: EstadoEspera,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EstadoEspera {
    Pendiente,
    Aprobada,
    Rechazada,
}

// ── Pantalla del kiosko ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ActivarParams {
    t: Option<String>,
}

/// Habilita el navegador del kiosko: guarda el token en la cookie y abre la pantalla.
pub async fn activar(State(state): State<AppState>, Query(params): Query<ActivarParams>) -> Response {
    let esperado = &state.config.kiosko_token;
    if esperado.is_empty() || params.t.as_deref() != Some(esperado.as_str()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let secure = if state.config.app_url.starts_with("https://") { "; Secure" } else { "" };
    let cookie = format!(
        "{COOKIE_KIOSKO}={esperado}; Path=/kiosko; Max-Age=31536000; HttpOnly; SameSite=Lax{secure}"
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to("/kiosko")).into_response()
}

fn es_kiosko(state: &AppState, headers: &HeaderMap) -> bool {
    let esperado = &state.config.kiosko_token;
    !esperado.is_empty()
        && headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .filter_map(|par| par.trim().split_once('='))
            .any(|(n, v)| n == COOKIE_KIOSKO && v == esperado)
}

pub async fn pantalla(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, StatusCode> {
    if !es_kiosko(&state, &headers) {
        return Ok((StatusCode::FORBIDDEN, "Este kiosko no está activado.").into_response());
    }
    let html = state
        .tmpl
        .get_template("app/kiosko.html")
        .and_then(|t| t.render(context! { en_app => true }))
        .map_err(|e| {
            tracing::error!("Render app/kiosko: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html).into_response())
}

/// QR vigente y cuántos milisegundos le quedan, para pedir el siguiente a tiempo.
pub async fn qr(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !es_kiosko(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let (ventana, ms) = kiosko::ventana_actual();
    let codigo = kiosko::codigo(&state.config.kiosko_token, ventana);
    let url = format!("{}/kiosko/k/{codigo}", state.config.app_url);
    match qr_svg(&url) {
        Some(svg) => Json(serde_json::json!({ "svg": svg, "ms": ms })).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ── En el celular del cliente ───────────────────────────────────────────────

/// Lo que abre el QR: si el código está vigente, empieza una visita de 15 minutos.
pub async fn escaneo(State(state): State<AppState>, Path(codigo): Path<String>) -> Result<Response, StatusCode> {
    let (ventana, _) = kiosko::ventana_actual();
    if !kiosko::codigo_valido(&state.config.kiosko_token, &codigo, ventana) {
        return alta::pagina(&state, Origen::Kiosko(Uuid::nil()), context! { paso => "no_disponible" });
    }
    let visita = state.kiosko_visitas.guardar(());
    Ok(Redirect::to(&format!("/kiosko/alta/{visita}")).into_response())
}

pub async fn datos_get(State(state): State<AppState>, Path(visita): Path<Uuid>) -> Result<Response, StatusCode> {
    let paso = if state.kiosko_visitas.ver(visita).is_some() { "datos" } else { "no_disponible" };
    alta::pagina(&state, Origen::Kiosko(visita), context! { paso })
}

#[derive(Deserialize)]
pub struct DatosForm {
    pub nombre: String,
    pub telefono: String,
}

pub async fn datos(
    State(state): State<AppState>,
    Path(visita): Path<Uuid>,
    Form(form): Form<DatosForm>,
) -> Result<Response, StatusCode> {
    let origen = Origen::Kiosko(visita);
    if state.kiosko_visitas.ver(visita).is_none() {
        return alta::pagina(&state, origen, context! { paso => "no_disponible" });
    }

    let nombre: String = form.nombre.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(100).collect();
    let (Some(tel), false) = (normalizar_telefono(&form.telefono), nombre.is_empty()) else {
        return alta::pagina(&state, origen, context! {
            paso => "datos", nombre => form.nombre, telefono => form.telefono,
            error => "Escribe tu nombre y tu teléfono a 10 dígitos.",
        });
    };

    let mut conn = state.pool.acquire().await.map_err(alta::error_bd)?;
    let existente = db::altas::cliente_por_telefono(&mut conn, &tel).await.map_err(alta::error_bd)?;
    drop(conn);
    // La visita se gasta aquí: para empezar otra alta hay que volver a escanear.
    state.kiosko_visitas.tomar(visita);

    let Some(cliente_id) = existente else {
        let alta = AltaPendiente {
            origen,
            celular_id: Uuid::new_v4(),
            nombre,
            quien: Quien::Nuevo { telefono: tel },
        };
        return alta::pagina_acceso(&state, alta);
    };

    // Ya tiene monedero: la caja confirma que es esa persona.
    let espera = state.kiosko_esperas.guardar(Espera { visita, cliente_id, estado: EstadoEspera::Pendiente });
    let token = state.kiosko_aprobaciones.guardar(espera);
    let registrado = db::altas::nombre_cliente(&state.pool, cliente_id)
        .await
        .map_err(alta::error_bd)?
        .unwrap_or_default();
    if let Err(e) = avisar_caja(&state, &registrado, &nombre, &tel, token).await {
        tracing::error!("ntfy kiosko: {e}");
        return alta::pagina(&state, origen, context! {
            paso => "datos", nombre => form.nombre, telefono => form.telefono,
            error => "No pudimos avisar a la caja. Pide ayuda ahí mismo.",
        });
    }
    alta::pagina(&state, origen, context! { paso => "espera", espera => espera.to_string() })
}

/// Aviso a la caja con botones que llaman a xplaya. En el celular del cliente nunca se
/// muestra el nombre registrado: quien no es dueño del teléfono no debe enterarse.
async fn avisar_caja(
    state: &AppState,
    registrado: &str,
    escribio: &str,
    tel: &str,
    token: Uuid,
) -> Result<(), String> {
    let base = &state.config.app_url;
    let mensaje = format!(
        "{registrado} ({}) quiere ligar su celular en el kiosko. Escribió: \"{escribio}\". ¿Es esa persona?",
        kiosko::mascara_telefono(tel)
    );
    let aprobar = format!("{base}/kiosko/aprobar/{token}");
    let rechazar = format!("{base}/kiosko/rechazar/{token}");

    if state.config.ntfy_token.is_empty() {
        tracing::warn!("ntfy sin token, aviso solo en el log: {mensaje} | aprobar: POST {aprobar} | rechazar: POST {rechazar}");
        return Ok(());
    }

    let cuerpo = serde_json::json!({
        "topic": TEMA_NTFY,
        "title": "Kiosko: ¿es el cliente?",
        "message": mensaje,
        "tags": ["iphone"],
        "priority": 4,
        "actions": [
            { "action": "http", "label": "Aprobar", "url": aprobar, "method": "POST", "clear": true },
            { "action": "http", "label": "Rechazar", "url": rechazar, "method": "POST", "clear": true },
        ],
    });
    let r = state
        .http
        .post(&state.config.ntfy_url)
        .bearer_auth(&state.config.ntfy_token)
        .json(&cuerpo)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("ntfy respondió {}", r.status()));
    }
    Ok(())
}

/// Estado de la espera, que el celular consulta cada 2 segundos.
pub async fn espera_estado(State(state): State<AppState>, Path(espera): Path<Uuid>) -> Json<serde_json::Value> {
    let estado = match state.kiosko_esperas.ver(espera).map(|e| e.estado) {
        Some(EstadoEspera::Pendiente) => "pendiente",
        Some(EstadoEspera::Aprobada) => "aprobada",
        Some(EstadoEspera::Rechazada) => "rechazada",
        None => "vencida",
    };
    Json(serde_json::json!({ "estado": estado }))
}

/// Aprobada: sigue al paso de crear la passkey. Rechazada: se lo dice.
pub async fn espera_continuar(State(state): State<AppState>, Path(espera): Path<Uuid>) -> Result<Response, StatusCode> {
    let Some(e) = state.kiosko_esperas.ver(espera) else {
        return alta::pagina(&state, Origen::Kiosko(Uuid::nil()), context! { paso => "no_disponible" });
    };
    let origen = Origen::Kiosko(e.visita);
    match e.estado {
        EstadoEspera::Pendiente => {
            alta::pagina(&state, origen, context! { paso => "espera", espera => espera.to_string() })
        }
        EstadoEspera::Rechazada => alta::pagina(&state, origen, context! { paso => "rechazado" }),
        EstadoEspera::Aprobada => {
            state.kiosko_esperas.tomar(espera);
            let nombre = db::altas::nombre_cliente(&state.pool, e.cliente_id)
                .await
                .map_err(alta::error_bd)?
                .unwrap_or_default();
            alta::pagina_acceso(&state, AltaPendiente {
                origen,
                celular_id: Uuid::new_v4(),
                nombre,
                quien: Quien::Cliente(e.cliente_id),
            })
        }
    }
}

// ── Botones de ntfy (los toca la caja) ──────────────────────────────────────

pub async fn aprobar(State(state): State<AppState>, Path(token): Path<Uuid>) -> Response {
    resolver(&state, token, EstadoEspera::Aprobada)
}

pub async fn rechazar(State(state): State<AppState>, Path(token): Path<Uuid>) -> Response {
    resolver(&state, token, EstadoEspera::Rechazada)
}

/// El token sirve una sola vez, para aprobar o para rechazar.
fn resolver(state: &AppState, token: Uuid, estado: EstadoEspera) -> Response {
    let Some(espera) = state.kiosko_aprobaciones.tomar(token) else {
        return (StatusCode::GONE, "Ya se había respondido o pasó mucho tiempo.").into_response();
    };
    let Some(mut e) = state.kiosko_esperas.tomar(espera) else {
        return (StatusCode::GONE, "El cliente ya no está esperando.").into_response();
    };
    e.estado = estado;
    state.kiosko_esperas.guardar_con(espera, e);
    let texto = if estado == EstadoEspera::Aprobada { "Aprobado" } else { "Rechazado" };
    texto.into_response()
}

// ── Celular sin passkeys ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SoloMonederoForm {
    pub ticket: Uuid,
    pub terminos: Option<String>,
}

/// Teléfono nuevo cuyo celular no puede crear passkeys: se crea su monedero sin ligar el
/// celular. Lo ve en la web, como hasta hoy.
pub async fn solo_monedero(
    State(state): State<AppState>,
    Path(visita): Path<Uuid>,
    Form(form): Form<SoloMonederoForm>,
) -> Result<Response, StatusCode> {
    let origen = Origen::Kiosko(visita);
    let Some(alta) = state.altas.ver(form.ticket).filter(|a| a.origen == origen) else {
        return alta::pagina(&state, origen, context! { paso => "no_disponible" });
    };
    let Quien::Nuevo { telefono } = &alta.quien else {
        return alta::pagina(&state, origen, context! { paso => "no_disponible" });
    };
    if form.terminos.is_none() {
        return alta::pagina_acceso(&state, alta);
    }

    let mut tx = state.pool.begin().await.map_err(alta::error_bd)?;
    if db::altas::cliente_por_telefono(&mut tx, telefono).await.map_err(alta::error_bd)?.is_some() {
        return alta::pagina(&state, origen, context! { paso => "no_disponible" });
    }
    let cliente_id = db::altas::crear_cliente(&mut tx, &alta.nombre, telefono).await.map_err(alta::error_bd)?;
    tx.commit().await.map_err(alta::error_bd)?;
    state.altas.tomar(form.ticket);

    alta::pagina(&state, origen, context! {
        paso => "abonado_nuevo",
        nombre => alta::primer_nombre(&alta.nombre),
        cliente_id => cliente_id.to_string(),
    })
}
