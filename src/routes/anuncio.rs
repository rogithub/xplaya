//! "Tu anuncio en la tienda": cada cliente con sesión pone un anuncio (imagen 9:16 o texto sobre
//! un color) para la franja derecha del kiosko. Espera a que la caja lo apruebe en el POS
//! (Contactos → Anuncios del kiosko) y sale 7 días; el cliente lo cambia o lo quita cuando quiera.
//!
//! La imagen, en 1080×1920 (`anuncio.rs`), en MinIO (bucket `papeleria-anuncios`); la BD, en
//! `db/anuncios.rs`. El kiosko los pide en `/kiosko/anuncios` (solo lo aprobado y vigente).

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use minijinja::context;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    anuncio::{self, Situacion},
    avisos::codificar_url,
    db::{self, anuncios::Contenido},
    minio::{Minio, BUCKET_ANUNCIOS},
    routes::{app::cliente_sesion, kiosko::es_kiosko},
    AppState,
};

/// Sin MinIO no hay anuncios (404); sin sesión, a la portada a entrar con la huella.
#[allow(clippy::result_large_err)] // se responde de inmediato; no viaja por varias capas
async fn requisitos(state: &AppState, headers: &HeaderMap) -> Result<(Minio, Uuid), Response> {
    let minio = state.minio.as_ref().ok_or_else(|| StatusCode::NOT_FOUND.into_response())?.en(BUCKET_ANUNCIOS);
    let cliente_id = cliente_sesion(state, headers).await.ok_or_else(|| Redirect::to("/").into_response())?;
    Ok((minio, cliente_id))
}

fn error_bd(e: sqlx::Error) -> Response {
    tracing::error!("BD anuncio: {e}");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

fn con_error(msg: &str) -> Response {
    Redirect::to(&format!("/anuncio?error={}", codificar_url(msg))).into_response()
}

fn ahora() -> chrono::NaiveDateTime {
    // Cancún está en UTC-5 todo el año; las fechas de la BD, también.
    let cancun = chrono::FixedOffset::west_opt(5 * 3600).expect("UTC-5");
    chrono::Utc::now().with_timezone(&cancun).naive_local()
}

/// Una línea para el renglón del monedero: en qué va su anuncio, o la invitación.
pub async fn resumen(state: &AppState, cliente_id: Uuid) -> Option<String> {
    state.minio.as_ref()?;
    let a = db::anuncios::de_cliente(&state.pool, cliente_id)
        .await
        .inspect_err(|e| tracing::error!("anuncio resumen: {e}"))
        .ok()?;
    Some(match a {
        None => "Gratis: tu publicidad en la pantalla de la tienda".to_string(),
        Some(a) => match anuncio::situacion(&a.estado, a.motivo.as_deref(), a.fechavence, ahora()) {
            Situacion::Esperando => "Esperando aprobación".to_string(),
            Situacion::EnKiosko { dias: 1 } => "En la pantalla · queda 1 día".to_string(),
            Situacion::EnKiosko { dias } => format!("En la pantalla · quedan {dias} días"),
            Situacion::NoAprobado { .. } => "No se aprobó: toca para ver por qué".to_string(),
            Situacion::Vencido => "Ya terminó: vuelve a mandarlo".to_string(),
        },
    })
}

// ── GET /anuncio ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PaginaParams {
    pub error: Option<String>,
    pub ok: Option<String>,
}

