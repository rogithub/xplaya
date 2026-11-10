//! Alta desde el enlace del recibo: `app.xplaya.com/alta/recibo/{venta}`.
//!
//! Los cuatro casos de `APP_MONEDERO.md`:
//!
//! | Venta | Quién abre el enlace | Qué pasa |
//! |---|---|---|
//! | Con cliente | cualquiera | 4 dígitos del teléfono → crea su passkey → celular ligado |
//! | Sin cliente | teléfono nuevo | nombre y teléfono → passkey → cliente nuevo, compra abonada |
//! | Sin cliente | teléfono que ya existe | se abona la compra a ese cliente; **no** se liga el celular |
//! | Sin cliente | cliente con sesión en la app | botón para abonar la compra a su monedero |
//!
//! Entre los 4 dígitos (o los datos) y la passkey, lo ya comprobado vive en memoria como
//! `AltaPendiente`, con un ticket que va en la página. Nada se escribe en la BD hasta que
//! la passkey queda creada, salvo los intentos fallidos.
//!
//! El kiosko de la tienda (`routes/kiosko.rs`) reusa la misma alta pendiente, la misma
//! plantilla y el mismo registro de la passkey, con `Origen::Kiosko`.

use axum::{
    extract::{Form, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use minijinja::{context, Value};
use serde::Deserialize;
use uuid::Uuid;
use webauthn_rs::prelude::RegisterPublicKeyCredential;

use crate::{
    db::{self, altas::Intento},
    passkeys,
    routes::{app, monedero::normalizar_telefono},
    AppState,
};

/// Lo que ya se comprobó y espera la passkey para completarse.
#[derive(Clone)]
pub struct AltaPendiente {
    pub origen: Origen,
    /// Id del celular nuevo; también es el "usuario" de la passkey (no revela al cliente).
    pub celular_id: Uuid,
    pub nombre: String,
    pub quien: Quien,
}

/// Desde dónde se liga el celular.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origen {
    /// Enlace del recibo de esta venta.
    Recibo(Uuid),
    /// Kiosko de la tienda; el id es la visita del QR y después la solicitud.
    Kiosko(Uuid),
}

#[derive(Clone)]
pub enum Quien {
    /// Cliente que ya existe (la venta lo tenía); pasó los 4 dígitos.
    Cliente(Uuid),
    /// Teléfono que no existe: el cliente se crea al terminar.
    Nuevo { telefono: String },
}

// ── GET /alta/recibo/{venta} ────────────────────────────────────────────────

pub async fn recibo(
    State(state): State<AppState>,
    Path(ajuste_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let Some(venta) = db::altas::venta_disponible(&state.pool, ajuste_id).await.map_err(error_bd)? else {
        return pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "no_disponible" });
    };

    if let Some(sesion) = app::cliente_sesion(&state, &headers).await {
        match venta.cliente_id {
            // Su propia compra: no hay nada que hacer.
            Some(id) if id == sesion => return Ok(Redirect::to("/").into_response()),
            // Olvidó mostrar su tarjeta: se le ofrece abonarla a su monedero.
            None => return pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "abonar" }),
            // Compra de otra persona (por ejemplo, de alguien de su familia): flujo normal.
            Some(_) => {}
        }
    }

    match venta.cliente_id {
        Some(_) => pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "digitos", cambio => venta.tiene_celular }),
        None => pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "datos" }),
    }
}

// ── POST /alta/recibo/{venta}/digitos ───────────────────────────────────────

#[derive(Deserialize)]
pub struct DigitosForm {
    pub digitos: String,
}

