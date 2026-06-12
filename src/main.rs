use axum::{Router, routing::{get, post}, response::Redirect, http::{header, HeaderValue}};
use minijinja::{Environment, path_loader};
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

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
    pub http: reqwest::Client,
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
    tmpl.add_global("imagina_url", minijinja::Value::from_safe_string(cfg.imagina_url.clone()));

    let http = reqwest::Client::new();
    let state = AppState { tmpl, config: cfg, pool, http };

    let app = Router::new()
        .route("/", get(routes::productos::lista))
        .route("/productos", get(|| async { Redirect::permanent("/") }))
        .route("/robots.txt", get(routes::pages::robots_txt))
        .route("/sitemap.xml", get(routes::pages::sitemap_xml))
        .route("/productos/{nid}", get(routes::productos::detalle))
        .route("/carrito", get(routes::carrito::pagina))
        .route("/pedidos", post(routes::carrito::crear_pedido))
        .route("/resena", get(routes::pages::resena))
        .route("/fotos", get(routes::pages::fotos))
        .route("/imagina", get(routes::pages::imagina))
        .route("/futbol", get(routes::futbol::pagina))
        .route("/terminos", get(routes::monedero::terminos))
        .route("/saldo", get(routes::monedero::saldo_get))
        .route("/saldo", post(routes::monedero::saldo_post))
        .route("/monedero/{cliente_id}", get(routes::monedero::app))
        .route("/recibo/{id}", get(routes::monedero::recibo))
        .route("/recibo/{id}/pdf", get(routes::monedero::recibo_pdf))
        .route("/recibo/{id}/print", get(routes::monedero::recibo_print))
        .route("/cotizacion/{uid}", get(routes::monedero::cotizacion))
        .route("/cotizacion/{uid}/pdf", get(routes::monedero::cotizacion_pdf))
        .route("/cotizacion/{uid}/print", get(routes::monedero::cotizacion_print))
        .route("/r/{code}", get(routes::monedero::redirigir))
        .nest_service(
            "/static",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new("static")),
        )
        .fallback(routes::pages::fallback)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Servidor en http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
