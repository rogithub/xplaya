//! Versión de los CSS y JS de `/static` calculada con su contenido, para que un reinicio no
//! deje a nadie con un archivo viejo guardado un año.
//!
//! El problema: `/static` se guarda un año (`immutable`). Durante un reinicio conviven el pod
//! viejo y el nuevo; un celular podía pedir la página al nuevo (`app.css?v=8`) y el archivo
//! al viejo, y guardaba el CSS viejo bajo el nombre nuevo.
//!
//! La solución:
//! - Al arrancar se calcula la huella (SHA-256, 10 hex) de cada `.css` y `.js` de `static/`.
//! - Las plantillas los piden con `{{ estatico("css/app.css") }}` → `/static/css/app.css?h=…`.
//!   Ya no hay que subir `?v=` a mano en CSS y JS.
//! - `cache_estaticos`: si la huella pedida es la de este pod, se guarda un año; si no (la
//!   pidió una página de otra versión), se entrega con `no-store` y el celular la vuelve a
//!   pedir la próxima vez.
//!
//! Las imágenes siguen con su `?v=` a mano (casi no cambian y algunas las leen redes sociales).

use std::{collections::HashMap, path::Path, sync::Arc};

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};

const UN_ANIO: &str = "public, max-age=31536000, immutable";

/// Ruta dentro de `static/` (`css/app.css`) → huella.
pub struct Huellas(HashMap<String, String>);

impl Huellas {
    /// Recorre `dir` y calcula la huella de cada `.css` y `.js`.
    pub fn calcular(dir: &str) -> Self {
        let mut mapa = HashMap::new();
        recorrer(Path::new(dir), Path::new(dir), &mut mapa);
        tracing::info!("Huellas de {} archivos CSS/JS de {dir}", mapa.len());
        Huellas(mapa)
    }

    /// URL con huella para las plantillas. Si el archivo no existe, sin huella (y se avisa en
    /// el log): la página carga igual.
    pub fn url(&self, ruta: &str) -> String {
        match self.0.get(ruta) {
            Some(h) => format!("/static/{ruta}?h={h}"),
            None => {
                tracing::warn!("estatico(\"{ruta}\"): no existe en static/");
                format!("/static/{ruta}")
            }
        }
    }

    /// `Cache-Control` según la huella pedida: la de este pod → un año; otra → `no-store`;
    /// sin huella (imágenes con `?v=`) → un año, como siempre.
    fn cache_control(&self, ruta: &str, pedida: Option<&str>) -> &'static str {
        match pedida {
            None => UN_ANIO,
            Some(h) if self.0.get(ruta).is_some_and(|actual| actual == h) => UN_ANIO,
            Some(_) => "no-store",
        }
    }
}

fn recorrer(raiz: &Path, dir: &Path, mapa: &mut HashMap<String, String>) {
    let Ok(entradas) = std::fs::read_dir(dir) else { return };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.is_dir() {
            recorrer(raiz, &ruta, mapa);
        } else if matches!(ruta.extension().and_then(|e| e.to_str()), Some("css" | "js"))
            && let Ok(bytes) = std::fs::read(&ruta)
            && let Ok(relativa) = ruta.strip_prefix(raiz)
        {
            let huella: String = Sha256::digest(&bytes).iter().take(5).map(|b| format!("{b:02x}")).collect();
            mapa.insert(relativa.to_string_lossy().replace('\\', "/"), huella);
        }
    }
}

/// Middleware de `/static` (va dentro de `nest_service`, así que la ruta ya no trae `/static`).
pub async fn cache_estaticos(State(huellas): State<Arc<Huellas>>, req: Request, next: Next) -> Response {
    let ruta = req.uri().path().trim_start_matches('/').to_string();
    let pedida = req
        .uri()
        .query()
        .and_then(|q| q.split('&').find_map(|par| par.strip_prefix("h=")))
        .map(str::to_string);
    let mut resp = next.run(req).await;
    // 304 (el navegador ya lo tiene) cuenta como bueno; 404 y errores no se guardan.
    let valor = if resp.status().is_success() || resp.status() == StatusCode::NOT_MODIFIED {
        huellas.cache_control(&ruta, pedida.as_deref())
    } else {
        "no-store"
    };
    resp.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(valor));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn huellas() -> Huellas {
        Huellas(HashMap::from([("css/app.css".to_string(), "abc1234567".to_string())]))
    }

    #[test]
    fn url_lleva_la_huella() {
        assert_eq!(huellas().url("css/app.css"), "/static/css/app.css?h=abc1234567");
        assert_eq!(huellas().url("css/no-existe.css"), "/static/css/no-existe.css");
    }

    #[test]
    fn solo_se_guarda_la_huella_de_este_pod() {
        let h = huellas();
        assert_eq!(h.cache_control("css/app.css", Some("abc1234567")), UN_ANIO);
        assert_eq!(h.cache_control("css/app.css", Some("otra000000")), "no-store");
        assert_eq!(h.cache_control("css/app.css", None), UN_ANIO);
        assert_eq!(h.cache_control("img/logo.svg", Some("lo-que-sea")), "no-store");
    }
}
