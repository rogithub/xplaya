//! Lo que la app recibe del chat: los mensajes como se dibujan y los eventos en vivo del SSE
//! (`routes/chat.rs`). Solo tipos para JSON; no es parte del contrato de NATS (ese es
//! `contrato.rs`) y puede cambiar sin subir `v`.

use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

use super::{contrato::Adjunto, reglas::Nuevo, tarjetas::Tarjeta};

/// Un mensaje como lo dibuja la app.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vista {
    pub id: Uuid,
    /// `cliente`, `papeleria`, `automatico` o `asistente`.
    pub autor: String,
    pub tipo: String,
    pub texto: Option<String>,
    pub responde_a: Option<Uuid>,
    /// Inicio del texto citado. En vivo va vacío: la app lo busca en lo que ya tiene.
    pub cita: Option<String>,
    /// Hora de la tienda.
    pub fecha: NaiveDateTime,
    pub leido: bool,
    /// Ya está en una pantalla de la caja (✓✓ grises). Solo en los del cliente.
    pub entregado: bool,
    pub editado: bool,
    pub eliminado: bool,
    /// Los archivos, con sus URLs. Vacío si no hay o si se eliminó.
    pub adjuntos: Vec<AdjuntoVista>,
    /// "Bajar todo": solo con más de un archivo.
    pub zip: Option<String>,
    /// El producto (nid), recibo o cotización de la tarjeta.
    #[serde(rename = "ref")]
    pub referencia: Option<String>,
    /// La tarjeta con datos al día (`tarjetas::poner`). `None` si el mensaje no es tarjeta o si
    /// ya no existe lo que señala.
    pub tarjeta: Option<Tarjeta>,
}

/// Un archivo del mensaje como lo dibuja la app. Las URLs van a `GET /chat/archivo/...`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AdjuntoVista {
    pub nombre: String,
    pub mime: String,
    pub bytes: u64,
    pub paginas: Option<u32>,
    /// `imagen`, `video`, `audio`, `pdf`, `office` (con su PDF) o `archivo` (solo se baja).
    pub clase: &'static str,
    pub original: String,
    pub pdf: Option<String>,
    pub mini: Option<String>,
}

/// Cómo se dibuja un archivo. Un Word cuya conversión falló queda como `archivo`.
fn clase(a: &Adjunto) -> &'static str {
    if a.pdf.is_some() {
        return "office";
    }
    match a.mime.split('/').next() {
        Some("image") => "imagen",
        Some("video") => "video",
        Some("audio") => "audio",
        _ if a.mime == "application/pdf" => "pdf",
        _ => "archivo",
    }
}

/// Los archivos de un mensaje con sus URLs, y la de "Bajar todo" si hay más de uno.
pub fn adjuntos_vista(mensaje: Uuid, adjuntos: &[Adjunto]) -> (Vec<AdjuntoVista>, Option<String>) {
    let url = |n: usize, cual: &str| format!("/chat/archivo/{mensaje}/{n}/{cual}");
    let vista = adjuntos
        .iter()
        .enumerate()
        .map(|(n, a)| AdjuntoVista {
            nombre: a.nombre.clone(),
            mime: a.mime.clone(),
            bytes: a.bytes,
            paginas: a.paginas,
            clase: clase(a),
            original: url(n, "original"),
            pdf: a.pdf.as_ref().map(|_| url(n, "pdf")),
            mini: a.miniatura.as_ref().map(|_| url(n, "mini")),
        })
        .collect();
    let zip = (adjuntos.len() > 1).then(|| format!("/chat/zip/{mensaje}"));
    (vista, zip)
}

impl From<&Nuevo> for Vista {
    fn from(n: &Nuevo) -> Self {
        let (adjuntos, zip) = adjuntos_vista(n.id, &n.adjuntos);
        Vista {
            id: n.id,
            autor: n.autor.texto().to_string(),
            tipo: n.tipo.texto().to_string(),
            texto: n.texto.clone(),
            responde_a: n.responde_a,
            cita: None,
            fecha: n.fecha,
            leido: false,
            entregado: false,
            editado: false,
            eliminado: false,
            adjuntos,
            zip,
            referencia: n.referencia.clone(),
            tarjeta: None,
        }
    }
}

