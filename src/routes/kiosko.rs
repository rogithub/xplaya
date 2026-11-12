//! Kiosko de la tienda: una pantalla del lado de los clientes (Chromium en modo kiosko)
//! con un QR que cambia cada 30 s. El cliente lo escanea con su celular y liga su monedero.
//!
//! 1. Escanea el QR → escribe nombre y teléfono → se crea una solicitud (`db/kiosko.rs`).
//! 2. El POS la muestra con un código de 4 dígitos (y llega por ntfy al celular de la papelería,
//!    `avisos.rs`); la caja se lo manda por WhatsApp.
//! 3. El cliente escribe el código: el teléfono es suyo. Crea su passkey y queda ligado (si
//!    el teléfono era nuevo, se crea su monedero en ese momento).
//!
//! La pantalla se habilita una vez con `/kiosko/activar?t=TOKEN`, que deja el token en una
//! cookie HttpOnly (la URL con el token solo vive en el autostart de la Raspberry). El mismo
//! token firma los QR (`kiosko.rs`), así que nadie fuera de la tienda genera uno válido.
//!
//! En el celular, todo cuelga de `/kiosko/s/{id}`: primero `id` es la visita que abrió el QR
//! (en memoria, 15 min) y después la solicitud (en la BD).

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
    db::{self, kiosko::Revision},
    kiosko,
    routes::{
        alta::{self, AltaPendiente, Origen, Quien},
        monedero::{normalizar_telefono, qr_svg},
    },
    AppState,
};

const COOKIE_KIOSKO: &str = "kiosko_token";

/// Formulario de reseña de la ficha de Google de la papelería (el mismo de `/resena`).
const URL_RESENA_GOOGLE: &str = "https://search.google.com/local/writereview?placeid=ChIJVw8jFnxDTo8RnF2hE0foXRw";

/// Kiosko de fotos de imagina (credencial y polaroid), en el celular del cliente.
const URL_IMAGINA_KIOSKO: &str = "https://imagina.xplaya.com/kiosko";

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
    let cfg = &state.config;
    let wifi_qr = (!cfg.kiosko_wifi_ssid.is_empty())
        .then(|| qr_svg(&kiosko::texto_qr_wifi(&cfg.kiosko_wifi_ssid, &cfg.kiosko_wifi_password)))
        .flatten()
        .map(minijinja::Value::from_safe_string);
    let resena_qr = qr_svg(URL_RESENA_GOOGLE).map(minijinja::Value::from_safe_string);
    let fotos_qr = qr_svg(URL_IMAGINA_KIOSKO).map(minijinja::Value::from_safe_string);
    let html = state
        .tmpl
        .get_template("app/kiosko.html")
        .and_then(|t| t.render(context! {
            en_app => true,
            wifi_qr,
            wifi_ssid => &cfg.kiosko_wifi_ssid,
            wifi_password => &cfg.kiosko_wifi_password,
            resena_qr,
            fotos_qr,
        }))
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

