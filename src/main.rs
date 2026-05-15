use axum::{Router, routing::get, response::Redirect};
use minijinja::{Environment, path_loader};
use tower_http::services::ServeDir;

mod config;
mod db;
mod middleware;
mod models;
mod routes;

#[derive(Clone)]
pub struct AppState {
    pub tmpl: Environment<'static>,
    pub config: config::Config,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = config::Config::from_env();
    let addr = format!("0.0.0.0:{}", cfg.port);

    let mut tmpl = Environment::new();
    tmpl.set_loader(path_loader("templates"));

    let state = AppState { tmpl, config: cfg };

    let app = Router::new()
        .route("/", get(|| async { Redirect::permanent("/productos") }))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Servidor en http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
