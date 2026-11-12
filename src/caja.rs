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
//!
//! Si el cliente no tiene la app en pantalla, se le avisa por Web Push una vez al abrirse la
//! cuenta y otra al cobrar (`avisar`). Son avisos del momento: sin fila en `Notificaciones`,
//! sin reintentos y con caducidad corta.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{db, push, AppState};

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
    /// Solo en `cobrada`.
    #[serde(default)]
    pub venta_id: Option<Uuid>,
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

/// Qué avisos por Web Push ya se mandaron de una cuenta.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Avisados {
    abierta: bool,
    cobrada: bool,
}

/// Qué avisar con este mensaje: uno al abrirse la cuenta y otro al cobrar, una sola vez por cuenta
/// (aunque la caja quite y vuelva a poner al cliente). Una cuenta distinta empieza de cero.
pub fn por_avisar(anterior: Option<(&Encabezado, Avisados)>, nueva: &Encabezado) -> (Option<Estado>, Avisados) {
    let mut avisados = match anterior {
        Some((a, avisados)) if a.cuenta_id == nueva.cuenta_id => avisados,
        _ => Avisados::default(),
    };
    let aviso = match nueva.estado {
        Estado::Abierta if !avisados.abierta => {
            avisados.abierta = true;
            Some(Estado::Abierta)
        }
        Estado::Cobrada if !avisados.cobrada => {
            avisados.cobrada = true;
            Some(Estado::Cobrada)
        }
        _ => None,
    };
    (aviso, avisados)
}

/// Un aviso por mandar (lo manda `avisar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aviso {
    pub cliente_id: Uuid,
    pub estado: Estado,
    pub venta_id: Option<Uuid>,
}

struct Ultima {
    encabezado: Encabezado,
    json: Arc<str>,
    recibida: Instant,
    avisados: Avisados,
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
    /// Cuántas conexiones SSE tiene abiertas cada cliente. La app solo la abre mientras está en
    /// pantalla (`cuentaCaja` en app.js), así que con alguna abierta el aviso no hace falta.
    en_pantalla: Mutex<HashMap<Uuid, usize>>,
}

/// Mientras exista, cuenta como una app en pantalla de ese cliente (`Cajas::en_pantalla`).
pub struct EnPantalla {
    cajas: Arc<Cajas>,
    cliente_id: Uuid,
}

impl Drop for EnPantalla {
    fn drop(&mut self) {
        let mut abiertas = self.cajas.en_pantalla.lock().unwrap();
        if let Some(n) = abiertas.get_mut(&self.cliente_id) {
            *n -= 1;
            if *n == 0 {
                abiertas.remove(&self.cliente_id);
            }
        }
    }
}

impl Cajas {
    pub fn new() -> Self {
        // Cada celular conectado tiene su receptor; si se atrasa más de 64 cambios, vuelve a
        // pedir la cuenta actual (`routes/cuenta.rs`).
        let (cambios, _) = broadcast::channel(64);
        Cajas { ultimas: Mutex::new(HashMap::new()), cambios, en_pantalla: Mutex::new(HashMap::new()) }
    }

