use axum::{extract::State, http::{header, StatusCode}, response::{Html, IntoResponse, Response}};
use chrono::Utc;
use minijinja::context;

use crate::{db, AppState};

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