pub async fn digitos(
    State(state): State<AppState>,
    Path(ajuste_id): Path<Uuid>,
    Form(form): Form<DigitosForm>,
) -> Result<Response, StatusCode> {
    let Some(venta) = db::altas::venta_disponible(&state.pool, ajuste_id).await.map_err(error_bd)? else {
        return pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "no_disponible" });
    };
    let Some(cliente_id) = venta.cliente_id else {
        return Ok(Redirect::to(&format!("/alta/recibo/{ajuste_id}")).into_response());
    };

    let digitos: String = form.digitos.chars().filter(|c| c.is_ascii_digit()).collect();
    if digitos.len() != 4 {
        return pagina(&state, Origen::Recibo(ajuste_id), context! {
            paso => "digitos", cambio => venta.tiene_celular,
            error => "Escribe los últimos 4 dígitos de tu teléfono.",
        });
    }

    match db::altas::revisar_digitos(&state.pool, ajuste_id, cliente_id, &digitos).await.map_err(error_bd)? {
        Intento::Correcto => {
            let nombre = db::altas::nombre_cliente(&state.pool, cliente_id)
                .await
                .map_err(error_bd)?
                .unwrap_or_default();
            let alta = AltaPendiente {
                origen: Origen::Recibo(ajuste_id),
                celular_id: Uuid::new_v4(),
                nombre,
                quien: Quien::Cliente(cliente_id),
            };
            pagina_acceso(&state, alta)
        }
        Intento::Incorrecto { quedan } => {
            let error = if quedan == 1 {
                "Esos no son. Te queda 1 intento.".to_string()
            } else {
                format!("Esos no son. Te quedan {quedan} intentos.")
            };
            pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "digitos", cambio => venta.tiene_celular, error })
        }
        Intento::Bloqueado => pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "bloqueado" }),
    }
}

// ── POST /alta/recibo/{venta}/datos ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct DatosForm {
    pub nombre: String,
    pub telefono: String,
}

pub async fn datos(
    State(state): State<AppState>,
    Path(ajuste_id): Path<Uuid>,
    Form(form): Form<DatosForm>,
) -> Result<Response, StatusCode> {
    let Some(venta) = db::altas::venta_disponible(&state.pool, ajuste_id).await.map_err(error_bd)? else {
        return pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "no_disponible" });
    };
    if venta.cliente_id.is_some() {
        return Ok(Redirect::to(&format!("/alta/recibo/{ajuste_id}")).into_response());
    }

    // Espacios de más fuera; el nombre se recorta a 100 caracteres.
    let nombre: String = form.nombre.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(100).collect();
    let tel = normalizar_telefono(&form.telefono);
    let (Some(tel), false) = (tel, nombre.is_empty()) else {
        return pagina(&state, Origen::Recibo(ajuste_id), context! {
            paso => "datos", nombre => form.nombre, telefono => form.telefono,
            error => "Escribe tu nombre y tu teléfono a 10 dígitos.",
        });
    };

    let mut tx = state.pool.begin().await.map_err(error_bd)?;
    let Some(cliente_id) = db::altas::cliente_por_telefono(&mut tx, &tel).await.map_err(error_bd)? else {
        // Teléfono nuevo: falta la passkey para crear su monedero.
        let alta = AltaPendiente {
            origen: Origen::Recibo(ajuste_id),
            celular_id: Uuid::new_v4(),
            nombre,
            quien: Quien::Nuevo { telefono: tel },
        };
        return pagina_acceso(&state, alta);
    };

    // El teléfono ya tiene monedero. Desde fuera de la tienda no se puede comprobar que
    // sea suyo, así que no se liga el celular; la compra sí se abona a ese monedero.
    let abonada = db::altas::abonar_venta(&mut tx, ajuste_id, cliente_id).await.map_err(error_bd)?;
    let gastado = db::altas::gastar(&mut tx, ajuste_id, "abonado", cliente_id).await.map_err(error_bd)?;
    if !(abonada && gastado) {
        return pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "no_disponible" });
    }
    tx.commit().await.map_err(error_bd)?;
    pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "abonado" })
}

// ── POST /alta/recibo/{venta}/abonar (cliente con sesión) ───────────────────

pub async fn abonar(
    State(state): State<AppState>,
    Path(ajuste_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let Some(cliente_id) = app::cliente_sesion(&state, &headers).await else {
        return Ok(Redirect::to(&format!("/alta/recibo/{ajuste_id}")).into_response());
    };

    let mut tx = state.pool.begin().await.map_err(error_bd)?;
    let abonada = db::altas::abonar_venta(&mut tx, ajuste_id, cliente_id).await.map_err(error_bd)?;
    let gastado = db::altas::gastar(&mut tx, ajuste_id, "abonado", cliente_id).await.map_err(error_bd)?;
    if !(abonada && gastado) {
        return pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "no_disponible" });
    }
    tx.commit().await.map_err(error_bd)?;
    Ok(Redirect::to("/").into_response())
}

