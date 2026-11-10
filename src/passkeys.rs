//! Passkeys (WebAuthn) de la app de clientes, con el crate `webauthn-rs`.
//!
//! Una passkey es un par de llaves que vive en el celular (y se respalda en la cuenta de
//! Apple o Google). El servidor guarda solo la llave pública; para entrar, el celular firma
//! un reto con la huella o la cara. Nada que adivinar ni que robar de la base de datos.
//!
//! Cada ceremonia (crear la passkey, entrar) tiene dos idas al servidor: `inicio` arma el
//! reto y guarda su estado; `fin` revisa la respuesta del celular contra ese estado. El
//! estado vive en memoria (`Pendientes`) unos minutos: xplaya corre con una sola réplica,
//! así que no hace falta guardarlo en la BD. Si el pod se reinicia a media ceremonia, el
//! cliente solo tiene que volver a tocar el botón.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use uuid::Uuid;
use webauthn_rs::prelude::{CreationChallengeResponse, Url, Webauthn, WebauthnBuilder};

use crate::config::Config;

/// Arma el `Webauthn` con el dominio de las passkeys (`PASSKEY_RP_ID`) y el origen de la
/// app (`APP_URL`). Truena al arrancar si no cuadran: mejor que fallar con cada cliente.
///
/// También acepta el sitio (`SITE_URL`) cuando está dentro del RP ID (en producción,
/// xplaya.com): su carrito tiene "Entrar con huella" con la misma passkey y crea su propia
/// sesión. En desarrollo (`localhost` contra `app.localhost`) no aplica.
pub fn webauthn(config: &Config) -> Webauthn {
    let origen = Url::parse(&config.app_url).expect("APP_URL no es una URL válida");
    let mut builder = WebauthnBuilder::new(&config.passkey_rp_id, &origen)
        .expect("PASSKEY_RP_ID tiene que ser el dominio de APP_URL o uno de sus padres")
        .rp_name("Papelería xplaya");
    if let Ok(sitio) = Url::parse(&config.site_url)
        && let Some(host) = sitio.host_str()
        && (host == config.passkey_rp_id || host.ends_with(&format!(".{}", config.passkey_rp_id)))
    {
        builder = builder.append_allowed_origin(&sitio);
    }
    builder.build().expect("Configuración de passkeys inválida")
}

/// Opciones para `navigator.credentials.create()`, pidiendo una passkey **descubrible**:
/// que el celular la ofrezca solo, sin que le digamos de quién es. Hace falta porque en
/// iPhone la app instalada no comparte cookies con Safari: al abrirla por primera vez no
/// sabemos quién es, y el celular tiene que proponer la passkey.
///
/// `webauthn-rs` pide passkeys con `residentKey: "discouraged"` (y su tipo para cambiarlo
/// no se exporta), así que se ajusta el JSON que va al navegador. La verificación del
/// servidor no depende de ese campo.
pub fn opciones_descubrible(ccr: &CreationChallengeResponse) -> serde_json::Value {
    let mut json = serde_json::to_value(ccr).unwrap_or_default();
    if let Some(sel) = json
        .pointer_mut("/publicKey/authenticatorSelection")
        .and_then(|v| v.as_object_mut())
    {
        sel.insert("residentKey".into(), "required".into());
        sel.insert("requireResidentKey".into(), true.into());
    }
    json
}

/// Datos que viven unos minutos en memoria, cada uno con su id aleatorio.
pub struct Pendientes<T> {
    mapa: Mutex<HashMap<Uuid, (Instant, T)>>,
    vida: Duration,
}

impl<T> Pendientes<T> {
    pub fn new(vida: Duration) -> Self {
        Self { mapa: Mutex::new(HashMap::new()), vida }
    }

    /// Guarda `valor` con un id nuevo y lo devuelve.
    pub fn guardar(&self, valor: T) -> Uuid {
        let id = Uuid::new_v4();
        self.guardar_con(id, valor);
        id
    }

    /// Guarda `valor` con un id dado (reemplaza al anterior). Aprovecha para tirar lo vencido.
    pub fn guardar_con(&self, id: Uuid, valor: T) {
        let mut mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        let vida = self.vida;
        mapa.retain(|_, (creado, _)| creado.elapsed() < vida);
        mapa.insert(id, (Instant::now(), valor));
    }

    /// Saca el valor (solo se puede usar una vez). `None` si no existe o ya venció.
    pub fn tomar(&self, id: Uuid) -> Option<T> {
        let mut mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        let (creado, valor) = mapa.remove(&id)?;
        (creado.elapsed() < self.vida).then_some(valor)
    }
}

impl<T: Clone> Pendientes<T> {
    /// Copia del valor sin sacarlo.
    pub fn ver(&self, id: Uuid) -> Option<T> {
        let mapa = self.mapa.lock().unwrap_or_else(|e| e.into_inner());
        let (creado, valor) = mapa.get(&id)?;
        (creado.elapsed() < self.vida).then(|| valor.clone())
    }
}

/// "iPhone", "Android" o "Otro", para la ficha del cliente en el POS.
pub fn dispositivo(user_agent: Option<&str>) -> &'static str {
    let ua = user_agent.unwrap_or("").to_lowercase();
    if ua.contains("iphone") || ua.contains("ipad") {
        "iPhone"
    } else if ua.contains("android") {
        "Android"
    } else {
        "Otro"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tomar_solo_una_vez() {
        let p = Pendientes::new(Duration::from_secs(60));
        let id = p.guardar(7);
        assert_eq!(p.ver(id), Some(7));
        assert_eq!(p.tomar(id), Some(7));
        assert_eq!(p.tomar(id), None);
    }

    #[test]
    fn vencidos_no_sirven() {
        let p = Pendientes::new(Duration::ZERO);
        let id = p.guardar("x");
        assert_eq!(p.tomar(id), None);
    }

    #[test]
    fn detecta_dispositivo() {
        assert_eq!(dispositivo(Some("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0)")), "iPhone");
        assert_eq!(dispositivo(Some("Mozilla/5.0 (Linux; Android 15)")), "Android");
        assert_eq!(dispositivo(None), "Otro");
    }
}