/// Productos al azar para la vitrina de la pantalla (foto, nombre y precio; se piden en caja).
/// La pantalla pide otra tanda cada pocos minutos.
pub async fn vitrina(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !es_kiosko(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match db::productos::vitrina(&state.pool, &state.config.content_base_url, 24).await {
        Ok(productos) => Json(productos).into_response(),
        Err(e) => {
            tracing::error!("Vitrina del kiosko: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct WhatsAppJson {
    pub telefono: String,
}

/// Desde la pantalla: "quiero que me agreguen a WhatsApp". El POS lo muestra con el botón
/// para escribirle (solicitud de tipo `whatsapp`).
pub async fn whatsapp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<WhatsAppJson>,
) -> Response {
    if !es_kiosko(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(tel) = normalizar_telefono(&body.telefono) else {
        return alta::error_json(StatusCode::BAD_REQUEST, "Escribe tu teléfono a 10 dígitos.");
    };
    match db::kiosko::crear_whatsapp(&state.pool, &tel).await {
        Ok(()) => {
            // Además del aviso del POS, al celular de la papelería (avisos.rs).
            state.avisos.whatsapp_kiosko(&state.pool, &tel);
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => {
            tracing::error!("kiosko whatsapp: {e}");
            alta::error_json(StatusCode::INTERNAL_SERVER_ERROR, "Algo falló. Pide ayuda en la caja.")
        }
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
    Ok(Redirect::to(&format!("/kiosko/s/{visita}")).into_response())
}

/// Visita abierta → pide los datos. Solicitud esperando el código → pide el código.
pub async fn pagina(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, StatusCode> {
    let origen = Origen::Kiosko(id);
    if state.kiosko_visitas.ver(id).is_some() {
        return alta::pagina(&state, origen, context! { paso => "datos" });
    }
    match db::kiosko::pendiente(&state.pool, id).await.map_err(alta::error_bd)? {
        Some(tel) => pagina_codigo(&state, id, &tel, None),
        None => alta::pagina(&state, origen, context! { paso => "no_disponible" }),
    }
}

fn pagina_codigo(state: &AppState, id: Uuid, tel: &str, error: Option<String>) -> Result<Response, StatusCode> {
    alta::pagina(state, Origen::Kiosko(id), context! {
        paso => "codigo", telefono => kiosko::mascara_telefono(tel), error,
    })
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

    // La visita se gasta aquí: para otra solicitud hay que volver a escanear.
    state.kiosko_visitas.tomar(visita);
    let (solicitud, codigo) = db::kiosko::crear(&state.pool, &nombre, &tel).await.map_err(alta::error_bd)?;
    // Además del aviso del POS, el código le llega al celular de la papelería (avisos.rs).
    state.avisos.alta_kiosko(&nombre, &tel, &codigo);
    Ok(Redirect::to(&format!("/kiosko/s/{solicitud}")).into_response())
}

#[derive(Deserialize)]
pub struct CodigoForm {
    pub codigo: String,
}

/// Código correcto: sigue a crear la passkey, como cliente que ya existe o como nuevo.
pub async fn codigo(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Form(form): Form<CodigoForm>,
) -> Result<Response, StatusCode> {
    let origen = Origen::Kiosko(id);
    let codigo: String = form.codigo.chars().filter(|c| c.is_ascii_digit()).collect();
    let Some(tel) = db::kiosko::pendiente(&state.pool, id).await.map_err(alta::error_bd)? else {
        return alta::pagina(&state, origen, context! { paso => "no_disponible" });
    };
    if codigo.len() != 4 {
        return pagina_codigo(&state, id, &tel, Some("Escribe los 4 dígitos que te mandamos.".into()));
    }

    let (nombre, telefono) = match db::kiosko::revisar_codigo(&state.pool, id, &codigo).await.map_err(alta::error_bd)? {
        Revision::Correcta { nombre, telefono } => (nombre, telefono),
        Revision::Incorrecta { quedan } => {
            let error = if quedan == 1 {
                "Ese no es. Te queda 1 intento.".to_string()
            } else {
                format!("Ese no es. Te quedan {quedan} intentos.")
            };
            return pagina_codigo(&state, id, &tel, Some(error));
        }
        Revision::Bloqueada => return alta::pagina(&state, origen, context! { paso => "bloqueado" }),
        Revision::NoDisponible => return alta::pagina(&state, origen, context! { paso => "no_disponible" }),
    };

    let mut conn = state.pool.acquire().await.map_err(alta::error_bd)?;
    let existente = db::altas::cliente_por_telefono(&mut conn, &telefono).await.map_err(alta::error_bd)?;
    drop(conn);
    let alta = match existente {
        Some(cliente_id) => AltaPendiente {
            origen,
            celular_id: Uuid::new_v4(),
            nombre: db::altas::nombre_cliente(&state.pool, cliente_id)
                .await
                .map_err(alta::error_bd)?
                .unwrap_or(nombre),
            quien: Quien::Cliente(cliente_id),
        },
        None => AltaPendiente {
            origen,
            celular_id: Uuid::new_v4(),
            nombre,
            quien: Quien::Nuevo { telefono },
        },
    };
    alta::pagina_acceso(&state, alta)
}

// ── Celular sin passkeys ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SoloMonederoForm {
    pub ticket: Uuid,
    pub terminos: Option<String>,
}

/// Teléfono nuevo (ya comprobado) cuyo celular no puede crear passkeys: se crea su monedero
/// sin ligar el celular. Lo ve en la web, como hasta hoy.
pub async fn solo_monedero(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Form(form): Form<SoloMonederoForm>,
) -> Result<Response, StatusCode> {
    let origen = Origen::Kiosko(id);
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
    if !db::kiosko::completar(&mut tx, id, cliente_id).await.map_err(alta::error_bd)? {
        return alta::pagina(&state, origen, context! { paso => "no_disponible" });
    }
    tx.commit().await.map_err(alta::error_bd)?;
    state.altas.tomar(form.ticket);

    alta::pagina(&state, origen, context! {
        paso => "abonado_nuevo",
        nombre => alta::primer_nombre(&alta.nombre),
        cliente_id => cliente_id.to_string(),
    })
}
