//! Qué es cada archivo que manda el cliente por el chat, cuánto puede pesar y dónde va en MinIO.
//! Puro, con pruebas: la ruta (`routes/chat.rs`) lee los primeros bytes, pregunta aquí y ejecuta.
//!
//! El tipo sale de los **primeros bytes** del archivo, no del mime que manda el celular. La
//! extensión del nombre solo desempata lo que los bytes no dicen: un `.webm` puede ser video o una
//! nota de voz, y un ZIP puede ser un Word.
//!
//! Lo que no se reconoce también se manda (un `.cdr`, un `.zip` con fotos): queda como
//! [`Clase::Archivo`], que solo se puede bajar. Por eso se sirven siempre con
//! `Content-Disposition` y `nosniff`.

use uuid::Uuid;

use super::reglas::Ajustes;

/// Cuántos bytes del inicio hacen falta para `clasificar`.
pub const BYTES_FIRMA: usize = 16;
/// Cloudflare corta peticiones de más de 100 MB: el video no pasa de esto aunque el ajuste diga más.
pub const MB_VIDEO_TOPE: u64 = 95;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    /// Con miniatura (JPEG, PNG, WebP). Un GIF va aquí pero sin miniatura.
    Imagen,
    Video,
    /// Notas de voz y audios.
    Audio,
    /// Se cuentan sus páginas.
    Pdf,
    /// Word, Excel, PowerPoint (y los de LibreOffice): se convierten a PDF con Gotenberg.
    Office,
    /// Cualquier otra cosa: solo se baja.
    Archivo,
}

/// El resultado de `clasificar`: la extensión (segura para la llave de MinIO) y el mime con que se
/// guarda y se sirve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tipo {
    pub clase: Clase,
    pub ext: String,
    pub mime: &'static str,
}

fn tipo(clase: Clase, ext: &str, mime: &'static str) -> Tipo {
    Tipo { clase, ext: ext.to_string(), mime }
}

/// La extensión del nombre, en minúsculas; vacía si no hay o si no es una extensión razonable
/// (solo letras y números, hasta 8). Así se puede poner en la llave sin codificar nada.
fn extension(nombre: &str) -> String {
    let Some((_, ext)) = nombre.rsplit_once('.') else {
        return String::new();
    };
    let ext = ext.to_ascii_lowercase();
    if !ext.is_empty() && ext.len() <= 8 && ext.bytes().all(|b| b.is_ascii_alphanumeric()) {
        ext
    } else {
        String::new()
    }
}

