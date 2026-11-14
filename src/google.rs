//! "Entrar con Google" por el Keycloak del cluster (realm `xplaya`, Google como proveedor).
//!
//! Para celulares que no pueden crear passkey (Android sin bloqueo de pantalla). Es el flujo
//! *authorization code* de OpenID Connect con PKCE:
//!
//! 1. `url_ida` arma la URL de Keycloak con `kc_idp_hint=google`, para que Keycloak mande
//!    directo a Google sin mostrar su propia pantalla.
//! 2. Google y Keycloak regresan a `{APP_URL}/google/regreso?code=…&state=…`.
//! 3. `cuenta` cambia el `code` por un token (con el secreto del cliente, de servidor a
//!    servidor) y le pide a Keycloak el `sub` del usuario: su id fijo en el realm.
//!
//! Se guarda el `sub`, no el correo: el correo puede cambiar y no hace falta conocerlo.
//! Como el token llega directo de Keycloak por HTTPS (no pasa por el navegador), no hace falta
//! revisar la firma del JWT: se usa el endpoint `userinfo` con el access token.
//!
//! El `state` es la llave de lo pendiente en memoria, no una cookie: en iPhone la app instalada
//! no comparte cookies con Safari, y el regreso puede caer en cualquiera de los dos.

use base64ct::{Base64UrlUnpadded, Encoding};
use reqwest::Url;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::Config;

/// Id del cliente en Keycloak (Clients → `xplaya`).
const CLIENT_ID: &str = "xplaya";

/// Configuración de Keycloak. `None` si falta el issuer o el secreto: el botón no aparece.
#[derive(Clone)]
pub struct Google {
    /// `https://…/realms/xplaya`; de aquí salen las demás URLs del realm.
    issuer: String,
    secreto: String,
    /// `{APP_URL}/google/regreso`; debe estar en *Valid redirect URIs* del cliente.
    regreso: String,
}

/// Qué hacer al volver de Google.
#[derive(Clone)]
pub enum Destino {
    /// Terminar el alta con este ticket (`AltaPendiente` de `routes/alta.rs`).
    Alta { ticket: Uuid },
    /// Volver a entrar a la app (sesión perdida).
    Entrar,
}

/// Lo que espera en memoria entre la ida a Google y el regreso, con el `state` como id.
#[derive(Clone)]
pub struct Ida {
    pub destino: Destino,
    /// El secreto de PKCE: Keycloak solo entrega el token a quien lo conoce.
    verificador: String,
}

impl Google {
    pub fn desde_config(cfg: &Config) -> Option<Self> {
        if cfg.keycloak_issuer.is_empty() || cfg.keycloak_client_secret.is_empty() {
            return None;
        }
        Some(Self {
            issuer: cfg.keycloak_issuer.trim_end_matches('/').to_string(),
            secreto: cfg.keycloak_client_secret.clone(),
            regreso: format!("{}/google/regreso", cfg.app_url.trim_end_matches('/')),
        })
    }

    /// Prepara la ida: el `Ida` que se guarda y la URL a la que se manda al celular, que
    /// lleva `state` (el id con que se guardó).
    pub fn ida(&self, destino: Destino) -> Ida {
        Ida { destino, verificador: Base64UrlUnpadded::encode_string(&rand::random::<[u8; 32]>()) }
    }

    pub fn url_ida(&self, state: Uuid, ida: &Ida) -> String {
        let reto = Base64UrlUnpadded::encode_string(&Sha256::digest(ida.verificador.as_bytes()));
        let mut url = Url::parse(&format!("{}/protocol/openid-connect/auth", self.issuer))
            .expect("KEYCLOAK_ISSUER no es una URL");
        url.query_pairs_mut()
            .append_pair("client_id", CLIENT_ID)
            .append_pair("response_type", "code")
            .append_pair("scope", "openid")
            .append_pair("redirect_uri", &self.regreso)
            .append_pair("state", &state.to_string())
            .append_pair("code_challenge", &reto)
            .append_pair("code_challenge_method", "S256")
            .append_pair("kc_idp_hint", "google")
            // Que Google siempre pregunte qué cuenta usar (en el celular puede haber varias).
            .append_pair("prompt", "select_account");
        url.into()
    }

    /// Cambia el `code` del regreso por el `sub` del usuario en Keycloak.
    pub async fn cuenta(&self, http: &reqwest::Client, code: &str, ida: &Ida) -> Result<String, String> {
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
        }
        #[derive(Deserialize)]
        struct UserInfo {
            sub: String,
        }

        let token: Token = http
            .post(format!("{}/protocol/openid-connect/token", self.issuer))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &self.regreso),
                ("client_id", CLIENT_ID),
                ("client_secret", &self.secreto),
                ("code_verifier", &ida.verificador),
            ])
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("token: {e}"))?
            .json()
            .await
            .map_err(|e| format!("token json: {e}"))?;

        let info: UserInfo = http
            .get(format!("{}/protocol/openid-connect/userinfo", self.issuer))
            .bearer_auth(&token.access_token)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("userinfo: {e}"))?
            .json()
            .await
            .map_err(|e| format!("userinfo json: {e}"))?;
        Ok(info.sub)
    }
}
