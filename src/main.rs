use axum::{Router, routing::{get, post}, response::Redirect};
use minijinja::{Environment, path_loader};
use sqlx::PgPool;
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
    pub pool: PgPool,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = config::Config::from_env();
    let addr = format!("0.0.0.0:{}", cfg.port);

    let pool = PgPool::connect(&cfg.database_url)
        .await
        .expect("No se pudo conectar a la base de datos");

    let mut tmpl = Environment::new();
    tmpl.set_loader(path_loader("templates"));
    tmpl.add_global("site_url", minijinja::Value::from_safe_string(cfg.site_url.clone()));

    let state = AppState { tmpl, config: cfg, pool };

    let app = Router::new()
        .route("/", get(|| async { Redirect::permanent("/productos") }))
        .route("/robots.txt", get(routes::pages::robots_txt))
        .route("/sitemap.xml", get(routes::pages::sitemap_xml))
        .route("/productos", get(routes::productos::lista))
        .route("/productos/{nid}", get(routes::productos::detalle))
        .route("/carrito", get(routes::carrito::pagina))
        .route("/pedidos", post(routes::carrito::crear_pedido))
        .route("/resena", get(routes::pages::resena))
        .route("/terminos", get(routes::monedero::terminos))
        .route("/saldo", get(routes::monedero::saldo_get))
        .route("/saldo", post(routes::monedero::saldo_post))
        .route("/monedero/{cliente_id}", get(routes::monedero::app))
        .route("/recibo/{id}", get(routes::monedero::recibo))
        .route("/cotizacion/{uid}", get(routes::monedero::cotizacion))
        .route("/r/{code}", get(routes::monedero::redirigir))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Servidor en http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