/// Qué es el archivo, por sus primeros bytes (`BYTES_FIRMA`) y, si hace falta, su nombre.
pub fn clasificar(nombre: &str, inicio: &[u8]) -> Tipo {
    let ext = extension(nombre);
    let en = |de: usize, firma: &[u8]| inicio.get(de..de + firma.len()) == Some(firma);

    if en(0, b"%PDF") {
        return tipo(Clase::Pdf, "pdf", "application/pdf");
    }
    if en(0, &[0xFF, 0xD8, 0xFF]) {
        return tipo(Clase::Imagen, "jpg", "image/jpeg");
    }
    if en(0, &[0x89, b'P', b'N', b'G']) {
        return tipo(Clase::Imagen, "png", "image/png");
    }
    if en(0, b"RIFF") && en(8, b"WEBP") {
        return tipo(Clase::Imagen, "webp", "image/webp");
    }
    if en(0, b"GIF8") {
        return tipo(Clase::Imagen, "gif", "image/gif");
    }
    if en(0, b"RIFF") && en(8, b"WAVE") {
        return tipo(Clase::Audio, "wav", "audio/wav");
    }
    if en(0, b"OggS") {
        return tipo(Clase::Audio, if ext == "opus" { "opus" } else { "ogg" }, "audio/ogg");
    }
    if en(0, b"ID3") || en(0, &[0xFF, 0xFB]) || en(0, &[0xFF, 0xF3]) || en(0, &[0xFF, 0xF2]) {
        return tipo(Clase::Audio, "mp3", "audio/mpeg");
    }
    // WebM / Matroska: la nota de voz de Chrome y los videos de Android.
    if en(0, &[0x1A, 0x45, 0xDF, 0xA3]) {
        return match ext.as_str() {
            "weba" | "opus" => tipo(Clase::Audio, "weba", "audio/webm"),
            "mkv" => tipo(Clase::Video, "mkv", "video/x-matroska"),
            _ => tipo(Clase::Video, "webm", "video/webm"),
        };
    }
    // ISO (MP4, MOV, M4A, HEIC): la marca va después de "ftyp".
    if en(4, b"ftyp") {
        let marca = inicio.get(8..12).unwrap_or_default();
        return match marca {
            b"heic" | b"heix" | b"mif1" | b"msf1" | b"hevc" => tipo(Clase::Archivo, "heic", "image/heic"),
            b"qt  " => tipo(Clase::Video, "mov", "video/quicktime"),
            b"M4A " => tipo(Clase::Audio, "m4a", "audio/mp4"),
            _ if matches!(ext.as_str(), "m4a" | "aac") => tipo(Clase::Audio, "m4a", "audio/mp4"),
            _ => tipo(Clase::Video, "mp4", "video/mp4"),
        };
    }
    // Word, Excel y PowerPoint nuevos (y los de LibreOffice) son un ZIP: decide la extensión.
    if en(0, b"PK\x03\x04") {
        return match ext.as_str() {
            "docx" => tipo(Clase::Office, "docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            "xlsx" => tipo(Clase::Office, "xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
            "pptx" => tipo(Clase::Office, "pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
            "odt" => tipo(Clase::Office, "odt", "application/vnd.oasis.opendocument.text"),
            "ods" => tipo(Clase::Office, "ods", "application/vnd.oasis.opendocument.spreadsheet"),
            "odp" => tipo(Clase::Office, "odp", "application/vnd.oasis.opendocument.presentation"),
            _ => tipo(Clase::Archivo, "zip", "application/zip"),
        };
    }
    // Word, Excel y PowerPoint viejos (OLE).
    if en(0, &[0xD0, 0xCF, 0x11, 0xE0]) {
        return match ext.as_str() {
            "doc" => tipo(Clase::Office, "doc", "application/msword"),
            "xls" => tipo(Clase::Office, "xls", "application/vnd.ms-excel"),
            "ppt" => tipo(Clase::Office, "ppt", "application/vnd.ms-powerpoint"),
            _ => tipo(Clase::Archivo, if ext.is_empty() { "bin" } else { &ext }, "application/octet-stream"),
        };
    }
    if en(0, b"{\\rtf") {
        return tipo(Clase::Office, "rtf", "application/rtf");
    }
    tipo(Clase::Archivo, if ext.is_empty() { "bin" } else { &ext }, "application/octet-stream")
}

/// Cuántos bytes puede pesar un archivo de esa clase: `CHAT_MB_VIDEO` (con tope de
/// `MB_VIDEO_TOPE`) los videos y `CHAT_MB_DOCUMENTO` todo lo demás.
pub fn limite(clase: Clase, ajustes: &Ajustes) -> u64 {
    let mb = match clase {
        Clase::Video => ajustes.mb_video.min(MB_VIDEO_TOPE),
        _ => ajustes.mb_documento,
    };
    mb * 1024 * 1024
}

/// El límite más grande, para cortar la subida antes de saber qué es.
pub fn limite_mayor(ajustes: &Ajustes) -> u64 {
    limite(Clase::Video, ajustes).max(limite(Clase::Archivo, ajustes))
}

/// Dónde va cada archivo en MinIO. Todo en la carpeta del cliente (así lo pide
/// `reglas::mensaje`) y del mensaje. El nombre que puso el cliente no va en la llave: la firma de
/// MinIO no codifica caracteres; ese nombre va en `Adjunto.nombre`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Llaves {
    pub original: String,
    /// El PDF convertido (Office).
    pub pdf: String,
    /// La miniatura (imágenes).
    pub miniatura: String,
}

pub fn llaves(cliente: Uuid, mensaje: Uuid, archivo: Uuid, ext: &str) -> Llaves {
    let base = format!("{cliente}/{mensaje}/{archivo}");
    Llaves { original: format!("{base}.{ext}"), pdf: format!("{base}-pdf.pdf"), miniatura: format!("{base}-mini.jpg") }
}

/// El nombre que se muestra en el chat y con el que se baja: sin rutas ni caracteres de
/// control, hasta 120. Sin nombre, `archivo.{ext}`.
pub fn nombre_visible(original: &str, ext: &str) -> String {
    let base = original.rsplit(['/', '\\']).next().unwrap_or("");
    let limpio: String = base.chars().filter(|c| !c.is_control()).take(120).collect();
    let limpio = limpio.trim();
    if limpio.is_empty() { format!("archivo.{ext}") } else { limpio.to_string() }
}

/// Lado mayor de la miniatura: se ve bien en la cuadrícula y en un globo, y pesa ~30 KB.
pub const LADO_MINIATURA: u32 = 480;

/// La miniatura en JPEG, derecha según el EXIF. `None` si la imagen no se puede abrir (el
/// mensaje se manda igual, sin miniatura).
pub fn miniatura(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = crate::anuncio::abrir_imagen(bytes)?;
    let chica = img.thumbnail(LADO_MINIATURA, LADO_MINIATURA).to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 75).encode_image(&chica).ok()?;
    Some(jpeg)
}

