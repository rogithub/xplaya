//! Autoservicio de impresión ("Imprime tú solito"): las cuentas de un trabajo, sin BD ni red.
//!
//! El cliente sube un PDF y elige páginas, copias y una cara / doble cara. Se cobra **por cara
//! impresa** ($1 la cara en b/n): caras × copias × precio. La doble cara solo ahorra papel.
//! Diseño: `ROADMAP.md` (Autoservicio de impresión) y en el POS
//! `docs/decisiones/2026-11-14-autoservicio-de-impresion.md`.

use rust_decimal::Decimal;
use serde::Serialize;

/// Tope de papel por trabajo.
pub const MAX_HOJAS: u32 = 100;
/// Tope de copias de un trabajo (aunque quepa en las hojas, más es un error de dedo).
pub const MAX_COPIAS: u32 = 50;
/// Tamaño máximo del PDF.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;

/// Lo que eligió el cliente.
#[derive(Debug, Clone)]
pub struct Opciones {
    /// "" = todas; si no, "1-3, 5".
    pub paginas: String,
    pub copias: u32,
    pub doble_cara: bool,
}

/// La cuenta de un trabajo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Cuenta {
    /// Caras impresas por copia (las páginas elegidas).
    pub caras: u32,
    /// Papel de todo el trabajo.
    pub hojas: u32,
    /// caras × copias × precio de la cara.
    pub total: Decimal,
    /// Las páginas, normalizadas ("" = todas): lo que se guarda y lo que recibe la impresora.
    pub paginas: String,
}

/// Saca la cuenta, o el mensaje para el cliente si algo no cuadra.
pub fn calcular(paginas_pdf: u32, opciones: &Opciones, precio_cara: Decimal) -> Result<Cuenta, String> {
    if opciones.copias == 0 {
        return Err("Pon al menos 1 copia.".into());
    }
    if opciones.copias > MAX_COPIAS {
        return Err(format!("Máximo {MAX_COPIAS} copias. Para más, pídelo en caja."));
    }
    let rangos = leer_paginas(&opciones.paginas, paginas_pdf)?;
    let caras: u32 = rangos.iter().map(|(a, b)| b - a + 1).sum();
    let hojas_por_copia = if opciones.doble_cara { caras.div_ceil(2) } else { caras };
    let hojas = hojas_por_copia * opciones.copias;
    if hojas > MAX_HOJAS {
        return Err(format!(
            "Son {hojas} hojas y el máximo es {MAX_HOJAS}. Para trabajos grandes, pídelo en caja."
        ));
    }
    Ok(Cuenta {
        caras,
        hojas,
        total: Decimal::from(caras * opciones.copias) * precio_cara,
        paginas: escribir_paginas(&rangos, paginas_pdf),
    })
}

/// "1-3, 5" → [(1, 3), (5, 5)], ordenados y sin encimarse. Vacío = todas.
fn leer_paginas(texto: &str, paginas_pdf: u32) -> Result<Vec<(u32, u32)>, String> {
    let texto = texto.trim();
    if texto.is_empty() || texto.eq_ignore_ascii_case("todas") {
        return Ok(vec![(1, paginas_pdf)]);
    }
    let mut rangos = Vec::new();
    for parte in texto.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = match parte.split_once('-') {
            Some((a, b)) => (numero(a)?, numero(b)?),
            None => {
                let n = numero(parte)?;
                (n, n)
            }
        };
        if a == 0 || b == 0 {
            return Err("Las páginas empiezan en 1.".into());
        }
        if a > b {
            return Err(format!("El rango {a}-{b} está al revés: escribe {b}-{a}."));
        }
        if b > paginas_pdf {
            let paginas = if paginas_pdf == 1 { "1 página".to_string() } else { format!("{paginas_pdf} páginas") };
            return Err(format!("Tu PDF tiene {paginas}; no existe la {b}."));
        }
        rangos.push((a, b));
    }
    if rangos.is_empty() {
        return Err("Escribe las páginas, por ejemplo 1-3, 5.".into());
    }
    // Ordenados y juntando los que se enciman ("1-5, 3-7" → 1-7): cada página sale una vez.
    rangos.sort();
    let mut juntos: Vec<(u32, u32)> = Vec::new();
    for (a, b) in rangos {
        match juntos.last_mut() {
            Some(ultimo) if a <= ultimo.1 + 1 => ultimo.1 = ultimo.1.max(b),
            _ => juntos.push((a, b)),
        }
    }
    Ok(juntos)
}

fn numero(texto: &str) -> Result<u32, String> {
    texto
        .trim()
        .parse()
        .map_err(|_| format!("No entiendo \"{}\". Escribe las páginas así: 1-3, 5.", texto.trim()))
}

/// [(1, 3), (5, 5)] → "1-3,5"; todas → "".
fn escribir_paginas(rangos: &[(u32, u32)], paginas_pdf: u32) -> String {
    if rangos == [(1, paginas_pdf)] {
        return String::new();
    }
    rangos
        .iter()
        .map(|&(a, b)| if a == b { a.to_string() } else { format!("{a}-{b}") })
        .collect::<Vec<_>>()
        .join(",")
}

