//! `GET /cuenta/eventos` (app.xplaya.com): la cuenta de la caja en vivo para el celular ligado.
//!
//! Es SSE (Server-Sent Events): una petición que se queda abierta y por la que el servidor va
//! mandando eventos de texto. El navegador la abre con `new EventSource(url)` y se reconecta
//! solo si se corta. Va en una sola dirección (servidor → celular), que es lo único que hace
//! falta aquí; por eso no es WebSocket.
//!
//! Eventos:
//! - `cuenta`: el JSON de la cuenta tal como lo publicó el POS (`caja.rs`).
//! - `sin-cuenta`: no hay cuenta que mostrar (se cerró, venció o nunca hubo).
//!
//! Al conectarse llega uno de los dos con lo que hay en ese momento. Solo con sesión de passkey:
//! sin ella no hay forma de saber que el celular es del cliente.

use std::{convert::Infallible, sync::Arc, time::{Duration, Instant}};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use tokio::sync::broadcast::{error::RecvError, Receiver};
use uuid::Uuid;

use crate::{
    caja::{Cajas, Cambio},
    routes::app::cliente_sesion,
    AppState,
};

/// Cada cuánto se revisa, sin cambios de la caja, si la cuenta que se está mostrando ya venció.
const REVISION: Duration = Duration::from_secs(60);

pub async fn eventos(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(cliente_id) = cliente_sesion(&state, &headers).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    // Primero suscribirse y luego leer la actual: así no se pierde un cambio entre las dos.
    let cambios = state.cajas.suscribir();
    let inicial = state.cajas.actual(cliente_id, Instant::now());
    let conexion = Conexion {
        cambios,
        cajas: state.cajas.clone(),
        cliente_id,
        mostrando: false,
        inicial: Some(inicial),
    };
    // `unfold` arma un stream llamando a `siguiente` una y otra vez; cada llamada espera el
    // próximo evento de este celular. KeepAlive manda un comentario cada 15 s para que ni el
    // túnel de Cloudflare ni el celular den la conexión por muerta.
    let stream = futures_util::stream::unfold(conexion, siguiente);
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

struct Conexion {
    cambios: Receiver<Cambio>,
    cajas: Arc<Cajas>,
    cliente_id: Uuid,
    /// Si lo último que se mandó fue una cuenta (para avisar cuando venza).
    mostrando: bool,
    /// Lo que hay al conectarse; se manda primero y una sola vez.
    inicial: Option<Option<Arc<str>>>,
}

impl Conexion {
    fn evento(&mut self, json: Option<Arc<str>>) -> Result<Event, Infallible> {
        self.mostrando = json.is_some();
        Ok(match json {
            Some(j) => Event::default().event("cuenta").data(&*j),
            None => Event::default().event("sin-cuenta").data(""),
        })
    }
}

async fn siguiente(mut c: Conexion) -> Option<(Result<Event, Infallible>, Conexion)> {
    if let Some(inicial) = c.inicial.take() {
        let e = c.evento(inicial);
        return Some((e, c));
    }
    loop {
        let json = tokio::select! {
            r = c.cambios.recv() => match r {
                Ok(cambio) if cambio.cliente_id == c.cliente_id => cambio.json,
                Ok(_) => continue, // de otro cliente
                // Se atrasó (muchos cambios de golpe): basta con la cuenta de ahora.
                Err(RecvError::Lagged(_)) => c.cajas.actual(c.cliente_id, Instant::now()),
                Err(RecvError::Closed) => return None,
            },
            _ = tokio::time::sleep(REVISION) => {
                if c.mostrando && c.cajas.actual(c.cliente_id, Instant::now()).is_none() {
                    None
                } else {
                    continue;
                }
            }
        };
        let e = c.evento(json);
        return Some((e, c));
    }
}