// ── Servir y bajar ──────────────────────────────────────────────────────────

/// Lo que el navegador puede mostrar sin riesgo: lo que reconoce `clasificar`. Todo lo demás (o
/// un mime raro que publique otro programa en el stream) se baja como `application/octet-stream`.
pub fn en_linea(mime: &str) -> bool {
    matches!(
        mime,
        "image/jpeg" | "image/png" | "image/webp" | "image/gif"
            | "video/mp4" | "video/webm" | "video/quicktime" | "video/x-matroska"
            | "audio/webm" | "audio/mp4" | "audio/ogg" | "audio/mpeg" | "audio/wav"
            | "application/pdf"
    )
}

/// `Content-Disposition` con el nombre del cliente: `filename` en ASCII para navegadores viejos
/// y `filename*` en UTF-8 (RFC 6266) con acentos y eñes.
pub fn disposicion(en_linea: bool, nombre: &str) -> String {
    let modo = if en_linea { "inline" } else { "attachment" };
    let ascii: String = nombre.chars().map(|c| if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' { c } else { '_' }).collect();
    let utf8: String = nombre
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("{modo}; filename=\"{ascii}\"; filename*=UTF-8''{utf8}")
}

/// Carpeta del zip según el tipo.
fn carpeta(mime: &str) -> &'static str {
    match mime.split('/').next() {
        Some("image") => "fotos",
        Some("video") => "videos",
        Some("audio") => "audios",
        _ => "documentos",
    }
}

