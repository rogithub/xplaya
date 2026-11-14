//! "Imprime tú solito": autoservicio de impresión en la app (app.xplaya.com), solo con sesión.
//!
//! El cliente sube **un PDF** (b/n, carta), elige páginas, copias y una cara / doble cara, y ve
//! el total ($1 por cara). El trabajo queda `preparado`. Al llegar a la tienda escanea el QR del
//! kiosko (`routes/kiosko.rs::escaneo`), que lo marca "en la tienda" 30 min (`AppState::en_tienda`);
//! con eso "Mandar a imprimir" pasa sus trabajos a la caja (`por_autorizar`). La caja lo autoriza
//! en el POS y xplaya lo imprime (pasos 3 y 4 de `ROADMAP.md`).
//!
//! Las cuentas, en `impresion.rs`; la BD, en `db/impresiones.rs`; el PDF, en MinIO (`minio.rs`).

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form, Json,
};
use minijinja::{context, Value};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    avisos::codificar_url,
    db::{self, impresiones::Trabajo},
    impresion::{self, Opciones},
    minio::Minio,
    routes::app::cliente_sesion,
    AppState,
};

/// Por qué no se puede usar: sin MinIO el autoservicio no existe; sin sesión, a la portada
/// (ahí se entra con la huella).
enum Rechazo {
    Apagado,
    SinSesion,
}

impl IntoResponse for Rechazo {
    fn into_response(self) -> Response {
        match self {
            Rechazo::Apagado => StatusCode::NOT_FOUND.into_response(),
            Rechazo::SinSesion => Redirect::to("/").into_response(),
        }
    }
}

async fn requisitos(state: &AppState, headers: &HeaderMap) -> Result<(Minio, Uuid), Rechazo> {
    let minio = state.minio.clone().ok_or(Rechazo::Apagado)?;
    let cliente_id = cliente_sesion(state, headers).await.ok_or(Rechazo::SinSesion)?;
    Ok((minio, cliente_id))
}

fn render(state: &AppState, plantilla: &str, ctx: Value) -> Response {
    match state.tmpl.get_template(plantilla).and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("Render {plantilla}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn error_bd(e: sqlx::Error) -> Response {
    tracing::error!("BD imprimir: {e}");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

/// Un trabajo como lo ve la plantilla.
#[derive(Serialize)]
struct Vista<'a> {
    #[serde(flatten)]
    t: &'a Trabajo,
    total: String,
}

fn vista(t: &Trabajo) -> Vista<'_> {
    Vista { t, total: format!("{:.2}", t.total()) }
}

// ── GET /imprimir ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListaParams {
    pub error: Option<String>,
    pub enviados: Option<u64>,
}

pub async fn lista(State(state): State<AppState>, headers: HeaderMap, Query(p): Query<ListaParams>) -> Response {
    let cliente_id = match requisitos(&state, &headers).await {
        Ok((_, id)) => id,
        Err(r) => return r.into_response(),
    };
    pagina_lista(&state, cliente_id, p.error, p.enviados).await
}

async fn pagina_lista(state: &AppState, cliente_id: Uuid, error: Option<String>, enviados: Option<u64>) -> Response {
    let trabajos = match db::impresiones::de_cliente(&state.pool, cliente_id).await {
        Ok(t) => t,
        Err(e) => return error_bd(e),
    };
    let preparados = trabajos.iter().filter(|t| t.estado == "preparado").count();
    // Mientras algo está en la caja o imprimiéndose, la página se recarga sola.
    let en_curso = trabajos.iter().any(|t| matches!(t.estado.as_str(), "por_autorizar" | "autorizado" | "imprimiendo"));
    let trabajos: Vec<_> = trabajos.iter().map(vista).collect();
    render(state, "app/imprimir.html", context! {
        en_app => true,
        nav => "imprimir",
        trabajos,
        preparados,
        en_curso,
        en_tienda => state.en_tienda.ver(cliente_id).is_some(),
        error,
        enviados,
        max_mb => impresion::MAX_BYTES / (1024 * 1024),
        max_hojas => impresion::MAX_HOJAS,
    })
}

// ── POST /imprimir (subir el PDF) ───────────────────────────────────────────

