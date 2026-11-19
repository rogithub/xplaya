//! Anuncios del kiosko: cada cliente con la app pone uno (imagen 9:16 como su estado de WhatsApp,
//! o texto sobre un color) y, cuando la caja lo aprueba en el POS, el kiosko lo pasa 7 días en su
//! franja derecha. Decisión: `inventario_papeleria/docs/decisiones/2026-11-17-anuncios-del-kiosko.md`.
//!
//! Aquí lo que no depende de la BD ni de HTTP: dejar la imagen en 1080×1920, los colores del
//! texto y en qué va el anuncio. La BD, en `db/anuncios.rs`; las rutas, en `routes/anuncio.rs`.

use std::io::Cursor;

use image::{imageops::FilterType, DynamicImage, ImageDecoder, ImageReader, Limits, RgbImage};

/// Lo que se le dice al cliente: la misma imagen de su estado de WhatsApp.
pub const ANCHO: u32 = 1080;
pub const ALTO: u32 = 1920;
pub const MAX_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_TEXTO: usize = 250;

/// Colores del anuncio de texto, como los de los estados de WhatsApp: oscuros, para letra blanca.
pub const COLORES: [&str; 8] =
    ["#128C7E", "#075E54", "#1F6FEB", "#2C3E50", "#7B3FA0", "#B0306A", "#C0392B", "#D35400"];

pub fn color_valido(color: &str) -> bool {
    COLORES.iter().any(|c| c.eq_ignore_ascii_case(color))
}

/// Deja la imagen del cliente en 1080×1920 JPEG. Si no es 9:16 no se recorta: va completa,
/// centrada sobre la misma foto agrandada y borrosa (como hace WhatsApp). Respeta la rotación
/// que guarda la cámara del celular (EXIF).
pub fn preparar_imagen(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = abrir_imagen(bytes).ok_or("No pudimos leer tu imagen. Usa una foto JPG o PNG.")?;
    let lienzo = acomodar(&img.to_rgb8());
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85)
        .encode_image(&lienzo)
        .map_err(|e| format!("JPEG: {e}"))?;
    Ok(jpeg)
}

/// Abre una foto del celular (JPEG, PNG o WebP) ya derecha según su EXIF, con tope de tamaño.
/// También la usan las miniaturas del chat (`chat/archivos.rs`).
pub fn abrir_imagen(bytes: &[u8]) -> Option<DynamicImage> {
    let mut lector = ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    // Una imagen chiquita en bytes puede pedir gigas al abrirse: tope de 12 000 px por lado.
    let mut limites = Limits::default();
    limites.max_image_width = Some(12_000);
    limites.max_image_height = Some(12_000);
    lector.limits(limites);
    let mut decodificador = lector.into_decoder().ok()?;
    let orientacion = decodificador.orientation().ok()?;
    let mut img = DynamicImage::from_decoder(decodificador).ok()?;
    img.apply_orientation(orientacion);
    Some(img)
}

fn acomodar(img: &RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    // Casi 9:16 (2 % de diferencia): se llena el lienzo; lo que se recorta ni se nota.
    let proporcion = (w as f64 / h as f64) / (ANCHO as f64 / ALTO as f64);
    if (0.98..=1.02).contains(&proporcion) {
        return image::imageops::resize(img, ANCHO, ALTO, FilterType::Lanczos3);
    }
    // Fondo: la foto llenando el lienzo, borrosa y un poco más oscura. Se emborrona en chico
    // (54×96) y se agranda: mucho más rápido que un desenfoque a tamaño completo.
    let chico = DynamicImage::ImageRgb8(img.clone()).resize_to_fill(54, 96, FilterType::Triangle);
    let chico = image::imageops::blur(&chico.to_rgb8(), 3.0);
    let mut lienzo = image::imageops::resize(&chico, ANCHO, ALTO, FilterType::Triangle);
    for p in lienzo.pixels_mut() {
        for c in p.0.iter_mut() {
            *c = (*c as u16 * 7 / 10) as u8;
        }
    }
    // Encima, la foto completa lo más grande que quepa.
    let frente = DynamicImage::ImageRgb8(img.clone()).resize(ANCHO, ALTO, FilterType::Lanczos3).to_rgb8();
    let x = (ANCHO - frente.width()) / 2;
    let y = (ALTO - frente.height()) / 2;
    image::imageops::overlay(&mut lienzo, &frente, x as i64, y as i64);
    lienzo
}

