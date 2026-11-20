//! "Mi perfil" (Más → Mi perfil): el nombre y la foto con los que la papelería ve al cliente en
//! el chat (`perfil.rs`). Sin MinIO solo se puede poner el nombre.

use axum::{
    extract::{Form, Multipart, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use minijinja::context;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    avisos::codificar_url,
    db,
    minio::BUCKET_PERFILES,
    perfil,
    routes::app::cliente_sesion,
    AppState,
};

fn error_bd(e: sqlx::Error) -> Response {
    tracing::error!("BD perfil: {e}");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

fn con_error(msg: &str) -> Response {
    Redirect::to(&format!("/perfil?error={}", codificar_url(msg))).into_response()
}

#[derive(Deserialize)]
pub struct PaginaParams {
    error: Option<String>,
    ok: Option<String>,
}

// ── GET /perfil ─────────────────────────────────────────────────────────────

pub async fn pagina(State(state): State<AppState>, headers: HeaderMap, Query(p): Query<PaginaParams>) -> Response {
    let Some(cliente) = cliente_sesion(&state, &headers).await else {
        return Redirect::to("/").into_response();
    };
    let perfil = match db::perfil::de_cliente(&state.pool, cliente).await {
        Ok(Some(p)) => p,
        Ok(None) => return Redirect::to("/").into_response(),
        Err(e) => return error_bd(e),
    };
    let html = state.tmpl.get_template("app/perfil.html").and_then(|t| {
        t.render(context! {
            en_app => true,
            nav => "perfil",
            perfil,
            fotos => state.minio.is_some(),
            max_nombre => perfil::MAX_NOMBRE,
            max_mb => perfil::MAX_BYTES / (1024 * 1024),
            error => p.error,
            ok => p.ok,
        })
    });
    match html {
        Ok(h) => Html(h).into_response(),
        Err(e) => {
            tracing::error!("Render app/perfil: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── POST /perfil/nombre ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct NombreForm {
    nombre: String,
}

pub async fn nombre(State(state): State<AppState>, headers: HeaderMap, Form(f): Form<NombreForm>) -> Response {
    let Some(cliente) = cliente_sesion(&state, &headers).await else {
        return Redirect::to("/").into_response();
    };
    let nombre = match perfil::limpiar_nombre(&f.nombre) {
        Ok(n) => n,
        Err(msg) => return con_error(&msg),
    };
    match db::perfil::guardar_nombre(&state.pool, cliente, nombre.as_deref()).await {
        Ok(()) => Redirect::to("/perfil?ok=nombre").into_response(),
        Err(e) => error_bd(e),
    }
}

// ── POST /perfil/foto (multipart: foto) ─────────────────────────────────────

pub async fn subir_foto(State(state): State<AppState>, headers: HeaderMap, mut multipart: Multipart) -> Response {
    let Some(minio) = state.minio.as_ref().map(|m| m.en(BUCKET_PERFILES)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(cliente) = cliente_sesion(&state, &headers).await else {
        return Redirect::to("/").into_response();
    };
    const GRANDE: &str = "Tu foto es muy grande o no llegó completa. Máximo 10 MB.";
    let mut foto = None;
    loop {
        match multipart.next_field().await {
            Ok(Some(campo)) if campo.name() == Some("foto") => match campo.bytes().await {
                Ok(b) if !b.is_empty() => foto = Some(b),
                Ok(_) => {}
                Err(_) => return con_error(GRANDE),
            },
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => return con_error(GRANDE),
        }
    }
    let Some(bytes) = foto else { return con_error("Elige una foto.") };
    // Decodificar y redimensionar es trabajo de CPU: fuera del hilo de tokio.
    let jpeg = match tokio::task::spawn_blocking(move || perfil::preparar_foto(&bytes)).await {
        Ok(Ok(j)) => j,
        Ok(Err(msg)) => return con_error(&msg),
        Err(e) => {
            tracing::error!("perfil foto: {e}");
            return con_error("No pudimos procesar tu foto. Intenta de nuevo.");
        }
    };
    // Llave nueva cada vez: el POS y los celulares no se quedan con la anterior en caché.
    let llave = format!("{cliente}/{}.jpg", Uuid::new_v4());
    if let Err(e) = minio.subir(&llave, jpeg, "image/jpeg").await {
        tracing::error!("perfil: {e}");
        return con_error("No pudimos guardar tu foto. Intenta de nuevo.");
    }
    match db::perfil::guardar_foto(&state.pool, cliente, Some(&llave)).await {
        Ok(anterior) => {
            if let Some(anterior) = anterior
                && let Err(e) = minio.borrar(&anterior).await
            {
                tracing::warn!("perfil, foto anterior: {e}");
            }
            Redirect::to("/perfil?ok=foto").into_response()
        }
        Err(e) => {
            let _ = minio.borrar(&llave).await;
            error_bd(e)
        }
    }
}

// ── POST /perfil/foto/quitar ────────────────────────────────────────────────

pub async fn quitar_foto(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(minio) = state.minio.as_ref().map(|m| m.en(BUCKET_PERFILES)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(cliente) = cliente_sesion(&state, &headers).await else {
        return Redirect::to("/").into_response();
    };
    match db::perfil::guardar_foto(&state.pool, cliente, None).await {
        Ok(anterior) => {
            // Si falla, la foto queda huérfana en el bucket; no se ve en ningún lado.
            if let Some(anterior) = anterior
                && let Err(e) = minio.borrar(&anterior).await
            {
                tracing::warn!("perfil quitar: {e}");
            }
            Redirect::to("/perfil?ok=quitada").into_response()
        }
        Err(e) => error_bd(e),
    }
}

// ── GET /perfil/foto?v=… (la suya, para verla en la app) ────────────────────

pub async fn foto(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(minio) = state.minio.as_ref().map(|m| m.en(BUCKET_PERFILES)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(cliente) = cliente_sesion(&state, &headers).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let llave = match db::perfil::de_cliente(&state.pool, cliente).await {
        Ok(Some(p)) => p.fotoperfil,
        Ok(None) => None,
        Err(e) => return error_bd(e),
    };
    let Some(llave) = llave else { return StatusCode::NOT_FOUND.into_response() };
    match minio.bajar(&llave).await {
        // La URL lleva la llave en `v`: cambia con cada foto, así que se puede guardar mucho.
        Ok(bytes) => ([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=2592000")], bytes)
            .into_response(),
        Err(e) => {
            tracing::error!("perfil foto: {e}");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
