#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub content_base_url: String,
    pub site_url: String,
    pub gotenberg_url: String,
    pub imagina_url: String,
    /// URL pública de la app de clientes (PWA del monedero). Las peticiones cuyo
    /// `Host` coincide con esta URL van al router de la app; el resto, al sitio.
    pub app_url: String,
    /// Dominio de las passkeys (RP ID de WebAuthn). Una passkey solo sirve en este
    /// dominio y sus subdominios: con `xplaya.com`, la de app.xplaya.com también
    /// funcionaría en el kiosko u otro subdominio. En desarrollo: `app.localhost`.
    pub passkey_rp_id: String,
    /// Token del kiosko de la tienda (`routes/kiosko.rs`). Vacío: el kiosko está apagado.
    /// Firma también los QR de 30 s, así que nadie fuera puede generar uno válido.
    pub kiosko_token: String,
    /// ntfy, para avisar a la caja cuando alguien liga en el kiosko un teléfono que ya
    /// tiene monedero. Sin token no se publica: el aviso (con sus enlaces) solo va al log,
    /// para probar en desarrollo.
    pub ntfy_url: String,
    pub ntfy_token: String,
    /// Código de tienda del Perfil de Negocio de Google (feed de inventario local).
    /// Sin él, `/feeds/google-local.xml` responde 404.
    pub google_store_code: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL no está configurada"),
            content_base_url: std::env::var("CONTENT_BASE_URL")
                .unwrap_or_else(|_| "https://cntnt.xplaya.com".to_string()),
            site_url: std::env::var("SITE_URL")
                .unwrap_or_else(|_| "https://xplaya.com".to_string()),
            gotenberg_url: std::env::var("GOTENBERG_URL")
                .unwrap_or_else(|_| "http://gotenberg-service.gotenberg.svc.cluster.local:3000".to_string()),
            imagina_url: std::env::var("IMAGINA_URL")
                .unwrap_or_else(|_| "https://imagina.xplaya.com".to_string()),
            app_url: std::env::var("APP_URL")
                .unwrap_or_else(|_| "https://app.xplaya.com".to_string()),
            passkey_rp_id: std::env::var("PASSKEY_RP_ID")
                .unwrap_or_else(|_| "xplaya.com".to_string()),
            kiosko_token: std::env::var("KIOSKO_TOKEN").unwrap_or_default().trim().to_string(),
            ntfy_url: std::env::var("NTFY_URL")
                .unwrap_or_else(|_| "https://ntfy.xplaya.com".to_string()),
            ntfy_token: std::env::var("NTFY_TOKEN").unwrap_or_default().trim().to_string(),
            google_store_code: std::env::var("GOOGLE_STORE_CODE")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }

    /// `Host` de `app_url` (sin esquema ni ruta), en minúsculas: "app.xplaya.com"
    /// en producción, "app.localhost:3000" en desarrollo.
    pub fn app_host(&self) -> String {
        let sin_esquema = self.app_url.split("://").nth(1).unwrap_or(&self.app_url);
        sin_esquema.split('/').next().unwrap_or("").to_lowercase()
    }
}
