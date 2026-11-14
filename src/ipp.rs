//! IPP (Internet Printing Protocol, RFC 8010/8011): lo mínimo para mandar un PDF a las Kyocera del
//! autoservicio de impresión y saber cuándo terminó. Sin CUPS ni la PC de la tienda: las impresoras
//! están en la misma red que el cluster y aceptan `application/pdf` directo (consultado el
//! 2026-11-14: rangos de páginas, copias, doble cara, carta, `print-scaling`).
//!
//! Un mensaje IPP es binario: versión, operación, id, grupos de atributos y, en Print-Job, el
//! documento al final. Va por HTTP POST (`application/ipp`). `ipp://` es http y `ipps://` https;
//! la MA5500ifx solo acepta https, con un certificado propio de la impresora.

use std::time::Duration;

/// Operaciones.
const PRINT_JOB: u16 = 0x0002;
const GET_JOB_ATTRIBUTES: u16 = 0x0009;
const GET_PRINTER_ATTRIBUTES: u16 = 0x000B;

/// Etiquetas de grupo y de valor.
const OPERACION: u8 = 0x01;
const TRABAJO: u8 = 0x02;
const FIN: u8 = 0x03;
const INTEGER: u8 = 0x21;
const ENUM: u8 = 0x23;
const RANGO: u8 = 0x33;
const NOMBRE: u8 = 0x42;
const KEYWORD: u8 = 0x44;
const URI: u8 = 0x45;
const CHARSET: u8 = 0x47;
const IDIOMA: u8 = 0x48;
const MIME: u8 = 0x49;

/// Lo que se imprime: todo en blanco y negro, carta y ajustado a la hoja.
#[derive(Debug, Clone)]
pub struct Trabajo<'a> {
    pub nombre: &'a str,
    /// Rangos de páginas; vacío = todas.
    pub paginas: Vec<(u32, u32)>,
    pub copias: u32,
    pub doble_cara: bool,
}

/// Valor de un atributo en una respuesta.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Entero(i32),
    Texto(String),
    Otro,
}

#[derive(Debug, Default)]
pub struct Respuesta {
    pub status: u16,
    /// Nombre → valores (los de varios valores traen todos).
    pub atributos: Vec<(String, Vec<Valor>)>,
}

impl Respuesta {
    /// 0x0000-0x00FF: bien (0x0001 = "bien, pero ignoró o cambió atributos").
    pub fn ok(&self) -> bool {
        self.status < 0x0100
    }

    pub fn entero(&self, nombre: &str) -> Option<i32> {
        self.valores(nombre).iter().find_map(|v| match v {
            Valor::Entero(n) => Some(*n),
            _ => None,
        })
    }

