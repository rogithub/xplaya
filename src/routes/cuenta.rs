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
//!
//! `GET /cuenta/venta/{venta_id}`: al llegar una cuenta `cobrada`, la app pide aquí cuánto ganó
//! de monedero y su saldo nuevo. El POS no lo manda: se calcula al guardar la venta.

use std::{convert::Infallible, sync::Arc, time::{Duration, Instant}};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde::Serialize;
use tokio::sync::broadcast::{error::RecvError, Receiver};
use uuid::Uuid;

use crate::{
    caja::{Cajas, Cambio, EnPantalla},
    db,
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
        // Mientras la conexión siga abierta, este cliente no recibe avisos por Web Push.
        _en_pantalla: state.cajas.en_pantalla(cliente_id),
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
    _en_pantalla: EnPantalla,
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
            // `null` y no vacío: EventSource no entrega los eventos sin datos.
            None => Event::default().event("sin-cuenta").data("null"),
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

/// Lo que la app muestra al cobrar. Montos ya con formato (`$12.50`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Premio {
    tiene_monedero: bool,
    ganado: String,
    /// Lo que pagó con su monedero en esta venta, si usó.
    usado: Option<String>,
    saldo: String,
    recibo: String,
}

pub async fn venta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(venta_id): Path<Uuid>,
) -> Result<Json<Premio>, StatusCode> {
    let cliente_id = cliente_sesion(&state, &headers).await.ok_or(StatusCode::UNAUTHORIZED)?;
    // El mismo cálculo que el recibo: monedero generado, usado y saldo después de la venta.
    let recibo = db::monedero::recibo(&state.pool, venta_id)
        .await
        .map_err(|e| {
            tracing::error!("cuenta: recibo {venta_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    // Solo sus propias ventas: con otro `ventaId` no se entera de nada ajeno.
    if recibo.cliente_id != Some(cliente_id) {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(Premio {
        tiene_monedero: recibo.tiene_monedero,
        ganado: recibo.monedero_generado,
        usado: (recibo.pago_monedero != "$0.00").then_some(recibo.pago_monedero),
        saldo: recibo.saldo_monedero,
        recibo: format!("{}/recibo/{}", state.config.site_url, venta_id),
    }))
}
