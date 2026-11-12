//! La cuenta de la caja en vivo, para la app del cliente.
//!
//! El POS publica en NATS `caja.cuenta` la cuenta completa en cada cambio, con un cliente
//! elegido (ver `inventario_papeleria/docs/decisiones/2026-11-12-cuenta-en-vivo-en-la-app.md`).
//! Es NATS núcleo, sin stream: lo que llega mientras xplaya está apagado se pierde, y no importa,
//! porque el siguiente cambio trae la cuenta entera.
//!
//! Aquí se guarda en memoria la última cuenta de cada cliente y se reparte a los celulares
//! conectados (`routes/cuenta.rs`, por SSE). El JSON se reenvía tal como llegó: xplaya solo
//! lee el encabezado para saber de quién es y si es más nueva que la que tiene.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::config::Config;

const SUBJECT: &str = "caja.cuenta";

/// Una cuenta abierta que deja de cambiar tanto tiempo se da por abandonada (la caja canceló
/// o cerró la pantalla sin cobrar; esos casos no mandan `cerrada`).
const VIDA_ABIERTA: Duration = Duration::from_secs(15 * 60);
/// Una cobrada se sigue mostrando un rato, para quien abre la app justo después de pagar.
const VIDA_COBRADA: Duration = Duration::from_secs(5 * 60);
/// Después de esto se borra de la memoria. Mientras tanto sirve para descartar mensajes viejos
/// de la misma cuenta que lleguen tarde.
const VIDA_EN_MEMORIA: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Estado {
    Abierta,
    Cobrada,
    Cerrada,
}

/// Lo único que xplaya lee del mensaje; el resto (renglones, total, ventaId) va directo a la app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Encabezado {
    pub cuenta_id: Uuid,
    pub secuencia: i64,
    pub cliente_id: Uuid,
    pub estado: Estado,
}

/// Cada pantalla de venta del POS tiene su `cuentaId` y numera sus mensajes. De la misma
/// cuenta, solo vale uno más nuevo; una cuenta distinta (otra venta) siempre reemplaza.
pub fn es_mas_nueva(anterior: Option<&Encabezado>, nueva: &Encabezado) -> bool {
    match anterior {
        None => true,
        Some(a) => a.cuenta_id != nueva.cuenta_id || nueva.secuencia > a.secuencia,
    }
}

/// Si la app debe mostrarla, según su estado y cuánto lleva sin cambiar.
pub fn se_muestra(estado: Estado, sin_cambios: Duration) -> bool {
    match estado {
        Estado::Abierta => sin_cambios <= VIDA_ABIERTA,
        Estado::Cobrada => sin_cambios <= VIDA_COBRADA,
        Estado::Cerrada => false,
    }
}

struct Ultima {
    encabezado: Encabezado,
    json: Arc<str>,
    recibida: Instant,
}

/// Un cambio para un cliente: el JSON que va a la app, o `None` si ya no hay cuenta que mostrar.
#[derive(Clone)]
pub struct Cambio {
    pub cliente_id: Uuid,
    pub json: Option<Arc<str>>,
}

pub struct Cajas {
    ultimas: Mutex<HashMap<Uuid, Ultima>>,
    cambios: broadcast::Sender<Cambio>,
}

impl Cajas {
    pub fn new() -> Self {
        // Cada celular conectado tiene su receptor; si se atrasa más de 64 cambios, vuelve a
        // pedir la cuenta actual (`routes/cuenta.rs`).
        let (cambios, _) = broadcast::channel(64);
        Cajas { ultimas: Mutex::new(HashMap::new()), cambios }
    }

    /// Un mensaje de `caja.cuenta`. Lo viejo o lo que no se entiende se ignora.
    pub fn recibir(&self, payload: &[u8], ahora: Instant) {
        let encabezado: Encabezado = match serde_json::from_slice(payload) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("caja: mensaje que no es una cuenta, se descarta: {e}");
                return;
            }
        };
        let Ok(texto) = std::str::from_utf8(payload) else { return };
        let json: Arc<str> = Arc::from(texto);

        {
            let mut ultimas = self.ultimas.lock().unwrap();
            ultimas.retain(|_, u| ahora.duration_since(u.recibida) <= VIDA_EN_MEMORIA);
            if !es_mas_nueva(ultimas.get(&encabezado.cliente_id).map(|u| &u.encabezado), &encabezado) {
                return;
            }
            ultimas.insert(encabezado.cliente_id, Ultima { encabezado, json: json.clone(), recibida: ahora });
        }

        let json = (encabezado.estado != Estado::Cerrada).then_some(json);
        // Sin celulares conectados `send` da error; no pasa nada.
        let _ = self.cambios.send(Cambio { cliente_id: encabezado.cliente_id, json });
    }

    /// La cuenta que la app de este cliente debe mostrar ahora, si hay.
    pub fn actual(&self, cliente_id: Uuid, ahora: Instant) -> Option<Arc<str>> {
        let ultimas = self.ultimas.lock().unwrap();
        let u = ultimas.get(&cliente_id)?;
        se_muestra(u.encabezado.estado, ahora.duration_since(u.recibida)).then(|| u.json.clone())
    }

    pub fn suscribir(&self) -> broadcast::Receiver<Cambio> {
        self.cambios.subscribe()
    }
}

