//! Feeds para Google Merchant Center (fichas gratuitas + inventario local).
//!
//! Formato: RSS 2.0 con el namespace `g:` de Google. Merchant Center los
//! descarga una vez al día como "fuente programada"; se generan desde la BD
//! en cada request, así que un producto nuevo con foto entra solo.

use axum::{extract::State, http::{header, StatusCode}, response::{IntoResponse, Response}};

use crate::{db, models::producto::FeedProducto, AppState};

// Límites de Merchant Center: title 150, description 5000 caracteres.
const MAX_TITLE: usize = 150;
const MAX_DESCRIPTION: usize = 5000;

/// GET /feeds/google.xml — catálogo de productos.
pub async fn google_productos(State(state): State<AppState>) -> Response {
    let productos = match db::productos::feed_productos(&state.pool).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("feed google DB error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let site = &state.config.site_url;
    let cdn = &state.config.content_base_url;

    let mut xml = abrir_rss(site, "Catálogo de productos");
    for p in &productos {
        xml.push_str(&item_producto(p, site, cdn));
    }
    xml.push_str("</channel>\n</rss>\n");
    respuesta_xml(xml)
}

/// GET /feeds/google-local.xml — inventario de la tienda física.
/// Solo existe si `GOOGLE_STORE_CODE` está configurado.
pub async fn google_inventario_local(State(state): State<AppState>) -> Response {
    let Some(store_code) = state.config.google_store_code.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let productos = match db::productos::feed_productos(&state.pool).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("feed google local DB error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let site = &state.config.site_url;
    let store_code = xml_escape(store_code);

    let mut xml = abrir_rss(site, "Inventario local");
    for p in &productos {
        xml.push_str("<item>\n");
        xml.push_str(&format!("  <g:id>{}</g:id>\n", p.nid));
        xml.push_str(&format!("  <g:store_code>{store_code}</g:store_code>\n"));
        xml.push_str("  <g:availability>in_stock</g:availability>\n");
        if let Some(q) = p.cantidad {
            xml.push_str(&format!("  <g:quantity>{q}</g:quantity>\n"));
        }
        xml.push_str(&format!("  <g:price>{} MXN</g:price>\n", p.precio_venta));
        xml.push_str("</item>\n");
    }
    xml.push_str("</channel>\n</rss>\n");
    respuesta_xml(xml)
}

fn abrir_rss(site: &str, titulo: &str) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<rss version=\"2.0\" xmlns:g=\"http://base.google.com/ns/1.0\">\n<channel>\n");
    xml.push_str(&format!("<title>xplaya.com — {}</title>\n", xml_escape(titulo)));
    xml.push_str(&format!("<link>{}/</link>\n", xml_escape(site)));
    xml.push_str("<description>Papelería xplaya.com, Playa del Carmen</description>\n");
    xml
}

fn item_producto(p: &FeedProducto, site: &str, cdn: &str) -> String {
    let descripcion = match &p.descripcion {
        Some(d) => d.clone(),
        None => format!("{} — {}", p.nombre, p.categoria),
    };

    let mut item = String::from("<item>\n");
    item.push_str(&format!("  <g:id>{}</g:id>\n", p.nid));
    item.push_str(&format!("  <title>{}</title>\n", xml_escape(&truncar(&p.nombre, MAX_TITLE))));
    item.push_str(&format!(
        "  <description>{}</description>\n",
        xml_escape(&truncar(&descripcion, MAX_DESCRIPTION))
    ));
    item.push_str(&format!("  <link>{}</link>\n", xml_escape(&format!("{site}/productos/{}", p.nid))));

    // Merchant Center admite hasta 10 imágenes adicionales.
    for (i, f) in p.fotos.iter().take(11).enumerate() {
        let tag = if i == 0 { "g:image_link" } else { "g:additional_image_link" };
        let url = format!("{cdn}/papeleria-fotos-productos/{f}");
        item.push_str(&format!("  <{tag}>{}</{tag}>\n", xml_escape(&url)));
    }

    item.push_str(&format!("  <g:price>{} MXN</g:price>\n", p.precio_venta));
    item.push_str("  <g:availability>in_stock</g:availability>\n");
    item.push_str("  <g:condition>new</g:condition>\n");
    item.push_str(&format!("  <g:product_type>{}</g:product_type>\n", xml_escape(&p.categoria)));

    if let Some(marca) = &p.marca {
        item.push_str(&format!("  <g:brand>{}</g:brand>\n", xml_escape(marca)));
    }
    // Sin GTIN válido no tenemos identificador del fabricante (no guardamos MPN),
    // así que se declara explícitamente para que Google no marque el producto.
    match &p.gtin {
        Some(gtin) => item.push_str(&format!("  <g:gtin>{gtin}</g:gtin>\n")),
        None => item.push_str("  <g:identifier_exists>no</g:identifier_exists>\n"),
    }

    item.push_str("</item>\n");
    item
}

fn respuesta_xml(xml: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
            // Google lo baja una vez al día; una hora de caché basta para no
            // golpear la BD si algo más lo pide seguido.
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        xml,
    )
        .into_response()
}

/// Escapa los 5 caracteres especiales de XML y quita los caracteres de control
/// que XML 1.0 no permite (a veces llegan pegados desde Excel o del POS) —
/// uno solo de esos invalida el feed completo.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// Corta por caracteres (no por bytes) para no partir una letra con acento.
fn truncar(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
