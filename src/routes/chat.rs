//! Chat de la app con la papelería (`CHAT.md`), solo con sesión de passkey y para quien lo
//! pueda ver (`CHAT_CLIENTES_PRUEBA`, `reglas::chat_visible`).
//!
//! Lo que hace el cliente no se guarda aquí: se publica en NATS (`chat/nats.rs`) y el consumer
//! lo guarda al volver del stream, igual que lo de la caja. La app se entera por el SSE de
//! `/chat/eventos`, que solo trae lo que ya está en `Mensajes`.
//!
//! - `GET /chat`: la pantalla, con los últimos mensajes.
//! - `GET /chat/anteriores?antes={id}`: más historial al subir.
//! - `GET /chat/eventos`: SSE (`vista::EnVivo`), como `routes/cuenta.rs`.
//! - `POST /chat/mensaje`, `/chat/leido`, `/chat/{id}/editar`, `/chat/{id}/eliminar`,
//!   `/chat/escribiendo`.
//! - `GET /chat/sin-leer`: el número del globito del menú.

use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Redirect, Response,
    },
    Json,
};
use chrono::{DateTime, FixedOffset, Utc};
use minijinja::context;
use serde::Deserialize;
use tokio::sync::broadcast::{error::RecvError, Receiver};
use uuid::Uuid;

use crate::{
    caja::EnPantalla,
    chat::{
        contrato::{Editado, Eliminado, Entrante, Evento, Lado, Leido, Tema, VERSION},
        nats::Cambio as CambioEnVivo,
        reglas::{self, Ajustes, Cambio},
    },
    db,
    routes::{alta::error_json, app::cliente_sesion},
    AppState,
};

/// Mensajes por página.
const PAGINA: i64 = 50;

/// Cliente con sesión que puede ver el chat, y los ajustes.
async fn del_chat(state: &AppState, headers: &HeaderMap) -> Result<(Uuid, Ajustes), StatusCode> {
    let cliente = cliente_sesion(state, headers).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let ajustes = db::chat::ajustes(&state.pool).await.map_err(error_bd)?;
    if !reglas::chat_visible(ajustes.clientes_prueba.as_deref(), cliente) {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok((cliente, ajustes))
}

fn error_bd(e: sqlx::Error) -> StatusCode {
    tracing::error!("chat: BD: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn ahora() -> DateTime<FixedOffset> {
    Utc::now().fixed_offset()
}

/// Publicar en NATS falló: la app lo deja en "enviando…" y lo vuelve a intentar.
fn sin_nats(e: String) -> Response {
    tracing::warn!("chat: NATS: {e}");
    error_json(StatusCode::SERVICE_UNAVAILABLE, "No se pudo mandar. Lo intentamos de nuevo en un momento.")
}

// ── Pantalla e historial ────────────────────────────────────────────────────

pub async fn pagina(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Ok((cliente, ajustes)) = del_chat(&state, &headers).await else {
        return Redirect::to("/").into_response();
    };
    let datos = async {
        let mensajes = db::chat::conversacion(&state.pool, cliente, None, PAGINA).await?;
        let bloqueado = db::chat::bloqueado(&state.pool, cliente).await?;
        Ok::<_, sqlx::Error>((mensajes, bloqueado))
    };
    let (mensajes, bloqueado) = match datos.await {
        Ok(d) => d,
        Err(e) => return error_bd(e).into_response(),
    };
    let ctx = context! {
        en_app => true,
        nav => "chat",
        chat => context! {
            mensajes,
            bloqueado,
            minutosEditar => ajustes.minutos_editar,
            hayMas => mensajes.len() as i64 == PAGINA,
            pagina => PAGINA,
        },
    };
    match state.tmpl.get_template("app/chat.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("Render app/chat.html: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct AnterioresParams {
    antes: Uuid,
}

pub async fn anteriores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<AnterioresParams>,
) -> Result<Response, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    let mensajes = db::chat::conversacion(&state.pool, cliente, Some(p.antes), PAGINA).await.map_err(error_bd)?;
    Ok(Json(mensajes).into_response())
}

pub async fn sin_leer(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    let n = db::chat::sin_leer(&state.pool, cliente).await.map_err(error_bd)?;
    Ok(Json(serde_json::json!({ "sinLeer": n })).into_response())
}

// ── En vivo ─────────────────────────────────────────────────────────────────

pub async fn eventos(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (cliente, _) = match del_chat(&state, &headers).await {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    let conexion = Conexion {
        cambios: state.chat.suscribir(),
        cliente,
        // Con el chat abierto tampoco hacen falta avisos por Web Push (etapa 6).
        _en_pantalla: state.cajas.en_pantalla(cliente),
    };
    let stream = futures_util::stream::unfold(conexion, siguiente);
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

struct Conexion {
    cambios: Receiver<CambioEnVivo>,
    cliente: Uuid,
    _en_pantalla: EnPantalla,
}

async fn siguiente(mut c: Conexion) -> Option<(Result<Event, Infallible>, Conexion)> {
    loop {
        let evento = match c.cambios.recv().await {
            Ok((cliente, cambio)) if cliente == c.cliente => {
                let json = serde_json::to_string(&*cambio).unwrap_or_else(|_| "null".into());
                Event::default().event(cambio.nombre()).data(json)
            }
            Ok(_) => continue, // de otro cliente
            // Se atrasó (muchos cambios de golpe): la app vuelve a pedir la conversación.
            Err(RecvError::Lagged(_)) => Event::default().event("recargar").data("null"),
            Err(RecvError::Closed) => return None,
        };
        return Some((Ok(evento), c));
    }
}

// ── Lo que hace el cliente ──────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevoMensaje {
    /// Lo genera el celular: un reintento sin señal no se guarda dos veces.
    id: Uuid,
    texto: String,
    responde_a: Option<Uuid>,
}

/// Responde cuando NATS ya lo tiene (el ✓). Se guarda y llega por el SSE un instante después.
pub async fn mandar(State(state): State<AppState>, headers: HeaderMap, Json(m): Json<NuevoMensaje>) -> Result<Response, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    let bloqueado = db::chat::bloqueado(&state.pool, cliente).await.map_err(error_bd)?;
    let mensaje = match reglas::del_cliente(cliente, m.id, &m.texto, m.responde_a, bloqueado, ahora()) {
        Ok(msg) => msg,
        Err(motivo) => return Ok(error_json(StatusCode::BAD_REQUEST, motivo)),
    };
    let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Mensaje };
    Ok(match state.chat.publicar(&tema, &mensaje, Some(mensaje.id)).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => sin_nats(e),
    })
}

