use serde::Serialize;

/// Video de un producto (`UrlContentProductos`). Si es una liga completa de TikTok se puede
/// incrustar su reproductor; si no (liga corta `vm.tiktok.com`, otra página), solo se enlaza.
#[derive(Serialize)]
pub struct Video {
    pub url: String,
    /// Reproductor de TikTok para un `<iframe>`, sin reproducción automática.
    pub reproductor: Option<String>,
}

impl Video {
    pub fn desde_url(url: &str) -> Self {
        let url = url.trim();
        Video { url: url.to_string(), reproductor: tiktok_id(url).map(|id| reproductor_tiktok(&id, false)) }
    }
}

/// El número del video en una liga completa de TikTok: `https://www.tiktok.com/@cuenta/video/7476860515940125970`.
/// Las ligas cortas (`vm.tiktok.com/…`) no lo traen: hay que abrirlas para saberlo.
pub fn tiktok_id(url: &str) -> Option<String> {
    let url = url.trim();
    let resto = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let (host, ruta) = resto.split_once('/')?;
    if host != "tiktok.com" && !host.ends_with(".tiktok.com") {
        return None;
    }
    let id = ruta.split("/video/").nth(1)?.split(['?', '/', '#']).next()?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())).then(|| id.to_string())
}

/// Reproductor oficial de TikTok (developers.tiktok.com/doc/embed-player): sin videos recomendados
/// de otras cuentas, sin descripción ni música. En el kiosko arranca solo y se repite.
pub fn reproductor_tiktok(id: &str, en_kiosko: bool) -> String {
    let base = format!("https://www.tiktok.com/player/v1/{id}?rel=0&description=0&music_info=0");
    if en_kiosko { format!("{base}&autoplay=1&loop=1") } else { base }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numero_de_una_liga_completa() {
        assert_eq!(
            tiktok_id("https://www.tiktok.com/@papeleria.xplaya.com/video/7476860515940125970").as_deref(),
            Some("7476860515940125970")
        );
        assert_eq!(tiktok_id(" https://www.tiktok.com/@x/video/123?is_from_webapp=1 ").as_deref(), Some("123"));
        assert_eq!(tiktok_id("https://tiktok.com/@/video/7480529169839869239").as_deref(), Some("7480529169839869239"));
    }

    #[test]
    fn sin_numero_no_se_incrusta() {
        assert_eq!(tiktok_id("https://vm.tiktok.com/ZMkneoMJq/"), None);
        assert_eq!(tiktok_id("https://papeleria.xplaya.com/Ver/1424"), None);
        assert_eq!(tiktok_id("https://evil.com/www.tiktok.com/@x/video/1"), None);
        assert_eq!(tiktok_id("https://www.tiktok.com/@x/video/abc"), None);
    }

    #[test]
    fn reproductor() {
        assert_eq!(reproductor_tiktok("1", false), "https://www.tiktok.com/player/v1/1?rel=0&description=0&music_info=0");
        assert!(reproductor_tiktok("1", true).ends_with("&autoplay=1&loop=1"));
        assert!(Video::desde_url("https://vm.tiktok.com/x/").reproductor.is_none());
    }
}

#[derive(Serialize)]
pub struct Presentacion {
    pub id: uuid::Uuid,
    pub nombre: String,
    pub factor: String,
    pub precio_venta: String,
}

#[derive(Serialize)]
pub struct ProductoCard {
    pub nid: i32,
    pub nombre: String,
    pub categoria: String,
    pub precio_venta: String,
    pub unidad_medida: String,
    pub foto_url: Option<String>,
    pub presentaciones: Vec<Presentacion>,
}

#[derive(Serialize)]
pub struct Paginacion {
    pub total_items: i64,
    pub pagina_actual: i32,
    pub total_paginas: i32,
    /// Números de página a mostrar. None = puntos suspensivos (…).
    /// Serializa como null en JSON → falsy en Minijinja.
    pub paginas: Vec<Option<i32>>,
}

impl Paginacion {
    pub fn new(total_items: i64, pagina_actual: i32, total_paginas: i32) -> Self {
        let paginas = Self::calcular(pagina_actual, total_paginas);
        Self { total_items, pagina_actual, total_paginas, paginas }
    }

    fn calcular(actual: i32, total: i32) -> Vec<Option<i32>> {
        if total <= 1 {
            return vec![];
        }
        // Con 7 páginas o menos caben todas sin elipsis
        if total <= 7 {
            return (1..=total).map(Some).collect();
        }
        // Ventana de ±1 alrededor de la página actual, acotada a [2, total-1]
        // para no solapar con las páginas extremas que siempre se muestran.
        let win_start = (actual - 1).max(2);
        let win_end = (actual + 1).min(total - 1);

        let mut paginas: Vec<Option<i32>> = vec![Some(1)];
        if win_start > 2 {
            paginas.push(None); // …
        }
        for n in win_start..=win_end {
            paginas.push(Some(n));
        }
        if win_end < total - 1 {
            paginas.push(None); // …
        }
        paginas.push(Some(total));
        paginas
    }
}

#[derive(Serialize)]
pub struct ComponenteKit {
    pub nombre: String,
    pub cantidad: String,
}

#[derive(Serialize)]
pub struct ProductoDetalle {
    pub nid: i32,
    pub id: uuid::Uuid,
    pub nombre: String,
    pub categoria: String,
    pub precio_venta: String,
    pub unidad_medida: String,
    pub stock: String,
    pub fotos: Vec<String>,
    pub videos: Vec<Video>,
    pub presentaciones: Vec<Presentacion>,
    pub componentes: Vec<ComponenteKit>,
    pub marca: Option<String>,
    pub modelo: Option<String>,
    pub descripcion: Option<String>,
    /// GTIN válido (EAN-13/UPC-A/etc.) — solo si CodigoBarrasItem pasa el dígito verificador GS1.
    pub gtin: Option<String>,
    /// Código interno cuando CodigoBarrasItem no es un GTIN válido.
    pub sku: Option<String>,
}

/// Una fila de los feeds de Google Merchant Center (productos e inventario local).
/// Se arma en `db::productos::feed_productos`; las rutas solo la convierten en XML.
pub struct FeedProducto {
    pub nid: i32,
    pub nombre: String,
    pub categoria: String,
    pub precio_venta: String,
    /// Existencia en piezas enteras; `None` si es stock ilimitado o menos de 1 pieza.
    pub cantidad: Option<i64>,
    pub marca: Option<String>,
    pub descripcion: Option<String>,
    pub gtin: Option<String>,
    /// Nombres de archivo en MinIO, en el mismo orden que la galería del detalle.
    pub fotos: Vec<String>,
}
