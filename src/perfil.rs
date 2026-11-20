//! "Mi perfil" en la app: el nombre y la foto con los que la papelería ve al cliente en el chat,
//! como en WhatsApp. El nombre va en `Clientes.NombreChat` (no toca `Nombre`, que es el de la
//! caja); la foto, cuadrada de 256 px, en MinIO (`papeleria-perfiles`) con su llave en
//! `Clientes.FotoPerfil`. La BD, en `db/perfil.rs`; las rutas, en `routes/perfil.rs`.

use image::imageops::FilterType;

/// Lado de la foto guardada: en el chat sale de 40 a 96 px; 256 alcanza con pantallas de 3x.
pub const LADO: u32 = 256;
pub const MAX_BYTES: usize = 10 * 1024 * 1024;
/// El tamaño de `Clientes.NombreChat`.
pub const MAX_NOMBRE: usize = 40;

/// La foto del cliente recortada al centro en cuadro y en `LADO`×`LADO` JPEG. Respeta la
/// rotación de la cámara (EXIF).
pub fn preparar_foto(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = crate::anuncio::abrir_imagen(bytes).ok_or("No pudimos leer tu foto. Usa una foto JPG o PNG.")?;
    let (w, h) = (img.width(), img.height());
    let lado = w.min(h);
    let cuadro = img.crop_imm((w - lado) / 2, (h - lado) / 2, lado, lado);
    let foto = image::imageops::resize(&cuadro.to_rgb8(), LADO, LADO, FilterType::Lanczos3);
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85)
        .encode_image(&foto)
        .map_err(|e| format!("JPEG: {e}"))?;
    Ok(jpeg)
}

/// El nombre como se guarda: sin espacios de más. Vacío quiere decir quitarlo (`None`).
pub fn limpiar_nombre(nombre: &str) -> Result<Option<String>, String> {
    let limpio = nombre.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpio.chars().count() > MAX_NOMBRE {
        return Err(format!("Tu nombre pasa de {MAX_NOMBRE} letras."));
    }
    Ok(Some(limpio).filter(|n| !n.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        // Mitad izquierda roja y derecha azul: el recorte al centro deja las dos.
        let img = RgbImage::from_fn(w, h, |x, _| if x < w / 2 { Rgb([200, 30, 30]) } else { Rgb([30, 30, 200]) });
        let mut b = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut b).encode_image(&img).unwrap();
        b
    }

    #[test]
    fn foto_cuadrada_al_centro() {
        for (w, h) in [(1600, 900), (900, 1600), (100, 100)] {
            let foto = image::load_from_memory(&preparar_foto(&jpeg(w, h)).unwrap()).unwrap().to_rgb8();
            assert_eq!(foto.dimensions(), (LADO, LADO), "{w}×{h}");
            let (izq, der) = (foto.get_pixel(10, 128), foto.get_pixel(245, 128));
            assert!(izq[0] > 150 && der[2] > 150, "{w}×{h}: quedó el centro");
        }
        assert!(preparar_foto(b"no es foto").is_err());
    }

    #[test]
    fn nombre() {
        assert_eq!(limpiar_nombre("  Lupita   Ramírez \n"), Ok(Some("Lupita Ramírez".into())));
        assert_eq!(limpiar_nombre("   "), Ok(None));
        assert_eq!(limpiar_nombre(&"ñ".repeat(40)).map(|n| n.map(|n| n.chars().count())), Ok(Some(40)));
        assert!(limpiar_nombre(&"a".repeat(41)).is_err());
    }
}
