use axum::{
    extract::{Form, Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use minijinja::context;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db,
    models::monedero::{AlertaVencimiento, VentaRecibo},
    AppState,
};

// ── /terminos ─────────────────────────────────────────────────────────────────

pub async fn terminos(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let datos = db::settings::terminos(&state.pool).await.map_err(|e| {
        tracing::error!("DB terminos: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let tmpl = state.tmpl.get_template("pages/terminos.html").map_err(|e| {
        tracing::error!("Template terminos: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let html = tmpl.render(context! { t => datos }).map_err(|e| {
        tracing::error!("Render terminos: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}

// ── /saldo ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SaldoParams {
    pub error: Option<String>,
    pub p: Option<String>,
}

/// Consultar con solo el teléfono muestra lo básico ([`saldo_por_telefono`]): cualquiera
/// puede escribir un número, así que no lleva al monedero completo.
pub async fn saldo_get(
    State(state): State<AppState>,
    Query(params): Query<SaldoParams>,
) -> Result<Response, StatusCode> {
    // ?p=telefono: el QR de las tarjetas impresas del POS (`tarjetasMonedero.ts`).
    if let Some(raw) = params.p {
        if let Some(tel) = normalizar_telefono(&raw)
            && let Some(resultado) = saldo_por_telefono(&state, &tel).await?
        {
            return render_saldo(&state, None, Some(resultado));
        }
        // Teléfono inválido o no encontrado → cae al formulario sin error (el QR simplemente no funcionó)
        return Ok(Redirect::to("/saldo").into_response());
    }
    render_saldo(&state, params.error, None)
}

fn render_saldo(
    state: &AppState,
    error: Option<String>,
    resultado: Option<SaldoPorTelefono>,
) -> Result<Response, StatusCode> {
    let tmpl = state.tmpl.get_template("monedero/saldo.html").map_err(|e| {
        tracing::error!("Template saldo: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! { error, resultado })
        .map_err(|e| {
            tracing::error!("Render saldo: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html).into_response())
}

#[derive(Deserialize)]
pub struct SaldoForm {
    pub telefono: String,
}

/// El resultado se pinta en la respuesta del POST, sin redirigir: así el teléfono no queda en
/// la URL (ni en el historial ni en las estadísticas).
pub async fn saldo_post(
    State(state): State<AppState>,
    Form(form): Form<SaldoForm>,
) -> Result<Response, StatusCode> {
    let Some(tel) = normalizar_telefono(&form.telefono) else {
        return Ok(Redirect::to(
            "/saldo?error=Ingresa+un+número+de+teléfono+de+10+dígitos.",
        )
        .into_response());
    };
    match saldo_por_telefono(&state, &tel).await? {
        Some(resultado) => render_saldo(&state, None, Some(resultado)),
        None => Ok(Redirect::to(
            "/saldo?error=Número+no+registrado.+Visítanos+en+la+papelería+para+unirte.",
        )
        .into_response()),
    }
}

/// Lo que se ve consultando con solo el teléfono (`/saldo` y "Ver mi saldo" en la app): el
/// saldo y lo que está por vencer, con el nombre tapado para que no sirva para averiguar de
/// quién es un número. Sin historial, sin QR de caja y sin el enlace al monedero completo
/// (ese solo llega por el recibo, el POS o la app con huella).
#[derive(Serialize)]
pub struct SaldoPorTelefono {
    pub nombre: String,
    pub saldo: String,
    pub alerta: Option<AlertaVencimiento>,
}

pub async fn saldo_por_telefono(state: &AppState, tel: &str) -> Result<Option<SaldoPorTelefono>, StatusCode> {
    let error = |e: sqlx::Error| {
        tracing::error!("Saldo por teléfono: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    };
    let Some(id) = db::monedero::buscar_cliente(&state.pool, tel).await.map_err(error)? else {
        return Ok(None);
    };
    Ok(db::monedero::monedero(&state.pool, id).await.map_err(error)?.map(|c| SaldoPorTelefono {
        nombre: nombre_oculto(&c.nombre),
        saldo: c.saldo,
        alerta: c.alerta,
    }))
}

/// "NAYELLI ALVARADO CRUZ" → "N••••• A.": la inicial del nombre y la del primer apellido. Los
/// puntos son siempre cinco para no dar el largo del nombre.
pub fn nombre_oculto(nombre: &str) -> String {
    let mut palabras = nombre.split_whitespace();
    let inicial = |p: &str| p.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    match (palabras.next(), palabras.next()) {
        (Some(n), Some(a)) => format!("{}••••• {}.", inicial(n), inicial(a)),
        (Some(n), None) => format!("{}•••••", inicial(n)),
        _ => String::new(),
    }
}

// ── /monedero/:cliente_id ─────────────────────────────────────────────────────

/// Monedero en xplaya.com. La misma tarjeta se sirve en app.xplaya.com desde
/// `routes::app::monedero`; las dos llaman a `render_monedero`.
pub async fn app(
    State(state): State<AppState>,
    Path(cliente_id): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    render_monedero(&state, cliente_id, false, false).await
}

/// Arma la tarjeta del monedero. `en_app` indica si se sirve en app.xplaya.com:
/// ahí se enlaza el manifest de la PWA y se registra el service worker; en
/// xplaya.com, en cambio, se muestra un enlace para instalar la app.
/// `con_sesion`: el celular está ligado (la app abre en `/` y se puede salir).
pub async fn render_monedero(
    state: &AppState,
    cliente_id: Uuid,
    en_app: bool,
    con_sesion: bool,
) -> Result<Html<String>, StatusCode> {
    let cliente = db::monedero::monedero(&state.pool, cliente_id)
        .await
        .map_err(|e| {
            tracing::error!("DB monedero {cliente_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // QR para escanear en caja: por ahora lleva el teléfono, igual que las tarjetas
    // impresas (el POS ya lo lee). En el paso 4 se cambia por ID + código que caduca.
    let qr_caja = qr_svg(&cliente.telefono);

    let tmpl = state.tmpl.get_template("monedero/app.html").map_err(|e| {
        tracing::error!("Template app: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! {
            cliente => cliente,
            cliente_id => cliente_id.to_string(),
            qr_caja => qr_caja,
            en_app => en_app,
            con_sesion => con_sesion,
            // Barra de abajo (Monedero · Tienda · Carrito), solo en la app.
            nav => en_app.then_some("monedero"),
        })
        .map_err(|e| {
            tracing::error!("Render app: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html))
}

/// Deja solo los dígitos y se queda con los últimos 10 (quita +52, espacios, guiones).
/// `None` si no quedan exactamente 10.
pub fn normalizar_telefono(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let tel = if digits.len() > 10 {
        digits[digits.len() - 10..].to_string()
    } else {
        digits
    };
    (tel.len() == 10).then_some(tel)
}

// ── /recibo/:id ───────────────────────────────────────────────────────────────

pub async fn recibo(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let venta = db::monedero::recibo(&state.pool, id)
        .await
        .map_err(|e| {
            tracing::error!("DB recibo {id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Enlace de alta en la app (una semana, un solo uso). Si la consulta falla, el recibo se
    // muestra igual, sin el enlace.
    let alta = db::altas::venta_disponible(&state.pool, id)
        .await
        .inspect_err(|e| tracing::error!("DB alta recibo {id}: {e}"))
        .ok()
        .flatten();
    let alta_disponible = alta.is_some();
    let alta_cambio = alta.is_some_and(|a| a.tiene_celular);

    let tmpl = state.tmpl.get_template("monedero/recibo.html").map_err(|e| {
        tracing::error!("Template recibo: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! {
            descripcion => descripcion_recibo(&venta, alta_disponible && !alta_cambio),
            venta => venta,
            id => id.to_string(),
            alta_disponible,
            alta_cambio,
        })
        .map_err(|e| {
            tracing::error!("Render recibo: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html))
}

/// Texto de la vista previa del recibo en WhatsApp: es lo primero que lee el cliente, así
/// que invita a la app mientras el enlace de alta sigue vivo (y el celular no está ligado).
fn descripcion_recibo(venta: &VentaRecibo, invita_app: bool) -> String {
    match (venta.tiene_monedero, invita_app) {
        (true, true) => format!(
            "👛 {} en tu monedero · Toca para ver tu ticket y guardar tu monedero en el celular 👆",
            venta.saldo_monedero
        ),
        (true, false) => format!("👛 {} en tu monedero · Toca para ver tu ticket 👆", venta.saldo_monedero),
        (false, true) => "Toca para ver tu compra y crear tu monedero gratis 👆".to_string(),
        (false, false) => "Toca aquí para ver los detalles de tu compra 👆".to_string(),
    }
}

// ── QR de validación ──────────────────────────────────────────────────────────

/// SVG inline de un QR que apunta a `url`. El cliente lo escanea y llega a la
/// versión en vivo del recibo/cotización en xplaya.com — eso valida que el
/// papel sea auténtico.
pub fn qr_svg(url: &str) -> Option<String> {
    let code = qrcode::QrCode::new(url.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color>()
            .min_dimensions(140, 140)
            .build(),
    )
}

// ── /recibo/:id/print ─────────────────────────────────────────────────────────

pub async fn recibo_print(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let venta = db::monedero::recibo(&state.pool, id)
        .await
        .map_err(|e| { tracing::error!("DB recibo {id}: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let qr = qr_svg(&format!("{}/recibo/{}", state.config.site_url, id));

    let tmpl = state.tmpl.get_template("monedero/recibo_print.html").map_err(|e| {
        tracing::error!("Template recibo_print: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(minijinja::context! { venta => venta, qr_svg => qr }).map_err(|e| {
        tracing::error!("Render recibo_print: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}

// ── /recibo/:id/pdf ───────────────────────────────────────────────────────────

pub async fn recibo_pdf(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let url = format!("{}/recibo/{}/print", state.config.site_url, id);
    pdf_desde_url(&state, &url, &format!("Recibo_{}.pdf", id)).await
}

// ── /cotizacion/:uid ──────────────────────────────────────────────────────────

pub async fn cotizacion(
    State(state): State<AppState>,
    Path(uid): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let cot = db::monedero::cotizacion(&state.pool, uid)
        .await
        .map_err(|e| {
            tracing::error!("DB cotizacion {uid}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let tmpl = state.tmpl.get_template("monedero/cotizacion.html").map_err(|e| {
        tracing::error!("Template cotizacion: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! { cotizacion => cot })
        .map_err(|e| {
            tracing::error!("Render cotizacion: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html))
}

// ── /cotizacion/:uid/print ────────────────────────────────────────────────────

pub async fn cotizacion_print(
    State(state): State<AppState>,
    Path(uid): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let cot = db::monedero::cotizacion(&state.pool, uid)
        .await
        .map_err(|e| { tracing::error!("DB cotizacion {uid}: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let qr = qr_svg(&format!("{}/cotizacion/{}", state.config.site_url, uid));

    let tmpl = state.tmpl.get_template("monedero/cotizacion_print.html").map_err(|e| {
        tracing::error!("Template cotizacion_print: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(minijinja::context! { cotizacion => cot, qr_svg => qr }).map_err(|e| {
        tracing::error!("Render cotizacion_print: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}

// ── /cotizacion/:uid/pdf ──────────────────────────────────────────────────────

pub async fn cotizacion_pdf(
    State(state): State<AppState>,
    Path(uid): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let url = format!("{}/cotizacion/{}/print", state.config.site_url, uid);
    pdf_desde_url(&state, &url, &format!("Cotizacion_{}.pdf", uid)).await
}

// ── PDF helper ────────────────────────────────────────────────────────────────

async fn pdf_desde_url(state: &AppState, url: &str, filename: &str) -> Result<Response, StatusCode> {
    let gotenberg = format!("{}/forms/chromium/convert/url", state.config.gotenberg_url);

    let form = reqwest::multipart::Form::new()
        .text("url", url.to_string());

    let resp = state.http
        .post(&gotenberg)
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Gotenberg request error: {e}");
            StatusCode::BAD_GATEWAY
        })?;

    if !resp.status().is_success() {
        tracing::error!("Gotenberg devolvió {}", resp.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    let bytes = resp.bytes().await.map_err(|e| {
        tracing::error!("Gotenberg read error: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    let disposition = format!("attachment; filename=\"{}\"", filename);
    Ok((
        [(header::CONTENT_TYPE, "application/pdf"),
         (header::CONTENT_DISPOSITION, &disposition)],
        bytes,
    ).into_response())
}

// ── /r/:code (ShortUrls redirect) ────────────────────────────────────────────

pub async fn redirigir(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Redirect, StatusCode> {
    let entry = db::short_urls::buscar(&state.pool, &code)
        .await
        .map_err(|e| {
            tracing::error!("DB short_url {code}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let (tipo, id) = entry;
    let url = match tipo.as_str() {
        "recibo" => format!("/recibo/{}", id),
        "cotizacion" => format!("/cotizacion/{}", id),
        "monedero" => format!("/monedero/{}", id),
        _ => return Err(StatusCode::NOT_FOUND),
    };

    Ok(Redirect::permanent(&url))
}

#[cfg(test)]
mod tests {
    use super::nombre_oculto;

    #[test]
    fn nombre_oculto_deja_las_iniciales() {
        assert_eq!(nombre_oculto("NAYELLI ALVARADO CRUZ"), "N••••• A.");
        assert_eq!(nombre_oculto("ángel"), "Á•••••");
        assert_eq!(nombre_oculto("  "), "");
    }
}