// ── POST /alta/recibo/{venta}/solo-abonar ───────────────────────────────────

#[derive(Deserialize)]
pub struct SoloAbonarForm {
    pub ticket: Uuid,
    pub terminos: Option<String>,
}

/// Teléfono nuevo cuyo celular no puede crear passkeys: se crea su monedero y se abona la
/// compra, sin ligar el celular. Puede ver su monedero en la web como hasta hoy.
pub async fn solo_abonar(
    State(state): State<AppState>,
    Path(ajuste_id): Path<Uuid>,
    Form(form): Form<SoloAbonarForm>,
) -> Result<Response, StatusCode> {
    let Some(alta) = state.altas.ver(form.ticket).filter(|a| a.origen == Origen::Recibo(ajuste_id)) else {
        return Ok(Redirect::to(&format!("/alta/recibo/{ajuste_id}")).into_response());
    };
    let Quien::Nuevo { telefono } = &alta.quien else {
        return Ok(Redirect::to(&format!("/alta/recibo/{ajuste_id}")).into_response());
    };
    if form.terminos.is_none() {
        return pagina_acceso(&state, alta);
    }

    let mut tx = state.pool.begin().await.map_err(error_bd)?;
    if db::altas::cliente_por_telefono(&mut tx, telefono).await.map_err(error_bd)?.is_some() {
        return pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "no_disponible" });
    }
    let cliente_id = db::altas::crear_cliente(&mut tx, &alta.nombre, telefono).await.map_err(error_bd)?;
    let abonada = db::altas::abonar_venta(&mut tx, ajuste_id, cliente_id).await.map_err(error_bd)?;
    let gastado = db::altas::gastar(&mut tx, ajuste_id, "abonado", cliente_id).await.map_err(error_bd)?;
    if !(abonada && gastado) {
        return pagina(&state, Origen::Recibo(ajuste_id), context! { paso => "no_disponible" });
    }
    tx.commit().await.map_err(error_bd)?;
    state.altas.tomar(form.ticket);

    pagina(&state, Origen::Recibo(ajuste_id), context! {
        paso => "abonado_nuevo",
        nombre => primer_nombre(&alta.nombre),
        cliente_id => cliente_id.to_string(),
    })
}

// ── POST /passkey/registro/inicio ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct TicketJson {
    pub ticket: Uuid,
}

/// Opciones para que el celular cree la passkey del alta pendiente.
pub async fn registro_inicio(
    State(state): State<AppState>,
    Json(body): Json<TicketJson>,
) -> Response {
    let Some(alta) = state.altas.ver(body.ticket) else {
        return error_json(StatusCode::GONE, "Pasó mucho tiempo. Vuelve a empezar.");
    };

    let nombre = format!("{} · Monedero xplaya", alta.nombre);
    match state.webauthn.start_passkey_registration(alta.celular_id, &nombre, &alta.nombre, None) {
        Ok((opciones, estado)) => {
            state.registros.guardar_con(body.ticket, estado);
            Json(passkeys::opciones_descubrible(&opciones)).into_response()
        }
        Err(e) => {
            tracing::error!("start_passkey_registration: {e:?}");
            error_json(StatusCode::INTERNAL_SERVER_ERROR, "No se pudo preparar tu acceso. Intenta de nuevo.")
        }
    }
}

// ── POST /passkey/registro/fin ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegistroFin {
    pub ticket: Uuid,
    pub terminos: bool,
    pub credencial: RegisterPublicKeyCredential,
}