pub async fn pagina(State(state): State<AppState>, headers: HeaderMap, Query(p): Query<PaginaParams>) -> Response {
    let cliente_id = match requisitos(&state, &headers).await {
        Ok((_, id)) => id,
        Err(r) => return r,
    };
    let anuncio = match db::anuncios::de_cliente(&state.pool, cliente_id).await {
        Ok(a) => a,
        Err(e) => return error_bd(e),
    };
    let situacion = anuncio
        .as_ref()
        .map(|a| anuncio::situacion(&a.estado, a.motivo.as_deref(), a.fechavence, ahora()));
    let html = state.tmpl.get_template("app/anuncio.html").and_then(|t| {
        t.render(context! {
            en_app => true,
            nav => "monedero",
            anuncio,
            situacion,
            // Versión para la URL de la imagen: la del celular no se queda con la anterior.
            version => anuncio.as_ref().map(|a| a.fechaenviado.and_utc().timestamp()),
            colores => anuncio::COLORES,
            max_texto => anuncio::MAX_TEXTO,
            max_mb => anuncio::MAX_BYTES / (1024 * 1024),
            error => p.error,
            ok => p.ok,
        })
    });
    match html {
        Ok(h) => Html(h).into_response(),
        Err(e) => {
            tracing::error!("Render app/anuncio: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── POST /anuncio (multipart: tipo, imagen | texto + color) ─────────────────

pub async fn guardar(State(state): State<AppState>, headers: HeaderMap, mut multipart: Multipart) -> Response {
    let (minio, cliente_id) = match requisitos(&state, &headers).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    const GRANDE: &str = "Tu imagen es muy grande o no llegó completa. Máximo 10 MB.";
    let (mut tipo, mut imagen, mut texto, mut color) = (String::new(), None, String::new(), String::new());
    loop {
        let campo = match multipart.next_field().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(_) => return con_error(GRANDE),
        };
        match campo.name().unwrap_or("") {
            "imagen" => match campo.bytes().await {
                Ok(b) if !b.is_empty() => imagen = Some(b),
                Ok(_) => {}
                Err(_) => return con_error(GRANDE),
            },
            nombre => {
                let nombre = nombre.to_string();
                let valor = campo.text().await.unwrap_or_default();
                match nombre.as_str() {
                    "tipo" => tipo = valor,
                    "texto" => texto = valor,
                    "color" => color = valor,
                    _ => {}
                }
            }
        }
    }

    let contenido_imagen;
    let texto_limpio;
    let contenido = match tipo.as_str() {
        "imagen" => {
            let Some(bytes) = imagen else { return con_error("Elige una imagen.") };
            if bytes.len() > anuncio::MAX_BYTES {
                return con_error("Tu imagen pasa de 10 MB.");
            }
            // Decodificar y redimensionar es trabajo de CPU: fuera del hilo de tokio.
            let jpeg = match tokio::task::spawn_blocking(move || anuncio::preparar_imagen(&bytes)).await {
                Ok(Ok(j)) => j,
                Ok(Err(msg)) => return con_error(&msg),
                Err(e) => {
                    tracing::error!("anuncio imagen: {e}");
                    return con_error("No pudimos procesar tu imagen. Intenta de nuevo.");
                }
            };
            contenido_imagen = format!("{cliente_id}/{}.jpg", Uuid::new_v4());
            if let Err(e) = minio.subir(&contenido_imagen, jpeg, "image/jpeg").await {
                tracing::error!("anuncio: {e}");
                return con_error("No pudimos guardar tu imagen. Intenta de nuevo.");
            }
            Contenido::Imagen { objeto: &contenido_imagen }
        }
        "texto" => {
            // Sin renglones vacíos de más (el kiosko respeta los saltos de línea).
            texto_limpio = texto.trim().replace("\r\n", "\n");
            if texto_limpio.is_empty() {
                return con_error("Escribe tu anuncio.");
            }
            if texto_limpio.chars().count() > anuncio::MAX_TEXTO {
                return con_error(&format!("Tu anuncio pasa de {} letras.", anuncio::MAX_TEXTO));
            }
            if !anuncio::color_valido(&color) {
                return con_error("Elige un color.");
            }
            Contenido::Texto { texto: &texto_limpio, color: &color }
        }
        _ => return con_error("Elige imagen o texto."),
    };

    match db::anuncios::guardar(&state.pool, cliente_id, &contenido).await {
        Ok(anterior) => {
            if let Some(objeto) = anterior
                && let Err(e) = minio.borrar(&objeto).await
            {
                tracing::warn!("anuncio, imagen anterior: {e}");
            }
            let nombre = db::anuncios::nombre_cliente(&state.pool, cliente_id).await.ok().flatten().unwrap_or_default();
            state.avisos.anuncio_nuevo(&nombre, &tipo);
            Redirect::to("/anuncio?ok=mandado").into_response()
        }
        Err(e) => {
            if let Contenido::Imagen { objeto } = contenido {
                let _ = minio.borrar(objeto).await;
            }
            error_bd(e)
        }
    }
}

// ── POST /anuncio/volver-a-mandar ───────────────────────────────────────────

pub async fn volver_a_mandar(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let cliente_id = match requisitos(&state, &headers).await {
        Ok((_, id)) => id,
        Err(r) => return r,
    };
    match db::anuncios::volver_a_mandar(&state.pool, cliente_id).await {
        Ok(0) => Redirect::to("/anuncio").into_response(),
        Ok(_) => {
            let a = db::anuncios::de_cliente(&state.pool, cliente_id).await.ok().flatten();
            let nombre = db::anuncios::nombre_cliente(&state.pool, cliente_id).await.ok().flatten().unwrap_or_default();
            state.avisos.anuncio_nuevo(&nombre, a.map(|a| a.tipo).as_deref().unwrap_or("texto"));
            Redirect::to("/anuncio?ok=mandado").into_response()
        }
        Err(e) => error_bd(e),
    }
}

// ── POST /anuncio/quitar ────────────────────────────────────────────────────

pub async fn quitar(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (minio, cliente_id) = match requisitos(&state, &headers).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    match db::anuncios::quitar(&state.pool, cliente_id).await {
        Ok(Some(Some(objeto))) => {
            // Si falla, la imagen queda huérfana en el bucket; no se ve en ningún lado.
            if let Err(e) = minio.borrar(&objeto).await {
                tracing::warn!("anuncio quitar: {e}");
            }
            Redirect::to("/anuncio?ok=quitado").into_response()
        }
        Ok(Some(None)) => Redirect::to("/anuncio?ok=quitado").into_response(),
        Ok(None) => Redirect::to("/anuncio").into_response(),
        Err(e) => error_bd(e),
    }
}

// ── GET /anuncio/imagen (la suya, para verla en la app) ─────────────────────

pub async fn imagen(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (minio, cliente_id) = match requisitos(&state, &headers).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    let objeto = match db::anuncios::de_cliente(&state.pool, cliente_id).await {
        Ok(Some(a)) => a.objeto,
        Ok(None) => None,
        Err(e) => return error_bd(e),
    };
    let Some(objeto) = objeto else { return StatusCode::NOT_FOUND.into_response() };
    jpeg(&minio, &objeto, "private, max-age=86400").await
}

async fn jpeg(minio: &Minio, objeto: &str, cache: &'static str) -> Response {
    match minio.bajar(objeto).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, cache)], bytes).into_response(),
        Err(e) => {
            tracing::error!("anuncio imagen: {e}");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

// ── Kiosko ──────────────────────────────────────────────────────────────────

/// Los anuncios que el kiosko pasa ahora (aprobados y vigentes). Sin datos del cliente.
pub async fn kiosko_lista(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !es_kiosko(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if state.minio.is_none() {
        return Json(Vec::<db::anuncios::EnKiosko>::new()).into_response();
    }
    match db::anuncios::en_kiosko(&state.pool).await {
        Ok(a) => Json(a).into_response(),
        Err(e) => error_bd(e),
    }
}

/// La imagen de un anuncio para el kiosko. La URL lleva `?v=` (cambia con cada anuncio nuevo del
/// cliente), así que el navegador del kiosko la guarda un día.
pub async fn kiosko_imagen(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>) -> Response {
    if !es_kiosko(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(minio) = state.minio.as_ref().map(|m| m.en(BUCKET_ANUNCIOS)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match db::anuncios::imagen_en_kiosko(&state.pool, id).await {
        Ok(Some(objeto)) => jpeg(&minio, &objeto, "private, max-age=86400").await,
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => error_bd(e),
    }
}

// ── Limpieza ────────────────────────────────────────────────────────────────

/// Cada día borra los rechazados y vencidos de hace más de 30 días, con su imagen.
pub fn limpiar(state: AppState) {
    let Some(minio) = state.minio.as_ref().map(|m| m.en(BUCKET_ANUNCIOS)) else { return };
    tokio::spawn(async move {
        let mut cada = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        loop {
            cada.tick().await;
            let viejos = match db::anuncios::viejos(&state.pool).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("anuncios, limpieza: {e}");
                    continue;
                }
            };
            for (id, objeto) in viejos {
                if let Some(objeto) = objeto
                    && let Err(e) = minio.borrar(&objeto).await
                {
                    tracing::warn!("anuncios, limpieza: {e}");
                    continue;
                }
                if let Err(e) = db::anuncios::borrar(&state.pool, id).await {
                    tracing::warn!("anuncios, limpieza {id}: {e}");
                }
            }
        }
    });
}