/// Escucha `caja.cuenta` en una conexión propia. Sin usuario de NATS no hace nada (la app
/// simplemente nunca muestra una cuenta). Si se cae, vuelve a empezar.
pub fn vigilar(cfg: &Config, cajas: Arc<Cajas>) {
    if cfg.nats_usuario.is_empty() {
        tracing::info!("caja: sin NATS_USUARIO, sin cuenta en vivo");
        return;
    }
    let cfg = cfg.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = escuchar(&cfg, &cajas).await {
                tracing::warn!("caja: NATS: {e}; reintento en 10 s");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

async fn escuchar(cfg: &Config, cajas: &Cajas) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cliente = async_nats::ConnectOptions::with_user_and_password(
        cfg.nats_usuario.clone(),
        cfg.nats_password.clone(),
    )
    .name("xplaya-caja")
    .connect(&cfg.nats_url)
    .await?;
    let mut mensajes = cliente.subscribe(SUBJECT).await?;
    tracing::info!("caja: escuchando NATS ({SUBJECT})");
    while let Some(m) = mensajes.next().await {
        cajas.recibir(&m.payload, Instant::now());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENTE: &str = "11111111-1111-1111-1111-111111111111";
    const CUENTA: &str = "22222222-2222-2222-2222-222222222222";

    fn mensaje(cuenta: &str, secuencia: i64, estado: &str) -> Vec<u8> {
        format!(
            r#"{{"cuentaId":"{cuenta}","secuencia":{secuencia},"clienteId":"{CLIENTE}","estado":"{estado}","renglones":[],"total":10}}"#
        )
        .into_bytes()
    }

    fn cliente() -> Uuid {
        CLIENTE.parse().unwrap()
    }

    #[test]
    fn guarda_la_ultima_y_la_reenvia_tal_cual() {
        let cajas = Cajas::new();
        let ahora = Instant::now();
        let m = mensaje(CUENTA, 1, "abierta");
        cajas.recibir(&m, ahora);
        assert_eq!(cajas.actual(cliente(), ahora).as_deref(), Some(std::str::from_utf8(&m).unwrap()));
    }

    #[test]
    fn ignora_un_mensaje_viejo_de_la_misma_cuenta() {
        let cajas = Cajas::new();
        let ahora = Instant::now();
        cajas.recibir(&mensaje(CUENTA, 5, "abierta"), ahora);
        cajas.recibir(&mensaje(CUENTA, 4, "cerrada"), ahora);
        assert!(cajas.actual(cliente(), ahora).unwrap().contains(r#""secuencia":5"#));
    }

    #[test]
    fn otra_cuenta_reemplaza_aunque_su_secuencia_sea_menor() {
        let cajas = Cajas::new();
        let ahora = Instant::now();
        cajas.recibir(&mensaje(CUENTA, 9, "abierta"), ahora);
        cajas.recibir(&mensaje("33333333-3333-3333-3333-333333333333", 1, "abierta"), ahora);
        assert!(cajas.actual(cliente(), ahora).unwrap().contains("33333333"));
    }

    #[test]
    fn cerrada_ya_no_se_muestra_y_avisa_sin_json() {
        let cajas = Cajas::new();
        let mut rx = cajas.suscribir();
        let ahora = Instant::now();
        cajas.recibir(&mensaje(CUENTA, 1, "abierta"), ahora);
        cajas.recibir(&mensaje(CUENTA, 2, "cerrada"), ahora);
        assert!(cajas.actual(cliente(), ahora).is_none());
        assert!(rx.try_recv().unwrap().json.is_some());
        assert!(rx.try_recv().unwrap().json.is_none());
    }

    #[test]
    fn basura_no_truena() {
        let cajas = Cajas::new();
        cajas.recibir(b"hola", Instant::now());
        cajas.recibir(br#"{"estado":"pagada"}"#, Instant::now());
    }

    #[test]
    fn vigencia_por_estado() {
        let min = |m: u64| Duration::from_secs(m * 60);
        assert!(se_muestra(Estado::Abierta, min(14)));
        assert!(!se_muestra(Estado::Abierta, min(16)));
        assert!(se_muestra(Estado::Cobrada, min(4)));
        assert!(!se_muestra(Estado::Cobrada, min(6)));
        assert!(!se_muestra(Estado::Cerrada, Duration::ZERO));
    }
}