/// Revisa la passkey nueva y, en una sola transacción, completa el alta: crea o actualiza
/// al cliente, abona la compra si hace falta, gasta el enlace, liga el celular (desligando
/// el anterior) y abre la sesión.
pub async fn registro_fin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RegistroFin>,
) -> Response {
    if !body.terminos {
        return error_json(StatusCode::BAD_REQUEST, "Acepta los términos del monedero para continuar.");
    }
    let (Some(alta), Some(estado)) = (state.altas.ver(body.ticket), state.registros.tomar(body.ticket)) else {
        return error_json(StatusCode::GONE, "Pasó mucho tiempo. Vuelve a empezar.");
    };

    let passkey = match state.webauthn.finish_passkey_registration(&body.credencial, &estado) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("finish_passkey_registration: {e:?}");
            return error_json(StatusCode::BAD_REQUEST, "No se pudo crear tu acceso. Intenta de nuevo.");
        }
    };

    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok());
    match completar_alta(&state, &alta, &passkey, passkeys::dispositivo(ua)).await {
        Ok(Some(token)) => {
            state.altas.tomar(body.ticket);
            let cookie = app::cookie_sesion(&state, &token);
            ([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ir": "/" }))).into_response()
        }
        Ok(None) => error_json(
            StatusCode::CONFLICT,
            "Ya no se pudo completar. Si necesitas ayuda, pregunta en la tienda.",
        ),
        Err(e) => {
            tracing::error!("completar_alta {:?}: {e}", alta.origen);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, "Algo falló de nuestro lado. Intenta de nuevo.")
        }
    }
}

/// `Ok(None)` si el enlace se gastó mientras tanto (o el teléfono se registró): no se
/// escribe nada. Desde el kiosko no hay venta que abonar: se cierra la solicitud.
async fn completar_alta(
    state: &AppState,
    alta: &AltaPendiente,
    passkey: &webauthn_rs::prelude::Passkey,
    dispositivo: &str,
) -> Result<Option<String>, sqlx::Error> {
    let mut tx = state.pool.begin().await?;

    let cliente_id = match &alta.quien {
        Quien::Cliente(id) => {
            db::altas::aceptar_programa(&mut tx, *id).await?;
            *id
        }
        Quien::Nuevo { telefono } => {
            if db::altas::cliente_por_telefono(&mut tx, telefono).await?.is_some() {
                return Ok(None);
            }
            let id = db::altas::crear_cliente(&mut tx, &alta.nombre, telefono).await?;
            if let Origen::Recibo(ajuste_id) = alta.origen
                && !db::altas::abonar_venta(&mut tx, ajuste_id, id).await?
            {
                return Ok(None);
            }
            id
        }
    };

    let origen = match alta.origen {
        Origen::Recibo(ajuste_id) => {
            if !db::altas::gastar(&mut tx, ajuste_id, "ligado", cliente_id).await? {
                return Ok(None);
            }
            "recibo"
        }
        Origen::Kiosko(solicitud) => {
            if !db::kiosko::completar(&mut tx, solicitud, cliente_id).await? {
                return Ok(None);
            }
            "kiosko"
        }
    };
    db::celulares::ligar(&mut tx, alta.celular_id, cliente_id, passkey, origen, dispositivo).await?;
    let token = db::celulares::crear_sesion(&mut tx, alta.celular_id).await?;
    tx.commit().await?;
    Ok(Some(token))
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Guarda el alta pendiente y muestra el paso de crear la passkey.
pub fn pagina_acceso(state: &AppState, alta: AltaPendiente) -> Result<Response, StatusCode> {
    let origen = alta.origen;
    let nombre = primer_nombre(&alta.nombre);
    let nuevo = matches!(alta.quien, Quien::Nuevo { .. });
    let ticket = state.altas.guardar(alta);
    pagina(state, origen, context! {
        paso => "acceso", ticket => ticket.to_string(), nombre, nuevo,
    })
}

/// `accion` es la ruta base de los formularios; `kiosko` cambia los textos.
pub fn pagina(state: &AppState, origen: Origen, ctx: Value) -> Result<Response, StatusCode> {
    let (accion, kiosko) = match origen {
        Origen::Recibo(id) => (format!("/alta/recibo/{id}"), false),
        Origen::Kiosko(id) => (format!("/kiosko/s/{id}"), true),
    };
    let tmpl = state.tmpl.get_template("app/alta.html").map_err(|e| {
        tracing::error!("Template app/alta: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! {
            accion => Value::from_safe_string(accion), kiosko, en_app => true, ..ctx
        })
        .map_err(|e| {
            tracing::error!("Render app/alta: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html).into_response())
}

pub fn primer_nombre(nombre: &str) -> String {
    nombre.split_whitespace().next().unwrap_or("").to_string()
}

pub fn error_bd(e: sqlx::Error) -> StatusCode {
    tracing::error!("BD alta: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

pub fn error_json(status: StatusCode, mensaje: &str) -> Response {
    (status, Json(serde_json::json!({ "error": mensaje }))).into_response()
}