pub async fn subir(State(state): State<AppState>, headers: HeaderMap, mut multipart: Multipart) -> Response {
    let (minio, cliente_id) = match requisitos(&state, &headers).await {
        Ok(r) => r,
        Err(r) => return r.into_response(),
    };
    let con_error = |msg: &str| {
        Redirect::to(&format!("/imprimir?error={}", codificar_url(msg)))
            .into_response()
    };

    // Un solo campo: `archivo`.
    let (nombre, bytes) = loop {
        match multipart.next_field().await {
            Ok(Some(campo)) if campo.name() == Some("archivo") => {
                let nombre = impresion::nombre_archivo(campo.file_name().unwrap_or(""));
                match campo.bytes().await {
                    Ok(b) => break (nombre, b),
                    Err(_) => return con_error("Tu PDF es muy grande o no llegó completo. Máximo 20 MB."),
                }
            }
            Ok(Some(_)) => continue,
            Ok(None) => return con_error("Elige un PDF."),
            Err(_) => return con_error("Tu PDF es muy grande o no llegó completo. Máximo 20 MB."),
        }
    };
    if bytes.is_empty() {
        return con_error("Elige un PDF.");
    }
    if bytes.len() > impresion::MAX_BYTES {
        return con_error("Tu PDF pasa de 20 MB. Pídelo en caja.");
    }
    let paginas_pdf = match impresion::contar_paginas(&bytes) {
        Ok(n) => n,
        Err(msg) => return con_error(&msg),
    };
    let precio = match db::impresiones::precio_cara(&state.pool).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            tracing::error!("imprimir: sin AUTOSERVICIO_IMPRESORAS o sin precio de su producto");
            return con_error("El autoservicio no está disponible por ahora. Pídelo en caja.");
        }
        Err(e) => return error_bd(e),
    };
    // Con todas las páginas, 1 copia y una cara; si pasa del tope de hojas, a doble cara.
    let mut opciones = Opciones { paginas: String::new(), copias: 1, doble_cara: false };
    let cuenta = match impresion::calcular(paginas_pdf, &opciones, precio) {
        Ok(c) => c,
        Err(_) => {
            opciones.doble_cara = true;
            match impresion::calcular(paginas_pdf, &opciones, precio) {
                Ok(c) => c,
                Err(msg) => return con_error(&msg),
            }
        }
    };

    let id = Uuid::new_v4();
    let objeto = format!("{cliente_id}/{id}.pdf");
    if let Err(e) = minio.subir(&objeto, bytes.to_vec(), "application/pdf").await {
        tracing::error!("imprimir: {e}");
        return con_error("No pudimos guardar tu PDF. Intenta de nuevo.");
    }
    let nuevo = db::impresiones::Nuevo {
        id,
        cliente_id,
        archivo: &nombre,
        objeto: &objeto,
        paginas_pdf,
        cuenta: &cuenta,
        copias: opciones.copias,
        doble_cara: opciones.doble_cara,
        precio_cara: precio,
    };
    if let Err(e) = db::impresiones::crear(&state.pool, &nuevo).await {
        let _ = minio.borrar(&objeto).await;
        return error_bd(e);
    }
    Redirect::to(&format!("/imprimir/{id}")).into_response()
}

// ── GET /imprimir/{id} ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TrabajoParams {
    pub error: Option<String>,
}

pub async fn trabajo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(p): Query<TrabajoParams>,
) -> Response {
    let cliente_id = match requisitos(&state, &headers).await {
        Ok((_, id)) => id,
        Err(r) => return r.into_response(),
    };
    let t = match db::impresiones::uno(&state.pool, id, cliente_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return Redirect::to("/imprimir").into_response(),
        Err(e) => return error_bd(e),
    };
    render(&state, "app/imprimir_trabajo.html", context! {
        en_app => true,
        nav => "imprimir",
        trabajo => vista(&t),
        en_tienda => state.en_tienda.ver(cliente_id).is_some(),
        error => p.error,
        max_copias => impresion::MAX_COPIAS,
    })
}

// ── GET /imprimir/{id}/cuenta?paginas=&copias=&doble=1 ──────────────────────

#[derive(Deserialize)]
pub struct OpcionesForm {
    #[serde(default)]
    pub paginas: String,
    pub copias: u32,
    /// Casilla del formulario: llega solo si está marcada.
    #[serde(default)]
    pub doble: Option<String>,
}

impl OpcionesForm {
    fn opciones(&self) -> Opciones {
        Opciones { paginas: self.paginas.clone(), copias: self.copias, doble_cara: self.doble.is_some() }
    }
}