/// En qué va el anuncio, para el cliente.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(tag = "clave")]
pub enum Situacion {
    /// La caja todavía no lo revisa.
    Esperando,
    /// Sale en el kiosko; días que le quedan contando el de hoy.
    EnKiosko { dias: i64 },
    /// La caja no lo aprobó o lo quitó (con el motivo, si lo escribió).
    NoAprobado { motivo: Option<String> },
    /// Ya pasaron sus 7 días.
    Vencido,
}

pub fn situacion(
    estado: &str,
    motivo: Option<&str>,
    vence: Option<chrono::NaiveDateTime>,
    ahora: chrono::NaiveDateTime,
) -> Situacion {
    match (estado, vence) {
        ("aprobado", Some(v)) if v > ahora => {
            let segundos = (v - ahora).num_seconds();
            Situacion::EnKiosko { dias: (segundos + 86_399) / 86_400 }
        }
        ("aprobado", _) => Situacion::Vencido,
        ("rechazado", _) => Situacion::NoAprobado { motivo: motivo.map(str::to_string) },
        _ => Situacion::Esperando,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn ahora() -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 17).unwrap().and_hms_opt(10, 0, 0).unwrap()
    }

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
        let mut b = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut b).encode_image(&img).unwrap();
        b
    }

    fn medidas(bytes: &[u8]) -> (u32, u32) {
        image::load_from_memory(bytes).unwrap().to_rgb8().dimensions()
    }

    #[test]
    fn vertical_9_16_queda_en_1080x1920() {
        assert_eq!(medidas(&preparar_imagen(&jpeg(720, 1280)).unwrap()), (ANCHO, ALTO));
    }

    #[test]
    fn horizontal_va_completa_sobre_fondo_borroso() {
        let salida = image::load_from_memory(&preparar_imagen(&jpeg(1600, 900)).unwrap()).unwrap().to_rgb8();
        assert_eq!(salida.dimensions(), (ANCHO, ALTO));
        // El centro es la foto; arriba, el fondo oscurecido (70 % del rojo).
        let centro = salida.get_pixel(ANCHO / 2, ALTO / 2).0[0];
        let arriba = salida.get_pixel(ANCHO / 2, 20).0[0];
        assert!(centro > 180, "centro {centro}");
        assert!(arriba < 160, "arriba {arriba}");
    }

    #[test]
    fn lo_que_no_es_imagen_se_rechaza() {
        assert!(preparar_imagen(b"%PDF-1.4 no soy foto").is_err());
    }

    #[test]
    fn colores_de_la_paleta() {
        assert!(color_valido("#128c7e"));
        assert!(!color_valido("#FFFFFF"));
        assert!(!color_valido("red"));
    }

    #[test]
    fn situaciones() {
        let a = ahora();
        assert_eq!(situacion("por_aprobar", None, None, a), Situacion::Esperando);
        assert_eq!(situacion("aprobado", None, Some(a + chrono::Duration::days(7)), a), Situacion::EnKiosko { dias: 7 });
        assert_eq!(situacion("aprobado", None, Some(a + chrono::Duration::hours(30)), a), Situacion::EnKiosko { dias: 2 });
        assert_eq!(situacion("aprobado", None, Some(a - chrono::Duration::hours(1)), a), Situacion::Vencido);
        assert_eq!(
            situacion("rechazado", Some("No se lee"), None, a),
            Situacion::NoAprobado { motivo: Some("No se lee".into()) }
        );
    }
}
