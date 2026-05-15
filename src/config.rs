#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub content_base_url: String,
    pub site_url: String,
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
        }
    }
}
