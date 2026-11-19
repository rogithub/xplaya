use std::{sync::Arc, time::Duration};

use axum::{Router, routing::{get, post}, response::Redirect, http::header, extract::Request};
use minijinja::{Environment, path_loader};
use sqlx::PgPool;
use tower::ServiceExt;
use tower_http::services::ServeDir;

mod anuncio;
mod autoservicio;
mod avisos;
mod caja;
mod chat;
mod config;
mod db;
mod estaticos;
mod fotos;
mod google;
mod impresion;
mod ipp;
mod kiosko;
mod middleware;
mod minio;
mod models;
mod passkeys;
mod push;
mod routes;

#[derive(Clone)]
pub struct AppState {
    pub tmpl: Environment<'static>,
    /// Versión de CSS, JS y plantillas (`estaticos.rs`): el kiosko se recarga si cambia.
    pub version_paginas: String,
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
    /// Kiosko: visitas abiertas por el QR, antes de escribir nombre y teléfono.
    pub kiosko_visitas: Arc<passkeys::Pendientes<()>>,
    /// Llaves VAPID de las notificaciones (`push.rs`). `None`: notificaciones apagadas.
    pub vapid: Option<Arc<push::Vapid>>,
    /// Avisos a la papelería por NATS → nats-ntfy → ntfy (`avisos.rs`).
    pub avisos: avisos::Avisos,
    /// La cuenta de la caja en vivo para la app del cliente (`caja.rs`, `routes/cuenta.rs`).
    pub cajas: Arc<caja::Cajas>,
    /// Fotos desde la app (`fotos.rs`): firma el permiso para imagina. `None`: sin fotos.
    pub fotos: Option<Arc<fotos::Firmador>>,
    /// MinIO del autoservicio de impresión (`minio.rs`). `None`: "Imprime tú solito" apagado.
    pub minio: Option<minio::Minio>,
    /// "Entrar con Google" por Keycloak (`google.rs`). `None`: el botón no aparece.
    pub google: Option<google::Google>,
    /// Idas a Google que esperan el regreso, con el `state` como id.
    pub google_idas: Arc<passkeys::Pendientes<google::Ida>>,
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
    // CSS y JS con su huella: {{ estatico("css/app.css") }} → /static/css/app.css?h=… (estaticos.rs)
    let huellas = Arc::new(estaticos::Huellas::calcular("static"));
    {
        let huellas = huellas.clone();
        tmpl.add_function("estatico", move |ruta: &str| {
            minijinja::Value::from_safe_string(huellas.url(ruta))
        });
    }

    // El kiosko se recarga solo cuando cambia (routes/kiosko.rs → /kiosko/qr).
    let version = huellas.version("templates");
    tracing::info!("versión de las páginas: {version}");
    tmpl.add_global("version_paginas", version.clone());

    let http = reqwest::Client::new();
    let webauthn = Arc::new(passkeys::webauthn(&cfg));
    let vapid = push::Vapid::desde_base64(&cfg.vapid_private_key).map(Arc::new);
    match &vapid {
        Some(v) => tracing::info!("notificaciones: llave pública VAPID {}", v.publica),
        None => tracing::info!("notificaciones apagadas (sin VAPID_PRIVATE_KEY)"),
    }
    // La app la usa para suscribirse; sin llave (vacía) no ofrece notificaciones.
    tmpl.add_global("vapid_publica", vapid.as_ref().map(|v| v.publica.clone()).unwrap_or_default());
    let avisos = avisos::Avisos::conectar(&cfg).await;
    let minio = minio::Minio::desde_config(&cfg);
    if minio.is_none() {
        tracing::info!("imprimir: sin MINIO_USUARIO, autoservicio de impresión apagado");
    }
    // La pestaña "Imprimir" de la app solo existe con MinIO.
    tmpl.add_global("imprimir_activo", minio.is_some());
    let fotos = fotos::Firmador::desde_base64(&cfg.fotos_llave_privada).map(Arc::new);
    match &fotos {
        Some(f) => tracing::info!("fotos: llave pública para imagina (XPLAYA_LLAVE_PUBLICA) {}", f.publica),
        None => tracing::info!("fotos apagadas (sin FOTOS_LLAVE_PRIVADA)"),
    }
    tmpl.add_global("fotos_activo", fotos.is_some());
    let google = google::Google::desde_config(&cfg);
    if google.is_none() {
        tracing::info!("google: sin KEYCLOAK_ISSUER o KEYCLOAK_CLIENT_SECRET, entrar con Google apagado");
    }
    tmpl.add_global("google_activo", google.is_some());
    let state = AppState {
        tmpl,
        version_paginas: version,
        config: cfg,
        pool,
        http,
        webauthn,
        altas: Arc::new(passkeys::Pendientes::new(Duration::from_secs(15 * 60))),
        registros: Arc::new(passkeys::Pendientes::new(Duration::from_secs(5 * 60))),
        entradas: Arc::new(passkeys::Pendientes::new(Duration::from_secs(5 * 60))),
        kiosko_visitas: Arc::new(passkeys::Pendientes::new(Duration::from_secs(15 * 60))),
        vapid,
        avisos,
        cajas: Arc::new(caja::Cajas::new()),
        fotos,
        minio,
        google,
        google_idas: Arc::new(passkeys::Pendientes::new(Duration::from_secs(10 * 60))),
    };
    // Tareas que entregan los avisos de la app (NATS + revisión periódica).
    push::vigilar(state.clone());
    fotos::vigilar(state.clone());
    chat::vigilar(state.clone());
    // La cuenta de la caja en vivo (NATS caja.cuenta).
    caja::vigilar(state.clone());
    // Borra los PDFs del autoservicio de impresión a los 7 días.
    routes::imprimir::limpiar(state.clone());
    // Borra los anuncios del kiosko rechazados o vencidos de hace más de 30 días.
    routes::anuncio::limpiar(state.clone());
    // Imprime los trabajos que la caja autorizó (autoservicio.rs).
    autoservicio::vigilar(state.clone());

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
        // "Entrar con huella" en el carrito: misma passkey que la app; la sesión es solo de xplaya.com.
        .route("/passkey/entrar/inicio", post(routes::app::entrar_inicio))
        .route("/passkey/entrar/fin", post(routes::app::entrar_fin))
        .route("/salir", post(routes::app::salir_sitio))
        .route("/resena", get(routes::pages::resena))
        .route("/preguntas-frecuentes", get(routes::pages::faq))
        .route("/devoluciones", get(routes::pages::devoluciones))
        .route("/impresiones", get(routes::pages::impresiones))
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
        .nest_service("/static", static_files(huellas.clone()))
        .fallback(routes::pages::fallback)
        .with_state(state.clone());

    // App de clientes (app.xplaya.com): sus propias rutas, mismos archivos estáticos.
    let app = routes::app::router()
        .nest_service("/static", static_files(huellas.clone()))
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

/// `/static`: el `Cache-Control` lo pone `estaticos::cache_estaticos` según la huella pedida
/// (un año si es la de este pod; `no-store` si es de otra versión). Las imágenes van con `?v=`:
/// si cambias una ya publicada, sube su `?v=`.
fn static_files(huellas: Arc<estaticos::Huellas>) -> Router {
    Router::new()
        .fallback_service(ServeDir::new("static"))
        .layer(axum::middleware::from_fn_with_state(huellas, estaticos::cache_estaticos))
}