/// Un cambio en la conversación de un cliente, para los celulares que la tienen abierta. El
/// nombre del evento SSE es `evento`; el resto va de datos.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "evento", rename_all = "lowercase")]
pub enum EnVivo {
    Mensaje(Box<Vista>),
    /// `delCliente`: la papelería leyó lo del cliente (✓✓); si no, el cliente leyó lo de la
    /// papelería (desde otro celular o pestaña).
    #[serde(rename_all = "camelCase")]
    Leido { del_cliente: bool, hasta: NaiveDateTime },
    /// Lo del cliente hasta `hasta` ya está en una pantalla de la caja (✓✓ grises).
    Entregado { hasta: NaiveDateTime },
    Editado { id: Uuid, texto: String, fecha: NaiveDateTime },
    Eliminado { id: Uuid, fecha: NaiveDateTime },
    /// La papelería está escribiendo (NATS núcleo, no se guarda).
    Escribiendo,
}

impl EnVivo {
    pub fn nombre(&self) -> &'static str {
        match self {
            EnVivo::Mensaje(_) => "mensaje",
            EnVivo::Leido { .. } => "leido",
            EnVivo::Entregado { .. } => "entregado",
            EnVivo::Editado { .. } => "editado",
            EnVivo::Eliminado { .. } => "eliminado",
            EnVivo::Escribiendo => "escribiendo",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_eventos_llevan_su_nombre_en_el_json() {
        let hora = NaiveDateTime::parse_from_str("2026-11-19 17:42:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let e = EnVivo::Leido { del_cliente: true, hasta: hora };
        assert_eq!(e.nombre(), "leido");
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({ "evento": "leido", "delCliente": true, "hasta": "2026-11-19T17:42:00" })
        );
        assert_eq!(serde_json::to_value(EnVivo::Escribiendo).unwrap(), serde_json::json!({ "evento": "escribiendo" }));
        let e = EnVivo::Entregado { hasta: hora };
        assert_eq!(e.nombre(), "entregado");
        assert_eq!(serde_json::to_value(&e).unwrap(), serde_json::json!({ "evento": "entregado", "hasta": "2026-11-19T17:42:00" }));
    }

    #[test]
    fn los_archivos_llevan_sus_urls() {
        let id = Uuid::parse_str("f60de3e0-88fc-40ec-b793-9302bc4478d0").unwrap();
        let adjunto = |nombre: &str, mime: &str, pdf: bool, mini: bool| Adjunto {
            nombre: nombre.into(),
            mime: mime.into(),
            bytes: 10,
            llave: "k".into(),
            paginas: None,
            pdf: pdf.then(|| "k.pdf".into()),
            miniatura: mini.then(|| "k.mini.jpg".into()),
        };
        let docx = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
        let (uno, zip) = adjuntos_vista(id, &[adjunto("a.jpg", "image/jpeg", false, true)]);
        assert_eq!(zip, None);
        assert_eq!(
            serde_json::to_value(&uno[0]).unwrap(),
            serde_json::json!({
                "nombre": "a.jpg", "mime": "image/jpeg", "bytes": 10, "paginas": null, "clase": "imagen",
                "original": format!("/chat/archivo/{id}/0/original"), "pdf": null,
                "mini": format!("/chat/archivo/{id}/0/mini"),
            })
        );
        let (varios, zip) = adjuntos_vista(
            id,
            &[
                adjunto("t.docx", docx, true, false),
                adjunto("t2.docx", docx, false, false),
                adjunto("v.mp4", "video/mp4", false, false),
                adjunto("n.ogg", "audio/ogg", false, false),
                adjunto("x.pdf", "application/pdf", false, false),
            ],
        );
        assert_eq!(zip, Some(format!("/chat/zip/{id}")));
        assert_eq!(varios[0].pdf, Some(format!("/chat/archivo/{id}/0/pdf")));
        let clases: Vec<_> = varios.iter().map(|a| a.clase).collect();
        assert_eq!(clases, ["office", "archivo", "video", "audio", "pdf"], "un Word sin PDF solo se baja");
    }
}
