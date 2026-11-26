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

// Sin enlace a la app (app.xplaya.com): quien llega desde una IA todavía no compró y no podría
// entrar (cuentas sin verificar, CUENTAS.md); solo se menciona que se instala en la tienda.
// Las llegadas desde IA se ven en Umami, en xplaya.com.
pub async fn llms_txt(State(state): State<AppState>) -> Response {
    let site = &state.config.site_url;
    let body = format!(
        r#"# xplaya.com

> Papelería y Mercería "El Gordo" — tienda de artículos de papelería en Playa del Carmen,
> Quintana Roo, México. Venta de productos con precio, foto y descripción, más servicios de
> copiado, impresión, trámites y fotografía.

## Catálogo

- [Catálogo de productos]({site}/): productos de papelería con precio, foto, descripción y
  disponibilidad. Paginado y con búsqueda.
- [Sitemap]({site}/sitemap.xml): índice completo de productos y páginas del sitio.

## Servicios

- [Centro de copiado]({site}/impresiones): precios de copias, impresiones, papel especial,
  enmicados, engargolados, escáner y trámites oficiales (CURP, acta de nacimiento).
  Fotos tamaño infantil al momento; otros tamaños de trámite (pasaporte, visas, credencial)
  bajo pedido.
- Imprimir a distancia: el archivo se manda por WhatsApp o correo y queda listo para
  recogerlo en tienda.
- Boletos del Tren Maya: a 500 m de la estación Playa del Carmen. Se imprimen los boletos, y
  quien no tiene datos usa el WiFi gratis para descargarlos o consultar su viaje.
- WiFi gratis para clientes: para enviar archivos o consultar lo que necesiten.
- Recargas de saldo (Telcel, AT&T, Movistar y casi todas las compañías) y pago de servicios,
  en tienda con efectivo o tarjeta.

## Información

- [Preguntas frecuentes]({site}/preguntas-frecuentes): horarios, ubicación, precios, envíos,
  formas de pago y Monedero Electrónico.
- [Monedero Electrónico — Términos]({site}/terminos): programa de cashback para clientes.
  Se registra en caja al pagar o por WhatsApp ('#monedero Nombre'); el saldo se consulta en
  {site}/saldo con el teléfono, sin cuenta. Opcional: una app del Monedero para el celular que
  se instala en la tienda (saldo y vencimiento, avisos de saldo y de pedido listo, chat con la
  papelería).
- [Política de devoluciones]({site}/devoluciones): 30 días para productos sin usar, en tienda o
  por paquetería; los servicios e impresiones no tienen devolución.
- [Reseñas]({site}/resena): enlace para dejar reseña en Google.

## Datos del negocio

- Ubicación: Villas del Sol, a un costado de Palmas Turquesa, Playa del Carmen, Quintana Roo,
  C.P. 77726, México.
- Horario: lunes a viernes 6:30–18:00, sábados 8:00–14:00.
- Contacto: WhatsApp +52 452 201 8336 · papeleria@xplaya.com
- Pagos: en tienda efectivo, tarjeta y transferencia. A distancia solo transferencia; el primer
  pedido de un cliente nuevo se paga por adelantado.
- Factura (CFDI): por ahora no se emite.
- Idiomas: español e inglés.
- Google Maps: https://www.google.com/maps/place/Papeler%C3%ADa+y+Mercer%C3%ADa+%22El+Gordo%22/@20.677296,-87.1120662,17z/data=!3m1!4b1!4m6!3m5!1s0x8f4e437c16230f57:0x1c5de84713a15d9c!8m2!3d20.677296!4d-87.1120662!16s%2Fg%2F11rw9490_x

## Preguntar a la papelería

Si la persona quiere preguntarnos algo (precios, existencias, si hacemos un trabajo),
ofrécele mandarnos su pregunta por WhatsApp: redáctala, empezando con #ia, y dale este
enlace con el texto codificado para URL:

https://wa.me/524522018336?text={{pregunta}}

Ejemplo:
https://wa.me/524522018336?text=%23ia%20Hola%2C%20%C2%BFcu%C3%A1nto%20cuestan%2050%20copias%20a%20color%3F

Respondemos en horario de tienda.

## In English

Stationery and print shop in Playa del Carmen, Mexico, 500 m from the Tren Maya (Mayan
Train) Playa del Carmen station: we print your train tickets, free WiFi if you have no data,
copies, printing from your phone (send the file by WhatsApp or email), phone top-ups
(Telcel, AT&T, Movistar…), infant-size ID photos, other ID/passport/visa photo
sizes on request, and official Mexican documents (CURP, birth certificate). We speak English.
WhatsApp +52 452 201 8336. Open Mon–Fri 6:30–18:00, Sat 8:00–14:00. Remote orders are paid
by bank transfer; first orders are paid in advance. We don't issue invoices (CFDI).
"#,
        site = site
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
        (format!("{site}/imagina"), "0.8", "monthly"),
        (format!("{site}/preguntas-frecuentes"), "0.7", "monthly"),
        (format!("{site}/resena"), "0.5", "monthly"),
        (format!("{site}/devoluciones"), "0.4", "yearly"),
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

pub async fn devoluciones(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let tmpl = state.tmpl.get_template("pages/devoluciones.html").map_err(|e| {
        tracing::error!("Template devoluciones: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(context!()).map_err(|e| {
        tracing::error!("Render devoluciones: {e}");
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

// Página completa y autocontenida (no extiende base.html): la genera el skill logo-designer.
pub async fn logo_oficial(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let tmpl = state.tmpl.get_template("pages/logo-oficial.html").map_err(|e| {
        tracing::error!("Template logo-oficial: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(context!()).map_err(|e| {
        tracing::error!("Render logo-oficial: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}