    /// Un mensaje de `caja.cuenta`. Lo viejo o lo que no se entiende se ignora. Devuelve el aviso
    /// por Web Push que toca, si toca.
    pub fn recibir(&self, payload: &[u8], ahora: Instant) -> Option<Aviso> {
        let encabezado: Encabezado = match serde_json::from_slice(payload) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("caja: mensaje que no es una cuenta, se descarta: {e}");
                return None;
            }
        };
        let texto = std::str::from_utf8(payload).ok()?;
        let json: Arc<str> = Arc::from(texto);

        let aviso = {
            let mut ultimas = self.ultimas.lock().unwrap();
            ultimas.retain(|_, u| ahora.duration_since(u.recibida) <= VIDA_EN_MEMORIA);
            let anterior = ultimas.get(&encabezado.cliente_id);
            if !es_mas_nueva(anterior.map(|u| &u.encabezado), &encabezado) {
                return None;
            }
            let (aviso, avisados) = por_avisar(anterior.map(|u| (&u.encabezado, u.avisados)), &encabezado);
            ultimas.insert(encabezado.cliente_id, Ultima { encabezado, json: json.clone(), recibida: ahora, avisados });
            aviso
        };

        let json = (encabezado.estado != Estado::Cerrada).then_some(json);
        // Sin celulares conectados `send` da error; no pasa nada.
        let _ = self.cambios.send(Cambio { cliente_id: encabezado.cliente_id, json });

        aviso.map(|estado| Aviso { cliente_id: encabezado.cliente_id, estado, venta_id: encabezado.venta_id })
    }

    /// La app de este cliente abrió su conexión: está en pantalla hasta que se suelte lo devuelto.
    pub fn en_pantalla(self: &Arc<Self>, cliente_id: Uuid) -> EnPantalla {
        *self.en_pantalla.lock().unwrap().entry(cliente_id).or_default() += 1;
        EnPantalla { cajas: self.clone(), cliente_id }
    }

    pub fn esta_en_pantalla(&self, cliente_id: Uuid) -> bool {
        self.en_pantalla.lock().unwrap().contains_key(&cliente_id)
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
pub fn vigilar(state: AppState) {
    if state.config.nats_usuario.is_empty() {
        tracing::info!("caja: sin NATS_USUARIO, sin cuenta en vivo");
        return;
    }
    tokio::spawn(async move {
        loop {
            if let Err(e) = escuchar(&state).await {
                tracing::warn!("caja: NATS: {e}; reintento en 10 s");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

async fn escuchar(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = &state.config;
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
        let Some(aviso) = state.cajas.recibir(&m.payload, Instant::now()) else { continue };
        // Sin llave VAPID no hay notificaciones; con la app en pantalla, el recuadro basta.
        if state.vapid.is_some() && !state.cajas.esta_en_pantalla(aviso.cliente_id) {
            // Aparte: buscar el premio y llamar al servicio de push no frena los siguientes mensajes.
            tokio::spawn(avisar(state.clone(), aviso));
        }
    }
    Ok(())
}

/// Los dos avisos usan el mismo `tag`: el del cobro reemplaza al de la cuenta abierta.
const TAG: &str = "cuenta-caja";
/// Avisos del momento: si el celular está apagado más que esto, ya no se entregan.
const CADUCIDAD: Duration = Duration::from_secs(10 * 60);

async fn avisar(state: AppState, aviso: Aviso) {
    let Some(vapid) = state.vapid.clone() else { return };
    let (titulo, texto) = match aviso.estado {
        Estado::Abierta => ("La caja te está atendiendo".to_string(), "Toca para ver tu cuenta en vivo.".to_string()),
        Estado::Cobrada => ("¡Gracias por tu compra!".to_string(), texto_cobrada(&state, aviso).await),
        Estado::Cerrada => return,
    };
    let cuerpo = serde_json::json!({ "titulo": titulo, "texto": texto, "url": "/", "tag": TAG });
    match push::mandar_del_momento(&state, &vapid, aviso.cliente_id, cuerpo, CADUCIDAD).await {
        Ok(true) => tracing::info!("caja: aviso {:?} a {}", aviso.estado, aviso.cliente_id),
        Ok(false) => {} // sin celular con avisos activos
        Err(e) => tracing::warn!("caja: aviso {:?} a {}: {e}", aviso.estado, aviso.cliente_id),
    }
}

/// "Ganaste $0.68 en tu monedero · tu saldo: $12.40", con el mismo cálculo que el recibo. Sin
/// monedero generado (o si falla la BD), el total pagado.
async fn texto_cobrada(state: &AppState, aviso: Aviso) -> String {
    let recibo = match aviso.venta_id {
        Some(id) => db::monedero::recibo(&state.pool, id).await
            .inspect_err(|e| tracing::error!("caja: recibo {id}: {e}"))
            .ok()
            .flatten(),
        None => None,
    };
    match recibo {
        Some(r) if r.tiene_monedero && r.monedero_generado != "$0.00" => {
            format!("Ganaste {} en tu monedero · tu saldo: {}", r.monedero_generado, r.saldo_monedero)
        }
        Some(r) => format!("Pagaste {}.", r.total),
        None => "Toca para ver tu compra.".to_string(),
    }
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

    fn encabezado(cuenta: &str, estado: Estado) -> Encabezado {
        Encabezado { cuenta_id: cuenta.parse().unwrap(), secuencia: 1, cliente_id: cliente(), estado, venta_id: None }
    }

    #[test]
    fn avisa_una_vez_al_abrir_y_una_al_cobrar() {
        let abierta = encabezado(CUENTA, Estado::Abierta);
        let (a1, av) = por_avisar(None, &abierta);
        let (a2, av) = por_avisar(Some((&abierta, av)), &abierta);
        let cobrada = encabezado(CUENTA, Estado::Cobrada);
        let (a3, av) = por_avisar(Some((&abierta, av)), &cobrada);
        let (a4, _) = por_avisar(Some((&cobrada, av)), &cobrada);
        assert_eq!([a1, a2, a3, a4], [Some(Estado::Abierta), None, Some(Estado::Cobrada), None]);
    }

    #[test]
    fn quitar_y_volver_a_poner_al_cliente_no_avisa_otra_vez() {
        let abierta = encabezado(CUENTA, Estado::Abierta);
        let cerrada = encabezado(CUENTA, Estado::Cerrada);
        let (_, av) = por_avisar(None, &abierta);
        let (a, av) = por_avisar(Some((&abierta, av)), &cerrada);
        assert_eq!(a, None);
        let (a, _) = por_avisar(Some((&cerrada, av)), &abierta);
        assert_eq!(a, None);
    }

    #[test]
    fn otra_venta_vuelve_a_avisar() {
        let abierta = encabezado(CUENTA, Estado::Abierta);
        let (_, av) = por_avisar(None, &abierta);
        let otra = encabezado("33333333-3333-3333-3333-333333333333", Estado::Abierta);
        assert_eq!(por_avisar(Some((&abierta, av)), &otra).0, Some(Estado::Abierta));
    }

    #[test]
    fn recibir_devuelve_el_aviso_con_la_venta() {
        let cajas = Cajas::new();
        let ahora = Instant::now();
        assert_eq!(cajas.recibir(&mensaje(CUENTA, 1, "abierta"), ahora).map(|a| a.estado), Some(Estado::Abierta));
        assert_eq!(cajas.recibir(&mensaje(CUENTA, 2, "abierta"), ahora), None);
        let cobrada = format!(
            r#"{{"cuentaId":"{CUENTA}","secuencia":3,"clienteId":"{CLIENTE}","estado":"cobrada","renglones":[],"total":10,"ventaId":"44444444-4444-4444-4444-444444444444"}}"#
        );
        let aviso = cajas.recibir(cobrada.as_bytes(), ahora).unwrap();
        assert_eq!(aviso.estado, Estado::Cobrada);
        assert_eq!(aviso.venta_id, Some("44444444-4444-4444-4444-444444444444".parse().unwrap()));
    }

    #[test]
    fn en_pantalla_mientras_haya_una_conexion() {
        let cajas = Arc::new(Cajas::new());
        let a = cajas.en_pantalla(cliente());
        let b = cajas.en_pantalla(cliente());
        drop(a);
        assert!(cajas.esta_en_pantalla(cliente()));
        drop(b);
        assert!(!cajas.esta_en_pantalla(cliente()));
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