/// El cliente vio la conversación: todo lo que le escribieron hasta ahora queda leído.
pub async fn leido(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Leido };
    let leido = Leido { v: VERSION, cliente_id: cliente, hasta: ahora() };
    Ok(match state.chat.publicar(&tema, &leido, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => sin_nats(e),
    })
}

#[derive(Deserialize)]
pub struct Edicion {
    texto: String,
}

/// Se revisa con las mismas reglas que usará el consumer, para contestar el error al momento.
pub async fn editar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(e): Json<Edicion>,
) -> Result<Response, StatusCode> {
    let (cliente, ajustes) = del_chat(&state, &headers).await?;
    if db::chat::bloqueado(&state.pool, cliente).await.map_err(error_bd)? {
        return Ok(error_json(StatusCode::BAD_REQUEST, "La papelería bloqueó tu chat."));
    }
    if e.texto.chars().count() > reglas::MAX_CARACTERES {
        return Ok(error_json(StatusCode::BAD_REQUEST, "El mensaje es muy largo."));
    }
    let editado = Editado { v: VERSION, cliente_id: cliente, id, texto: e.texto, fecha: ahora() };
    let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Editado };
    let guardado = db::chat::guardado(&state.pool, id).await.map_err(error_bd)?;
    if let Cambio::Ignorar(motivo) = reglas::aplicar(&tema, Entrante::Editado(editado.clone()), guardado.as_ref(), 0, &ajustes) {
        tracing::info!("chat: {cliente} no pudo editar {id}: {motivo}");
        return Ok(error_json(StatusCode::BAD_REQUEST, "Este mensaje ya no se puede editar."));
    }
    Ok(match state.chat.publicar(&tema, &editado, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => sin_nats(e),
    })
}

/// "Eliminar para todos". Con el chat bloqueado también se puede.
pub async fn eliminar(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>) -> Result<Response, StatusCode> {
    let (cliente, ajustes) = del_chat(&state, &headers).await?;
    let eliminado = Eliminado { v: VERSION, cliente_id: cliente, id, fecha: ahora() };
    let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Eliminado };
    let guardado = db::chat::guardado(&state.pool, id).await.map_err(error_bd)?;
    if let Cambio::Ignorar(motivo) = reglas::aplicar(&tema, Entrante::Eliminado(eliminado.clone()), guardado.as_ref(), 0, &ajustes) {
        tracing::info!("chat: {cliente} no pudo eliminar {id}: {motivo}");
        return Ok(error_json(StatusCode::BAD_REQUEST, "Este mensaje no se puede eliminar."));
    }
    Ok(match state.chat.publicar(&tema, &eliminado, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => sin_nats(e),
    })
}

/// La app lo manda como mucho cada 3 s mientras el cliente escribe.
pub async fn escribiendo(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    state.chat.escribiendo(cliente).await;
    Ok(StatusCode::NO_CONTENT)
}
