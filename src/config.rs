#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub content_base_url: String,
    pub site_url: String,
    pub gotenberg_url: String,
    pub imagina_url: String,
    /// Token compartido con el kiosko físico — valida POST /kiosko/pedidos (Fase 3).
    pub kiosko_token: String,
    /// URL del servicio bge-m3 para búsqueda semántica (Embeddings Fase 4).
    /// None → el fallback semántico queda apagado y la búsqueda funciona como siempre.
    pub bge_embeddings_url: Option<String>,
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
            kiosko_token: std::env::var("KIOSKO_TOKEN").unwrap_or_default(),
            bge_embeddings_url: std::env::var("BGE_EMBEDDINGS_URL")
                .ok()
                .filter(|s| !s.is_empty()),
        }
    }
}
