use std::{sync::Arc, time::Duration};

use axum::{Router, routing::{get, post}, response::Redirect, http::{header, HeaderValue}, extract::Request};
use minijinja::{Environment, path_loader};
use sqlx::PgPool;
use tower::{ServiceBuilder, ServiceExt};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

mod config;
mod db;
mod kiosko;
mod middleware;
mod models;
mod passkeys;
mod routes;

#[derive(Clone)]
pub struct AppState {
    pub tmpl: Environment<'static>,
    pub config: config::Config,
    pub pool: PgPool,
    pub http: reqwest::Client,
    /// Passkeys de la app de clientes (`passkeys.rs`).
    pub webauthn: Arc<webauthn_rs::Webauthn>,
    /// Altas del recibo que ya pasaron los 4 dígitos (o los datos) y esperan la passkey.
    pub altas: Arc<passkeys::Pendientes<routes::alta::AltaPendiente>>,
    /// Estado de "crear passkey", con el ticket del alta como id.
    pub registros: Arc<passkeys::Pendientes<webauthn_rs::prelude::PasskeyRegistration>>,
    /// Estado de "entrar con passkey".
    pub entradas: Arc<passkeys::Pendientes<webauthn_rs::prelude::DiscoverableAuthentication>>,
    /// Kiosko: visitas abiertas por el QR, esperas de aprobación de la caja y los tokens
    /// de los botones de ntfy (apuntan a la espera).
    pub kiosko_visitas: Arc<passkeys::Pendientes<()>>,
    pub kiosko_esperas: Arc<passkeys::Pendientes<routes::kiosko::Espera>>,
    pub kiosko_aprobaciones: Arc<passkeys::Pendientes<uuid::Uuid>>,
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
    tmpl.add_global("app_url", minijinja::Value::from_safe_string(cfg.app_url.clone()));

    let http = reqwest::Client::new();
    let webauthn = Arc::new(passkeys::webauthn(&cfg));
    let state = AppState {
        tmpl,
        config: cfg,
        pool,
        http,
        webauthn,
        altas: Arc::new(passkeys::Pendientes::new(Duration::from_secs(15 * 60))),
        registros: Arc::new(passkeys::Pendientes::new(Duration::from_secs(5 * 60))),
        entradas: Arc::new(passkeys::Pendientes::new(Duration::from_secs(5 * 60))),
        kiosko_visitas: Arc::new(passkeys::Pendientes::new(Duration::from_secs(15 * 60))),
        kiosko_esperas: Arc::new(passkeys::Pendientes::new(Duration::from_secs(10 * 60))),
        kiosko_aprobaciones: Arc::new(passkeys::Pendientes::new(Duration::from_secs(10 * 60))),
    };

    let site = Router::new()
        .route("/", get(routes::productos::lista))
        .route("/productos", get(|| async { Redirect::permanent("/") }))
        .route("/robots.txt", get(routes::pages::robots_txt))
        .route("/llms.txt", get(routes::pages::llms_txt))
        .route("/sitemap.xml", get(routes::pages::sitemap_xml))
        .route("/feeds/google.xml", get(routes::feeds::google_productos))
        .route("/feeds/google-local.xml", get(routes::feeds::google_inventario_local))
        .route("/productos/{nid}", get(routes::productos::detalle))
        .route("/carrito", get(routes::carrito::pagina))
        .route("/pedidos", post(routes::carrito::crear_pedido))
        .route("/resena", get(routes::pages::resena))
        .route("/preguntas-frecuentes", get(routes::pages::faq))
        .route("/devoluciones", get(routes::pages::devoluciones))
        .route("/impresiones", get(routes::pages::impresiones))
        .route("/fotos", get(routes::pages::fotos))
        .route("/foto-credencial", get(routes::pages::foto_credencial))
        .route("/imagina", get(routes::pages::imagina))
        .route("/logo-oficial", get(routes::pages::logo_oficial))
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
        .nest_service("/static", static_files())
        .fallback(routes::pages::fallback)
        .with_state(state.clone());

    // App de clientes (app.xplaya.com): sus propias rutas, mismos archivos estáticos.
    let app = routes::app::router()
        .nest_service("/static", static_files())
        .with_state(state.clone());

    // Un solo servidor para los dos dominios: según el `Host` de cada petición se
    // la pasa al router de la app o al del sitio. `oneshot` ejecuta el router
    // elegido con esa petición (un Router de Axum también es un Service de tower).
    let app_host = state.config.app_host();
    let root = Router::new().fallback_service(tower::service_fn(move |req: Request| {
        let host = req
            .headers()
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .or_else(|| req.uri().authority().map(|a| a.as_str()))
            .unwrap_or("")
            .to_lowercase();
        let router = if host == app_host { app.clone() } else { site.clone() };
        router.oneshot(req)
    }));

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Servidor en http://{}", addr);
    axum::serve(listener, root).await.unwrap();
}

/// `/static` con caché de un año: si cambias un archivo ya publicado, sube su `?v=`.
fn static_files() -> tower_http::set_header::SetResponseHeader<ServeDir, HeaderValue> {
    ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .service(ServeDir::new("static"))
}
