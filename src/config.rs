#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub content_base_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            content_base_url: std::env::var("CONTENT_BASE_URL")
                .unwrap_or_else(|_| "https://cntnt.xplaya.com".to_string()),
        }
    }
}
