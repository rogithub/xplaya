//! Notificaciones Web Push de la app (APP_MONEDERO.md, sección Notificaciones).
//!
//! VAPID: un par de llaves P-256 que identifica a xplaya ante los servicios de push (Google,
//! Apple, Mozilla). El celular se suscribe con la **pública** y solo acepta mensajes firmados con
//! la **privada**, que vive en el servidor (`VAPID_PRIVATE_KEY`, SealedSecret en k3s). La pública
//! se deriva de la privada: un solo secreto que guardar.

use base64ct::{Base64UrlUnpadded, Encoding};
use web_push_native::jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair};

pub struct Vapid {
    /// Firma cada mensaje (JWT ES256 en el encabezado `Authorization`).
    #[allow(dead_code)] // se usa al mandar notificaciones (siguiente paso)
    pub par: ES256KeyPair,
    /// Llave pública en base64url (65 bytes, punto sin comprimir): el `applicationServerKey`
    /// que la app le pasa a `pushManager.subscribe()`.
    pub publica: String,
}

impl Vapid {
    /// `None` si la variable está vacía: la app no ofrece notificaciones.
    pub fn desde_base64(privada: &str) -> Option<Self> {
        let privada = privada.trim();
        if privada.is_empty() {
            return None;
        }
        let bytes = Base64UrlUnpadded::decode_vec(privada)
            .expect("VAPID_PRIVATE_KEY tiene que ser base64url sin relleno (32 bytes)");
        let par = ES256KeyPair::from_bytes(&bytes)
            .expect("VAPID_PRIVATE_KEY no es una llave P-256 válida");
        let publica = Base64UrlUnpadded::encode_string(&par.public_key().public_key().to_bytes_uncompressed());
        Some(Self { par, publica })
    }
}
