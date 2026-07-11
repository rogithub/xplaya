use axum::{extract::State, http::{header, StatusCode, Uri}, response::{Html, IntoResponse, Redirect, Response}};
use chrono::Utc;
use minijinja::context;

use crate::{db, AppState};

pub async fn fallback(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path();
    if path.ends_with('/') && path.len() > 1 {
        let trimmed = path.trim_end_matches('/');
        let location = match uri.query() {
            Some(q) => format!("{}?{}", trimmed, q),
            None => trimmed.to_string(),
        };
        return Redirect::permanent(&location).into_response();
    }

    match state.tmpl.get_template("pages/404.html") {
        Ok(tmpl) => match tmpl.render(context!()) {
            Ok(html) => (StatusCode::NOT_FOUND, Html(html)).into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        },
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn robots_txt(State(state): State<AppState>) -> Response {
    let body = format!(
        "User-agent: *\nAllow: /\nDisallow: /monedero/\nDisallow: /recibo/\nDisallow: /cotizacion/\n\nSitemap: {}/sitemap.xml\n",
        state.config.site_url
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

pub async fn sitemap_xml(State(state): State<AppState>) -> Response {
    let productos = match db::productos::sitemap_productos(&state.pool).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("sitemap DB error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let site = &state.config.site_url;
    let cdn = &state.config.content_base_url;
    let today = Utc::now().format("%Y-%m-%d");

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\"\n");
    xml.push_str("        xmlns:image=\"http://www.google.com/schemas/sitemap-image/1.1\">\n");

    // Páginas estáticas
    for (loc, priority, freq) in [
        (format!("{site}/"), "1.0", "daily"),
        (format!("{site}/impresiones"), "0.9", "monthly"),
        (format!("{site}/fotos"), "0.8", "monthly"),
        (format!("{site}/imagina"), "0.8", "monthly"),
        (format!("{site}/futbol"), "0.8", "daily"),
        (format!("{site}/preguntas-frecuentes"), "0.7", "monthly"),
        (format!("{site}/resena"), "0.5", "monthly"),
    ] {
        xml.push_str(&format!(
            "<url><loc>{loc}</loc><lastmod>{today}</lastmod><priority>{priority}</priority><changefreq>{freq}</changefreq></url>\n"
        ));
    }

    // Productos con imágenes
    for (nid, fotos) in &productos {
        xml.push_str("<url>\n");
        xml.push_str(&format!("  <loc>{site}/productos/{nid}</loc>\n"));
        xml.push_str(&format!("  <lastmod>{today}</lastmod>\n"));
        xml.push_str("  <priority>0.8</priority>\n");
        xml.push_str("  <changefreq>weekly</changefreq>\n");
        for f in fotos {
            xml.push_str("  <image:image>\n");
            xml.push_str(&format!("    <image:loc>{cdn}/papeleria-fotos-productos/{f}</image:loc>\n"));
            xml.push_str("  </image:image>\n");
        }
        xml.push_str("</url>\n");
    }

    xml.push_str("</urlset>\n");
    ([(header::CONTENT_TYPE, "application/xml; charset=utf-8")], xml).into_response()
}

pub async fn resena(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let tmpl = state.tmpl.get_template("pages/resena.html").map_err(|e| {
        tracing::error!("Template resena: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(context!()).map_err(|e| {
        tracing::error!("Render resena: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}

pub async fn faq(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let tmpl = state.tmpl.get_template("pages/faq.html").map_err(|e| {
        tracing::error!("Template faq: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(context!()).map_err(|e| {
        tracing::error!("Render faq: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}

pub async fn fotos(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let tmpl = state.tmpl.get_template("pages/fotos.html").map_err(|e| {
        tracing::error!("Template fotos: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(context!()).map_err(|e| {
        tracing::error!("Render fotos: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}

pub async fn impresiones(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let tmpl = state.tmpl.get_template("pages/impresiones.html").map_err(|e| {
        tracing::error!("Template impresiones: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(context!()).map_err(|e| {
        tracing::error!("Render impresiones: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}

pub async fn imagina(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let tmpl = state.tmpl.get_template("pages/imagina.html").map_err(|e| {
        tracing::error!("Template imagina: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(context!(imagina_url => state.config.imagina_url)).map_err(|e| {
        tracing::error!("Render imagina: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}