/// La cuenta al instante mientras el cliente cambia las opciones (la misma que se guarda).
pub async fn cuenta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(form): Query<OpcionesForm>,
) -> Response {
    let cliente_id = match requisitos(&state, &headers).await {
        Ok((_, id)) => id,
        Err(r) => return r.into_response(),
    };
    let t = match db::impresiones::uno(&state.pool, id, cliente_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => return error_bd(e),
    };
    match impresion::calcular(t.paginas_pdf as u32, &form.opciones(), t.precio_cara) {
        Ok(c) => Json(serde_json::json!({
            "caras": c.caras, "hojas": c.hojas, "total": format!("{:.2}", c.total),
        }))
        .into_response(),
        Err(msg) => Json(serde_json::json!({ "error": msg })).into_response(),
    }
}

// ── POST /imprimir/{id} (guardar opciones) ──────────────────────────────────

pub async fn guardar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Form(form): Form<OpcionesForm>,
) -> Response {
    let cliente_id = match requisitos(&state, &headers).await {
        Ok((_, id)) => id,
        Err(r) => return r.into_response(),
    };
    let t = match db::impresiones::uno(&state.pool, id, cliente_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return Redirect::to("/imprimir").into_response(),
        Err(e) => return error_bd(e),
    };
    let opciones = form.opciones();
    let cuenta = match impresion::calcular(t.paginas_pdf as u32, &opciones, t.precio_cara) {
        Ok(c) => c,
        Err(msg) => {
            return Redirect::to(&format!("/imprimir/{id}?error={}", codificar_url(&msg))).into_response();
        }
    };
    match db::impresiones::cambiar_opciones(&state.pool, id, cliente_id, &cuenta, opciones.copias, opciones.doble_cara).await {
        Ok(_) => Redirect::to("/imprimir").into_response(),
        Err(e) => error_bd(e),
    }
}

// ── POST /imprimir/{id}/quitar ──────────────────────────────────────────────

pub async fn quitar(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>) -> Response {
    let (minio, cliente_id) = match requisitos(&state, &headers).await {
        Ok(r) => r,
        Err(r) => return r.into_response(),
    };
    match db::impresiones::quitar(&state.pool, id, cliente_id).await {
        Ok(Some(objeto)) => {
            // Si falla, lo borra la limpieza (y MinIO a los 8 días).
            if let Err(e) = minio.borrar(&objeto).await {
                tracing::warn!("imprimir: {e}");
            }
            Redirect::to("/imprimir").into_response()
        }
        Ok(None) => Redirect::to("/imprimir").into_response(),
        Err(e) => error_bd(e),
    }
}

// ── POST /imprimir/mandar ───────────────────────────────────────────────────

/// Sus trabajos preparados pasan a la caja. Solo "en la tienda" (escaneó el QR del kiosko).
pub async fn mandar(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let cliente_id = match requisitos(&state, &headers).await {
        Ok((_, id)) => id,
        Err(r) => return r.into_response(),
    };
    if state.en_tienda.ver(cliente_id).is_none() {
        let msg = "Para mandarlos, escanea el QR de la pantalla de la papelería.";
        return pagina_lista(&state, cliente_id, Some(msg.into()), None).await;
    }
    match db::impresiones::mandar(&state.pool, cliente_id).await {
        Ok(n) => Redirect::to(&format!("/imprimir?enviados={n}")).into_response(),
        Err(e) => error_bd(e),
    }
}

// ── Limpieza ────────────────────────────────────────────────────────────────

/// Cada hora borra los PDFs de los trabajos que cumplieron sus días y los marca `vencido`.
/// MinIO los borra solo a los 8 días si esto falla.
pub fn limpiar(state: AppState) {
    let Some(minio) = state.minio.clone() else { return };
    tokio::spawn(async move {
        let mut cada = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
        loop {
            cada.tick().await;
            let vencidos = match db::impresiones::vencidos(&state.pool).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("imprimir, limpieza: {e}");
                    continue;
                }
            };
            for (id, objeto) in vencidos {
                match minio.borrar(&objeto).await {
                    Ok(()) => {
                        if let Err(e) = db::impresiones::marcar_vencido(&state.pool, id).await {
                            tracing::warn!("imprimir, limpieza {id}: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("imprimir, limpieza: {e}"),
                }
            }
        }
    });
}
