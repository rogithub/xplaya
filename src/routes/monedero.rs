use axum::{
    extract::{Form, Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use minijinja::context;
use serde::Deserialize;
use uuid::Uuid;

use crate::{db, AppState};

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

pub async fn saldo_get(
    State(state): State<AppState>,
    Query(params): Query<SaldoParams>,
) -> Result<Response, StatusCode> {
    // Si viene ?p=telefono, busca directamente sin mostrar el formulario
    if let Some(raw) = params.p {
        let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        let tel = if digits.len() > 10 {
            digits[digits.len() - 10..].to_string()
        } else {
            digits
        };
        if tel.len() == 10 {
            let cliente_id = db::monedero::buscar_cliente(&state.pool, &tel)
                .await
                .map_err(|e| {
                    tracing::error!("buscar_cliente (param p): {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            if let Some(id) = cliente_id {
                return Ok(Redirect::to(&format!("/monedero/{}", id)).into_response());
            }
        }
        // Teléfono inválido o no encontrado → cae al formulario sin error (el QR simplemente no funcionó)
        return Ok(Redirect::to("/saldo").into_response());
    }

    let tmpl = state.tmpl.get_template("monedero/saldo.html").map_err(|e| {
        tracing::error!("Template saldo: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! { error => params.error })
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

pub async fn saldo_post(
    State(state): State<AppState>,
    Form(form): Form<SaldoForm>,
) -> Result<Redirect, StatusCode> {
    let digits: String = form.telefono.chars().filter(|c| c.is_ascii_digit()).collect();
    let tel = if digits.len() > 10 {
        digits[digits.len() - 10..].to_string()
    } else {
        digits
    };

    if tel.len() != 10 {
        return Ok(Redirect::to(
            "/saldo?error=Ingresa+un+número+de+teléfono+de+10+dígitos.",
        ));
    }

    let cliente_id = db::monedero::buscar_cliente(&state.pool, &tel)
        .await
        .map_err(|e| {
            tracing::error!("buscar_cliente: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match cliente_id {
        None => Ok(Redirect::to(
            "/saldo?error=Número+no+registrado.+Visítanos+en+la+papelería+para+unirte.",
        )),
        Some(id) => Ok(Redirect::to(&format!("/monedero/{}", id))),
    }
}

// ── /app/:cliente_id ──────────────────────────────────────────────────────────

pub async fn app(
    State(state): State<AppState>,
    Path(cliente_id): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let cliente = db::monedero::monedero(&state.pool, cliente_id)
        .await
        .map_err(|e| {
            tracing::error!("DB monedero {cliente_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let tmpl = state.tmpl.get_template("monedero/app.html").map_err(|e| {
        tracing::error!("Template app: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! { cliente => cliente })
        .map_err(|e| {
            tracing::error!("Render app: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html))
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

    let tmpl = state.tmpl.get_template("monedero/recibo.html").map_err(|e| {
        tracing::error!("Template recibo: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context! { venta => venta, id => id.to_string() })
        .map_err(|e| {
            tracing::error!("Render recibo: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html))
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

    let tmpl = state.tmpl.get_template("monedero/recibo_print.html").map_err(|e| {
        tracing::error!("Template recibo_print: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(minijinja::context! { venta => venta }).map_err(|e| {
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

    let tmpl = state.tmpl.get_template("monedero/cotizacion_print.html").map_err(|e| {
        tracing::error!("Template cotizacion_print: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(minijinja::context! { cotizacion => cot }).map_err(|e| {
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