/// Páginas del PDF, o el mensaje para el cliente si no se puede abrir.
pub fn contar_paginas(bytes: &[u8]) -> Result<u32, String> {
    if !bytes.starts_with(b"%PDF") {
        return Err("Ese archivo no es un PDF.".into());
    }
    let doc = lopdf::Document::load_mem(bytes)
        .map_err(|_| "No pudimos abrir tu PDF. ¿Está completo? Pídelo en caja.".to_string())?;
    if doc.is_encrypted() {
        return Err("Tu PDF tiene contraseña. Quítasela o pídelo en caja.".into());
    }
    match u32::try_from(doc.get_pages().len()) {
        Ok(0) | Err(_) => Err("Tu PDF no tiene páginas.".into()),
        Ok(n) => Ok(n),
    }
}

/// Nombre del PDF para guardarlo y mostrarlo: sin rutas ni caracteres de control, máx. 120.
pub fn nombre_archivo(original: &str) -> String {
    let base = original.rsplit(['/', '\\']).next().unwrap_or("");
    let limpio: String = base.chars().filter(|c| !c.is_control()).take(120).collect();
    let limpio = limpio.trim();
    if limpio.is_empty() { "documento.pdf".to_string() } else { limpio.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(paginas: &str, copias: u32, doble_cara: bool) -> Opciones {
        Opciones { paginas: paginas.into(), copias, doble_cara }
    }

    #[test]
    fn todas_una_cara() {
        let c = calcular(14, &op("", 1, false), Decimal::ONE).unwrap();
        assert_eq!((c.caras, c.hojas, c.total, c.paginas.as_str()), (14, 14, Decimal::from(14), ""));
    }

    #[test]
    fn doble_cara_ahorra_papel_y_cuesta_igual() {
        let c = calcular(13, &op("", 2, true), Decimal::ONE).unwrap();
        // 13 caras por copia → 7 hojas por copia (la última de un lado), 2 copias.
        assert_eq!((c.caras, c.hojas, c.total), (13, 14, Decimal::from(26)));
    }

    #[test]
    fn rangos_ordenados_y_sin_repetir() {
        let c = calcular(20, &op(" 5, 1-3 ,2-4", 1, false), Decimal::ONE).unwrap();
        assert_eq!((c.caras, c.paginas.as_str()), (5, "1-5"));
        let c = calcular(20, &op("10,1-2", 1, false), Decimal::ONE).unwrap();
        assert_eq!((c.caras, c.paginas.as_str()), (3, "1-2,10"));
    }

    #[test]
    fn escribir_todas_las_paginas_es_vacio() {
        assert_eq!(calcular(3, &op("1-3", 1, false), Decimal::ONE).unwrap().paginas, "");
        assert_eq!(calcular(3, &op("Todas", 1, false), Decimal::ONE).unwrap().paginas, "");
    }

    #[test]
    fn errores_para_el_cliente() {
        let e = |p: &str| calcular(10, &op(p, 1, false), Decimal::ONE).unwrap_err();
        assert!(e("12").contains("tiene 10 páginas"));
        assert!(e("5-2").contains("al revés"));
        assert!(e("0").contains("empiezan en 1"));
        assert!(e("uno").contains("No entiendo"));
        assert!(e(",").contains("Escribe las páginas"));
        assert!(calcular(10, &op("", 0, false), Decimal::ONE).unwrap_err().contains("al menos 1"));
    }

    #[test]
    fn topes() {
        assert!(calcular(101, &op("", 1, false), Decimal::ONE).unwrap_err().contains("máximo es 100"));
        // 101 caras a doble cara son 51 hojas: sí cabe.
        assert_eq!(calcular(101, &op("", 1, true), Decimal::ONE).unwrap().hojas, 51);
        assert!(calcular(2, &op("", 51, false), Decimal::ONE).unwrap_err().contains("Máximo 50 copias"));
    }

    /// PDF de `n` páginas en blanco, armado con lopdf.
    fn pdf(n: u32) -> Vec<u8> {
        use lopdf::{dictionary, Document, Object};
        let mut doc = Document::with_version("1.5");
        let paginas_id = doc.new_object_id();
        let hijos: Vec<Object> = (0..n)
            .map(|_| {
                doc.add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => paginas_id,
                    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                })
                .into()
            })
            .collect();
        doc.objects.insert(
            paginas_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => hijos, "Count" => n }),
        );
        let catalogo = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => paginas_id });
        doc.trailer.set("Root", catalogo);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn cuenta_las_paginas_del_pdf() {
        assert_eq!(contar_paginas(&pdf(3)), Ok(3));
        assert!(contar_paginas(b"hola").unwrap_err().contains("no es un PDF"));
        assert!(contar_paginas(b"%PDF-1.5 basura").unwrap_err().contains("No pudimos abrir"));
    }

    #[test]
    fn nombre_sin_rutas() {
        assert_eq!(nombre_archivo("C:\\docs\\tarea final.pdf"), "tarea final.pdf");
        assert_eq!(nombre_archivo("../../x.pdf"), "x.pdf");
        assert_eq!(nombre_archivo("  "), "documento.pdf");
    }
}