/// Qué va en el zip de "Bajar todo": `(llave en MinIO, ruta en el zip)`.
///
/// - Carpetas por tipo (`fotos/`, `videos/`, `audios/`, `documentos/`); si todo es de un tipo,
///   sin carpetas.
/// - Un Word va con su original y el PDF convertido (`tesis.docx` y `tesis.pdf`).
/// - Nombres repetidos llevan `(2)`, `(3)`…; los caracteres que Windows no acepta, `_`.
pub fn rutas_zip(adjuntos: &[super::contrato::Adjunto]) -> Vec<(String, String)> {
    let mut archivos: Vec<(&str, String, &'static str)> = Vec::new();
    for a in adjuntos {
        let ext = a.llave.rsplit_once('.').map_or("", |(_, e)| e);
        let mut nombre = limpiar_para_zip(&a.nombre);
        if !ext.is_empty() && !nombre.to_lowercase().ends_with(&format!(".{ext}")) {
            nombre = format!("{nombre}.{ext}");
        }
        let que = carpeta(&a.mime);
        let pdf = a.pdf.as_deref().map(|pdf| {
            let base = nombre.rsplit_once('.').map_or(nombre.as_str(), |(b, _)| b);
            (pdf, format!("{base}.pdf"), que)
        });
        archivos.push((&a.llave, nombre, que));
        archivos.extend(pdf);
    }
    let una_carpeta = archivos.iter().all(|(_, _, c)| *c == archivos[0].2);
    let mut usadas = std::collections::HashSet::new();
    archivos
        .into_iter()
        .map(|(llave, nombre, que)| {
            let ruta = |n: &str| if una_carpeta { n.to_string() } else { format!("{que}/{n}") };
            let (base, ext) = nombre.rsplit_once('.').map_or((nombre.as_str(), String::new()), |(b, e)| (b, format!(".{e}")));
            let mut final_ = ruta(&nombre);
            let mut n = 2;
            while !usadas.insert(final_.to_lowercase()) {
                final_ = ruta(&format!("{base} ({n}){ext}"));
                n += 1;
            }
            (llave.to_string(), final_)
        })
        .collect()
}

fn limpiar_para_zip(nombre: &str) -> String {
    let limpio: String = nombre.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect();
    let limpio = limpio.trim().trim_start_matches('.').to_string();
    if limpio.is_empty() { "archivo".to_string() } else { limpio }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clase(nombre: &str, inicio: &[u8]) -> (Clase, String, &'static str) {
        let t = clasificar(nombre, inicio);
        (t.clase, t.ext, t.mime)
    }

    fn ftyp(marca: &[u8; 4]) -> Vec<u8> {
        [&[0, 0, 0, 0x20][..], b"ftyp", marca, &[0, 0, 0, 0]].concat()
    }

    #[test]
    fn por_los_bytes_no_por_el_nombre() {
        assert_eq!(clase("foto.pdf", &[0xFF, 0xD8, 0xFF, 0xE0]).0, Clase::Imagen);
        assert_eq!(clase("tesis.jpg", b"%PDF-1.7"), (Clase::Pdf, "pdf".into(), "application/pdf"));
        assert_eq!(clase("x", &[0x89, b'P', b'N', b'G', 0x0D]).1, "png");
        assert_eq!(clase("x", b"RIFF\0\0\0\0WEBPVP8 ").1, "webp");
        assert_eq!(clase("x", b"GIF89a").0, Clase::Imagen);
    }

    #[test]
    fn audios() {
        assert_eq!(clase("x", b"OggS\0").1, "ogg");
        assert_eq!(clase("PTT-2026.opus", b"OggS\0").1, "opus");
        assert_eq!(clase("x", b"ID3\x04").1, "mp3");
        assert_eq!(clase("x", b"RIFF\0\0\0\0WAVEfmt ").1, "wav");
        assert_eq!(clase("x", &ftyp(b"M4A ")).0, Clase::Audio);
    }

    #[test]
    fn webm_y_mp4_se_desempatan_por_el_nombre() {
        let webm = [0x1A, 0x45, 0xDF, 0xA3, 0x9F];
        assert_eq!(clase("nota-de-voz.weba", &webm), (Clase::Audio, "weba".into(), "audio/webm"));
        assert_eq!(clase("video.webm", &webm).0, Clase::Video);
        assert_eq!(clase("sin-nombre", &webm).0, Clase::Video);
        assert_eq!(clase("nota-de-voz.m4a", &ftyp(b"iso5")), (Clase::Audio, "m4a".into(), "audio/mp4"));
        assert_eq!(clase("VID_2026.mp4", &ftyp(b"isom")), (Clase::Video, "mp4".into(), "video/mp4"));
        assert_eq!(clase("IMG_0001.MOV", &ftyp(b"qt  ")).1, "mov");
    }

    #[test]
    fn heic_se_baja_pero_no_se_ve() {
        assert_eq!(clase("IMG_0001.HEIC", &ftyp(b"heic")), (Clase::Archivo, "heic".into(), "image/heic"));
    }

    #[test]
    fn office_por_la_extension_dentro_del_zip_o_ole() {
        assert_eq!(clase("Tesis Final.DOCX", b"PK\x03\x04\x14\0").1, "docx");
        assert_eq!(clase("ventas.xlsx", b"PK\x03\x04").0, Clase::Office);
        assert_eq!(clase("expo.pptx", b"PK\x03\x04").0, Clase::Office);
        assert_eq!(clase("fotos.zip", b"PK\x03\x04"), (Clase::Archivo, "zip".into(), "application/zip"));
        assert_eq!(clase("viejo.doc", &[0xD0, 0xCF, 0x11, 0xE0, 0xA1]).1, "doc");
        assert_eq!(clase("raro.msg", &[0xD0, 0xCF, 0x11, 0xE0]), (Clase::Archivo, "msg".into(), "application/octet-stream"));
        assert_eq!(clase("carta.rtf", b"{\\rtf1\\ansi").0, Clase::Office);
    }

    #[test]
    fn lo_demas_se_baja_con_una_extension_segura() {
        assert_eq!(clase("logo.CDR", b"RIFF\0\0\0\0CDR"), (Clase::Archivo, "cdr".into(), "application/octet-stream"));
        assert_eq!(clase("sin extension", b"hola").1, "bin");
        assert_eq!(clase("../../etc/pa ss.w%d", b"hola").1, "bin");
        assert_eq!(clase("muy.largaextension", b"hola").1, "bin");
        assert_eq!(clase("", b"").1, "bin");
    }

    #[test]
    fn limites() {
        let a = Ajustes::default();
        assert_eq!(limite(Clase::Pdf, &a), 25 * 1024 * 1024);
        assert_eq!(limite(Clase::Video, &a), 95 * 1024 * 1024, "100 en el ajuste, pero Cloudflare corta en 100");
        let chico = Ajustes { mb_video: 50, mb_documento: 10, ..Ajustes::default() };
        assert_eq!(limite(Clase::Video, &chico), 50 * 1024 * 1024);
        assert_eq!(limite(Clase::Imagen, &chico), 10 * 1024 * 1024);
        assert_eq!(limite_mayor(&chico), 50 * 1024 * 1024);
    }

    #[test]
    fn nombres_que_se_muestran() {
        assert_eq!(nombre_visible("C:\\Users\\ana\\Tesis final.docx", "docx"), "Tesis final.docx");
        assert_eq!(nombre_visible("../../etc/passwd", "bin"), "passwd");
        assert_eq!(nombre_visible("  \n ", "jpg"), "archivo.jpg");
        assert_eq!(nombre_visible(&"a".repeat(300), "pdf").chars().count(), 120);
    }

    #[test]
    fn miniatura_de_una_foto_grande() {
        let foto = image::RgbImage::from_pixel(1600, 1200, image::Rgb([200, 30, 30]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(foto).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let mini = miniatura(&png).unwrap();
        let leida = image::load_from_memory(&mini).unwrap();
        assert_eq!((leida.width(), leida.height()), (480, 360));
        assert_eq!(miniatura(b"no es imagen"), None);
    }

    #[test]
    fn que_se_ve_en_el_navegador() {
        assert!(en_linea("image/jpeg") && en_linea("video/mp4") && en_linea("application/pdf") && en_linea("audio/webm"));
        assert!(!en_linea("text/html") && !en_linea("image/svg+xml") && !en_linea("application/octet-stream"));
    }

    #[test]
    fn disposicion_con_acentos() {
        assert_eq!(
            disposicion(false, "Tésis \"final\".docx"),
            "attachment; filename=\"T_sis _final_.docx\"; filename*=UTF-8''T%C3%A9sis%20%22final%22.docx"
        );
        assert!(disposicion(true, "a.pdf").starts_with("inline; "));
    }

    fn adj(nombre: &str, mime: &str, llave: &str, pdf: Option<&str>) -> super::super::contrato::Adjunto {
        super::super::contrato::Adjunto {
            nombre: nombre.into(),
            mime: mime.into(),
            bytes: 1,
            llave: llave.into(),
            paginas: None,
            pdf: pdf.map(Into::into),
            miniatura: None,
        }
    }

    #[test]
    fn zip_de_puras_fotos_sin_carpetas_y_sin_repetidos() {
        let fotos = [
            adj("IMG_1.jpg", "image/jpeg", "c/m/1.jpg", None),
            adj("img_1.JPG", "image/jpeg", "c/m/2.jpg", None),
            adj("IMG_1.jpg", "image/jpeg", "c/m/3.jpg", None),
            adj("WhatsApp Image", "image/jpeg", "c/m/4.jpg", None),
        ];
        let rutas: Vec<String> = rutas_zip(&fotos).into_iter().map(|(_, r)| r).collect();
        assert_eq!(rutas, ["IMG_1.jpg", "img_1 (2).JPG", "IMG_1 (3).jpg", "WhatsApp Image.jpg"]);
    }

    #[test]
    fn zip_mezclado_por_carpetas_con_el_pdf_del_word() {
        let todo = [
            adj("portada.png", "image/png", "c/m/1.png", None),
            adj("Tesis: final.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "c/m/2.docx", Some("c/m/2-pdf.pdf")),
            adj("nota-de-voz.weba", "audio/webm", "c/m/3.weba", None),
            adj("../../x.mp4", "video/mp4", "c/m/4.mp4", None),
        ];
        assert_eq!(
            rutas_zip(&todo),
            [
                ("c/m/1.png".to_string(), "fotos/portada.png".to_string()),
                ("c/m/2.docx".to_string(), "documentos/Tesis_ final.docx".to_string()),
                ("c/m/2-pdf.pdf".to_string(), "documentos/Tesis_ final.pdf".to_string()),
                ("c/m/3.weba".to_string(), "audios/nota-de-voz.weba".to_string()),
                ("c/m/4.mp4".to_string(), "videos/_.._x.mp4".to_string()),
            ]
        );
    }

    #[test]
    fn llaves_en_la_carpeta_del_cliente_y_del_mensaje() {
        let (c, m, a) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
        let l = llaves(c, m, a, "docx");
        assert_eq!(l.original, format!("{c}/{m}/{a}.docx"));
        assert_eq!(l.pdf, format!("{c}/{m}/{a}-pdf.pdf"));
        assert_eq!(l.miniatura, format!("{c}/{m}/{a}-mini.jpg"));
        // Un PDF original no choca con el convertido.
        assert_ne!(llaves(c, m, a, "pdf").original, l.pdf);
    }
}