    pub fn textos(&self, nombre: &str) -> Vec<String> {
        self.valores(nombre)
            .iter()
            .filter_map(|v| match v {
                Valor::Texto(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    fn valores(&self, nombre: &str) -> &[Valor] {
        self.atributos.iter().find(|(n, _)| n == nombre).map_or(&[], |(_, v)| v)
    }
}

// ── Armar mensajes ──────────────────────────────────────────────────────────

struct Mensaje(Vec<u8>);

impl Mensaje {
    fn nuevo(operacion: u16, id: u32) -> Self {
        let mut b = vec![1, 1]; // IPP/1.1: la entienden todas
        b.extend(operacion.to_be_bytes());
        b.extend(id.to_be_bytes());
        Mensaje(b)
    }

    fn grupo(&mut self, etiqueta: u8) -> &mut Self {
        self.0.push(etiqueta);
        self
    }

    /// Un valor; con `nombre` vacío es otro valor del atributo anterior.
    fn valor(&mut self, etiqueta: u8, nombre: &str, valor: &[u8]) -> &mut Self {
        self.0.push(etiqueta);
        self.0.extend((nombre.len() as u16).to_be_bytes());
        self.0.extend(nombre.as_bytes());
        self.0.extend((valor.len() as u16).to_be_bytes());
        self.0.extend(valor);
        self
    }

    fn texto(&mut self, etiqueta: u8, nombre: &str, valor: &str) -> &mut Self {
        self.valor(etiqueta, nombre, valor.as_bytes())
    }

    fn entero(&mut self, etiqueta: u8, nombre: &str, valor: i32) -> &mut Self {
        self.valor(etiqueta, nombre, &valor.to_be_bytes())
    }

    /// Los atributos que van en toda petición.
    fn encabezado(&mut self, uri: &str) -> &mut Self {
        self.grupo(OPERACION)
            .texto(CHARSET, "attributes-charset", "utf-8")
            .texto(IDIOMA, "attributes-natural-language", "es-mx")
            .texto(URI, "printer-uri", uri)
    }

    fn pedir(&mut self, atributos: &[&str]) -> &mut Self {
        for (i, a) in atributos.iter().enumerate() {
            self.texto(KEYWORD, if i == 0 { "requested-attributes" } else { "" }, a);
        }
        self
    }
}

/// Print-Job con el PDF al final.
fn print_job(uri: &str, t: &Trabajo, pdf: &[u8]) -> Vec<u8> {
    let mut m = Mensaje::nuevo(PRINT_JOB, 1);
    m.encabezado(uri)
        .texto(NOMBRE, "requesting-user-name", "xplaya")
        .texto(NOMBRE, "job-name", &recortar(t.nombre, 200))
        .texto(MIME, "document-format", "application/pdf")
        .grupo(TRABAJO)
        .entero(INTEGER, "copies", t.copias as i32)
        .texto(KEYWORD, "sides", if t.doble_cara { "two-sided-long-edge" } else { "one-sided" })
        .texto(KEYWORD, "media", "na_letter_8.5x11in")
        .texto(KEYWORD, "print-color-mode", "monochrome")
        .texto(KEYWORD, "print-scaling", "fit")
        // Copias completas una tras otra (1-2-3, 1-2-3), no 1-1, 2-2, 3-3.
        .texto(KEYWORD, "multiple-document-handling", "separate-documents-collated-copies");
    for (i, (a, b)) in t.paginas.iter().enumerate() {
        let mut rango = (*a as i32).to_be_bytes().to_vec();
        rango.extend((*b as i32).to_be_bytes());
        m.valor(RANGO, if i == 0 { "page-ranges" } else { "" }, &rango);
    }
    m.grupo(FIN);
    let mut b = m.0;
    b.extend(pdf);
    b
}

fn get_job(uri: &str, job_id: i32) -> Vec<u8> {
    let mut m = Mensaje::nuevo(GET_JOB_ATTRIBUTES, 2);
    m.encabezado(uri)
        .entero(INTEGER, "job-id", job_id)
        .pedir(&["job-state", "job-state-reasons"])
        .grupo(FIN);
    m.0
}

fn get_printer(uri: &str) -> Vec<u8> {
    let mut m = Mensaje::nuevo(GET_PRINTER_ATTRIBUTES, 3);
    m.encabezado(uri).pedir(&["printer-state", "printer-state-reasons"]).grupo(FIN);
    m.0
}

fn recortar(texto: &str, max: usize) -> String {
    texto.chars().take(max).collect()
}

// ── Leer respuestas ─────────────────────────────────────────────────────────

pub fn leer(bytes: &[u8]) -> Result<Respuesta, String> {
    let corta = || "respuesta IPP incompleta".to_string();
    if bytes.len() < 8 {
        return Err(corta());
    }
    let mut r = Respuesta { status: u16::from_be_bytes([bytes[2], bytes[3]]), atributos: Vec::new() };
    let mut i = 8;
    while i < bytes.len() {
        let etiqueta = bytes[i];
        i += 1;
        if etiqueta == FIN {
            break;
        }
        if etiqueta < 0x10 {
            continue; // otro grupo
        }
        let largo = |i: usize| -> Result<usize, String> {
            bytes.get(i..i + 2).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize).ok_or_else(corta)
        };
        let ln = largo(i)?;
        let nombre = bytes.get(i + 2..i + 2 + ln).ok_or_else(corta)?;
        i += 2 + ln;
        let lv = largo(i)?;
        let valor = bytes.get(i + 2..i + 2 + lv).ok_or_else(corta)?;
        i += 2 + lv;
        let v = match etiqueta {
            INTEGER | ENUM if lv == 4 => Valor::Entero(i32::from_be_bytes([valor[0], valor[1], valor[2], valor[3]])),
            0x40..=0x4F => Valor::Texto(String::from_utf8_lossy(valor).into_owned()),
            _ => Valor::Otro,
        };
        if nombre.is_empty() {
            if let Some((_, vs)) = r.atributos.last_mut() {
                vs.push(v);
            }
        } else {
            r.atributos.push((String::from_utf8_lossy(nombre).into_owned(), vec![v]));
        }
    }
    Ok(r)
}

// ── Mandar ──────────────────────────────────────────────────────────────────

/// Cliente HTTP para las impresoras: acepta su certificado propio (solo se usa en la red de la
/// tienda, con las direcciones de `Settings.AUTOSERVICIO_IMPRESORAS`).
pub fn cliente() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .expect("cliente HTTP de impresoras")
}

/// `ipp://host:631/ipp/print` → `http://host:631/ipp/print`; `ipps://` → `https://`.
pub fn url_http(uri: &str) -> Result<String, String> {
    if let Some(resto) = uri.strip_prefix("ipps://") {
        Ok(format!("https://{resto}"))
    } else if let Some(resto) = uri.strip_prefix("ipp://") {
        Ok(format!("http://{resto}"))
    } else {
        Err(format!("dirección de impresora inválida: {uri}"))
    }
}

async fn enviar(http: &reqwest::Client, uri: &str, cuerpo: Vec<u8>) -> Result<Respuesta, String> {
    let resp = http
        .post(url_http(uri)?)
        .header("content-type", "application/ipp")
        .body(cuerpo)
        .send()
        .await
        .map_err(|e| if e.is_connect() || e.is_timeout() { "La impresora no responde.".to_string() } else { format!("IPP: {e}") })?;
    if !resp.status().is_success() {
        return Err(format!("La impresora respondió HTTP {}.", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("IPP: {e}"))?;
    leer(&bytes)
}

/// Manda el PDF. Devuelve el `job-id` de la impresora.
pub async fn imprimir(http: &reqwest::Client, uri: &str, t: &Trabajo<'_>, pdf: &[u8]) -> Result<i32, String> {
    let r = enviar(http, uri, print_job(uri, t, pdf)).await?;
    if !r.ok() {
        return Err(format!("La impresora no aceptó el trabajo (IPP 0x{:04x}).", r.status));
    }
    r.entero("job-id").ok_or_else(|| "La impresora no devolvió el número del trabajo.".into())
}

/// Estado del trabajo en la impresora: 3 pendiente, 4 detenido, 5 imprimiendo, 6 detenido por la
/// impresora, 7 cancelado, 8 abortado, 9 terminado. Y sus razones (`job-state-reasons`).
pub async fn estado_trabajo(http: &reqwest::Client, uri: &str, job_id: i32) -> Result<(i32, Vec<String>), String> {
    let r = enviar(http, uri, get_job(uri, job_id)).await?;
    if !r.ok() {
        return Err(format!("La impresora no dio el estado del trabajo (IPP 0x{:04x}).", r.status));
    }
    Ok((r.entero("job-state").unwrap_or(0), r.textos("job-state-reasons")))
}

/// Razones de la impresora (`printer-state-reasons`: `media-empty`, `media-jam`…), para explicar
/// un error. Vacío si no responde.
pub async fn razones_impresora(http: &reqwest::Client, uri: &str) -> Vec<String> {
    match enviar(http, uri, get_printer(uri)).await {
        Ok(r) => r.textos("printer-state-reasons").into_iter().filter(|x| x != "none").collect(),
        Err(_) => Vec::new(),
    }
}

/// Lo que entiende la cajera de las razones de la impresora o del trabajo.
pub fn explicar(razones: &[String]) -> Option<&'static str> {
    let tiene = |palabra: &str| razones.iter().any(|r| r.starts_with(palabra));
    if tiene("media-empty") || tiene("media-needed") {
        Some("Sin papel.")
    } else if tiene("media-jam") {
        Some("Papel atorado.")
    } else if tiene("toner-empty") || tiene("marker-supply-empty") {
        Some("Sin tóner.")
    } else if tiene("door-open") || tiene("cover-open") {
        Some("Tapa abierta.")
    } else if tiene("document-format-error") || tiene("document-unprintable") {
        Some("La impresora no pudo leer el PDF.")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_job_lleva_las_opciones_y_el_pdf_al_final() {
        let t = Trabajo { nombre: "tarea.pdf", paginas: vec![(1, 3), (5, 5)], copias: 2, doble_cara: true };
        let b = print_job("ipp://192.168.0.6:631/ipp/print", &t, b"%PDF-1.5 x");
        assert_eq!(&b[..4], &[1, 1, 0x00, 0x02]); // IPP/1.1, Print-Job
        assert!(b.ends_with(b"\x03%PDF-1.5 x")); // fin de atributos y el documento
        let r = leer(&b).unwrap();
        assert_eq!(r.textos("printer-uri"), ["ipp://192.168.0.6:631/ipp/print"]);
        assert_eq!(r.textos("sides"), ["two-sided-long-edge"]);
        assert_eq!(r.textos("media"), ["na_letter_8.5x11in"]);
        assert_eq!(r.entero("copies"), Some(2));
        // Los dos rangos en el mismo atributo.
        assert_eq!(r.atributos.iter().find(|(n, _)| n == "page-ranges").map(|(_, v)| v.len()), Some(2));
    }

    #[test]
    fn todas_las_paginas_no_manda_rangos() {
        let t = Trabajo { nombre: "x", paginas: vec![], copias: 1, doble_cara: false };
        let r = leer(&print_job("ipp://h/ipp/print", &t, b"")).unwrap();
        assert!(r.atributos.iter().all(|(n, _)| n != "page-ranges"));
        assert_eq!(r.textos("sides"), ["one-sided"]);
    }

    #[test]
    fn lee_status_enteros_y_varios_valores() {
        // status 0x0000, job-id 42, job-state 9, job-state-reasons [a, b]
        let mut m = Mensaje::nuevo(0x0000, 1);
        m.grupo(TRABAJO)
            .entero(INTEGER, "job-id", 42)
            .entero(ENUM, "job-state", 9)
            .texto(KEYWORD, "job-state-reasons", "job-completed-successfully")
            .texto(KEYWORD, "", "otra")
            .grupo(FIN);
        let r = leer(&m.0).unwrap();
        assert!(r.ok());
        assert_eq!(r.entero("job-id"), Some(42));
        assert_eq!(r.entero("job-state"), Some(9));
        assert_eq!(r.textos("job-state-reasons"), ["job-completed-successfully", "otra"]);
        assert!(leer(&[1, 1]).is_err());
    }

    #[test]
    fn direcciones() {
        assert_eq!(url_http("ipps://192.168.0.9:443/ipp/print").unwrap(), "https://192.168.0.9:443/ipp/print");
        assert_eq!(url_http("ipp://192.168.0.6:631/ipp/print").unwrap(), "http://192.168.0.6:631/ipp/print");
        assert!(url_http("http://x").is_err());
    }

    /// Contra las impresoras de verdad, solo lectura (no imprime): `cargo test ipp -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn consulta_las_kyocera() {
        let http = cliente();
        for uri in ["ipp://192.168.0.6:631/ipp/print", "ipps://192.168.0.9:443/ipp/print"] {
            let r = enviar(&http, uri, get_printer(uri)).await.unwrap();
            println!("{uri}: ok={} estado={:?} razones={:?}", r.ok(), r.entero("printer-state"), r.textos("printer-state-reasons"));
            assert!(r.ok());
        }
    }

    #[test]
    fn explica_las_razones() {
        assert_eq!(explicar(&["media-empty-error".into()]), Some("Sin papel."));
        assert_eq!(explicar(&["media-jam-warning".into()]), Some("Papel atorado."));
        assert_eq!(explicar(&["none".into()]), None);
    }
}
