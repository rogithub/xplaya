//! El contrato del chat, versión 1 (`CHAT.md`, ejemplos en `contratos/chat-v1/`).
//!
//! Lo comparten xplaya, el POS y el asistente: si cambia, sube `v`. Aquí solo hay tipos y cómo
//! leer el tema; nada de red ni de base de datos.

use chrono::{DateTime, FixedOffset, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const VERSION: u32 = 1;

/// Quién publica. Sale del tema, no del JSON: los permisos de NATS deciden quién puede usar cada lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Cliente,
    Papeleria,
    Automatico,
    Asistente,
}

impl Lado {
    pub fn leer(texto: &str) -> Option<Self> {
        match texto {
            "cliente" => Some(Self::Cliente),
            "papeleria" => Some(Self::Papeleria),
            "automatico" => Some(Self::Automatico),
            "asistente" => Some(Self::Asistente),
            _ => None,
        }
    }

    /// Como va en el tema y en la columna `Mensajes.Autor`.
    pub fn texto(self) -> &'static str {
        match self {
            Self::Cliente => "cliente",
            Self::Papeleria => "papeleria",
            Self::Automatico => "automatico",
            Self::Asistente => "asistente",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    Mensaje,
    Leido,
    Editado,
    Eliminado,
    /// NATS núcleo, no pasa por el stream.
    Escribiendo,
    /// Lo que propone el asistente (etapa 8).
    Sugerencia,
}

impl Evento {
    fn leer(texto: &str) -> Option<Self> {
        match texto {
            "mensaje" => Some(Self::Mensaje),
            "leido" => Some(Self::Leido),
            "editado" => Some(Self::Editado),
            "eliminado" => Some(Self::Eliminado),
            "escribiendo" => Some(Self::Escribiendo),
            "sugerencia" => Some(Self::Sugerencia),
            _ => None,
        }
    }

    fn texto(self) -> &'static str {
        match self {
            Self::Mensaje => "mensaje",
            Self::Leido => "leido",
            Self::Editado => "editado",
            Self::Eliminado => "eliminado",
            Self::Escribiendo => "escribiendo",
            Self::Sugerencia => "sugerencia",
        }
    }
}

/// El tema de NATS: `chat.{clienteId}.{lado}.{evento}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tema {
    pub cliente: Uuid,
    pub lado: Lado,
    pub evento: Evento,
}

impl Tema {
    pub fn leer(tema: &str) -> Option<Self> {
        let mut partes = tema.split('.');
        let (Some("chat"), Some(cliente), Some(lado), Some(evento), None) =
            (partes.next(), partes.next(), partes.next(), partes.next(), partes.next())
        else {
            return None;
        };
        Some(Self { cliente: cliente.parse().ok()?, lado: Lado::leer(lado)?, evento: Evento::leer(evento)? })
    }

    /// Lo usa xplaya al publicar.
    pub fn armar(&self) -> String {
        format!("chat.{}.{}.{}", self.cliente, self.lado.texto(), self.evento.texto())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tipo {
    Texto,
    Aviso,
    Producto,
    Recibo,
    Cotizacion,
}

impl Tipo {
    /// Como va en la columna `Mensajes.Tipo`.
    pub fn texto(self) -> &'static str {
        match self {
            Self::Texto => "texto",
            Self::Aviso => "aviso",
            Self::Producto => "producto",
            Self::Recibo => "recibo",
            Self::Cotizacion => "cotizacion",
        }
    }
}

/// Un archivo del mensaje. Vive en MinIO (bucket `papeleria-chat`); aquí solo van sus llaves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adjunto {
    pub nombre: String,
    pub mime: String,
    pub bytes: u64,
    pub llave: String,
    /// PDF, o Word/Excel/PowerPoint ya convertido.
    pub paginas: Option<u32>,
    /// El PDF convertido; el original sigue en `llave`.
    pub pdf: Option<String>,
    /// Solo imágenes.
    pub miniatura: Option<String>,
}

impl Adjunto {
    /// Todas las llaves de MinIO de este archivo: original, PDF y miniatura.
    pub fn llaves(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.llave.as_str()).chain(self.pdf.as_deref()).chain(self.miniatura.as_deref())
    }
}

/// `chat.{cliente}.{lado}.mensaje`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mensaje {
    pub v: u32,
    pub id: Uuid,
    pub cliente_id: Uuid,
    pub tipo: Tipo,
    pub texto: Option<String>,
    #[serde(default)]
    pub adjuntos: Vec<Adjunto>,
    pub responde_a: Option<Uuid>,
    /// Producto (nid), recibo o cotización de la tarjeta.
    #[serde(rename = "ref")]
    pub referencia: Option<String>,
    pub encargo_id: Option<Uuid>,
    pub fecha: DateTime<FixedOffset>,
}

/// `chat.{cliente}.{lado}.leido`: ese lado leyó todo lo del otro lado hasta `hasta`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Leido {
    pub v: u32,
    pub cliente_id: Uuid,
    pub hasta: DateTime<FixedOffset>,
}

/// `chat.{cliente}.{lado}.editado`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Editado {
    pub v: u32,
    pub cliente_id: Uuid,
    pub id: Uuid,
    pub texto: String,
    pub fecha: DateTime<FixedOffset>,
}

/// `chat.{cliente}.{lado}.eliminado`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Eliminado {
    pub v: u32,
    pub cliente_id: Uuid,
    pub id: Uuid,
    pub fecha: DateTime<FixedOffset>,
}

/// Lo que llegó por el stream, ya leído según el evento del tema.
#[derive(Debug, Clone, PartialEq)]
pub enum Entrante {
    Mensaje(Mensaje),
    Leido(Leido),
    Editado(Editado),
    Eliminado(Eliminado),
    Sugerencia,
}

#[derive(Debug)]
pub enum NoSeLee {
    /// Una versión que este xplaya no conoce: se deja pasar sin romper nada.
    Version(Option<u32>),
    /// `escribiendo` no debería llegar por el stream.
    EventoFueraDelStream,
    Json(serde_json::Error),
}

impl std::fmt::Display for NoSeLee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Version(v) => write!(f, "versión {v:?} desconocida"),
            Self::EventoFueraDelStream => write!(f, "evento que no va por el stream"),
            Self::Json(e) => write!(f, "JSON: {e}"),
        }
    }
}

/// Lee el cuerpo según el evento. Primero mira `v`, para que un formato nuevo no se confunda con
/// uno roto.
pub fn leer(evento: Evento, cuerpo: &[u8]) -> Result<Entrante, NoSeLee> {
    #[derive(Deserialize)]
    struct SoloVersion {
        v: Option<u32>,
    }
    let v = serde_json::from_slice::<SoloVersion>(cuerpo).map_err(NoSeLee::Json)?.v;
    if v != Some(VERSION) {
        return Err(NoSeLee::Version(v));
    }
    let json = |e| NoSeLee::Json(e);
    Ok(match evento {
        Evento::Mensaje => Entrante::Mensaje(serde_json::from_slice(cuerpo).map_err(json)?),
        Evento::Leido => Entrante::Leido(serde_json::from_slice(cuerpo).map_err(json)?),
        Evento::Editado => Entrante::Editado(serde_json::from_slice(cuerpo).map_err(json)?),
        Evento::Eliminado => Entrante::Eliminado(serde_json::from_slice(cuerpo).map_err(json)?),
        Evento::Sugerencia => Entrante::Sugerencia,
        Evento::Escribiendo => return Err(NoSeLee::EventoFueraDelStream),
    })
}

/// La hora de la tienda (Cancún, UTC−5 todo el año), como se guardan las fechas en la BD.
pub fn hora_tienda(fecha: DateTime<FixedOffset>) -> NaiveDateTime {
    fecha.with_timezone(&cancun()).naive_local()
}

/// Lo contrario de `hora_tienda`: una fecha de la BD, con su zona, para el contrato.
pub fn con_zona(fecha: NaiveDateTime) -> DateTime<FixedOffset> {
    fecha.and_local_timezone(cancun()).single().expect("UTC−5 fijo: cada hora existe una vez")
}

fn cancun() -> FixedOffset {
    FixedOffset::west_opt(5 * 3600).expect("UTC−5 es válido")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENTE: &str = "8a1e5c2e-3f4b-4c6d-9e8f-0a1b2c3d4e5f";

    fn ejemplo(nombre: &str) -> Vec<u8> {
        let ruta = format!("{}/contratos/chat-v1/{nombre}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(&ruta).unwrap_or_else(|e| panic!("{ruta}: {e}"))
    }

    /// Cada ejemplo se lee y se vuelve a escribir igual: si alguien cambia un campo por error, falla.
    fn ida_y_vuelta<T: serde::de::DeserializeOwned + Serialize>(nombre: &str) {
        let original: serde_json::Value = serde_json::from_slice(&ejemplo(nombre)).unwrap();
        let leido: T = serde_json::from_value(original.clone()).unwrap_or_else(|e| panic!("{nombre}: {e}"));
        assert_eq!(serde_json::to_value(&leido).unwrap(), original, "{nombre}");
    }

    #[test]
    fn los_ejemplos_del_contrato_van_y_vuelven_iguales() {
        ida_y_vuelta::<Mensaje>("mensaje.json");
        ida_y_vuelta::<Mensaje>("mensaje-con-adjuntos.json");
        ida_y_vuelta::<Mensaje>("mensaje-voz.json");
        ida_y_vuelta::<Mensaje>("mensaje-tarjeta.json");
        ida_y_vuelta::<Leido>("leido.json");
        ida_y_vuelta::<Editado>("editado.json");
        ida_y_vuelta::<Eliminado>("eliminado.json");
    }

    #[test]
    fn leer_segun_el_evento() {
        assert!(matches!(leer(Evento::Mensaje, &ejemplo("mensaje.json")), Ok(Entrante::Mensaje(_))));
        assert!(matches!(leer(Evento::Leido, &ejemplo("leido.json")), Ok(Entrante::Leido(_))));
        assert!(matches!(leer(Evento::Editado, &ejemplo("editado.json")), Ok(Entrante::Editado(_))));
        assert!(matches!(leer(Evento::Eliminado, &ejemplo("eliminado.json")), Ok(Entrante::Eliminado(_))));
        // Un leído con el cuerpo de un mensaje no es un leído.
        assert!(matches!(leer(Evento::Leido, &ejemplo("mensaje.json")), Err(NoSeLee::Json(_))));
    }

    #[test]
    fn una_version_desconocida_no_se_confunde_con_un_json_roto() {
        assert!(matches!(leer(Evento::Mensaje, br#"{"v": 2, "otra": "forma"}"#), Err(NoSeLee::Version(Some(2)))));
        assert!(matches!(leer(Evento::Mensaje, br#"{"id": "x"}"#), Err(NoSeLee::Version(None))));
        assert!(matches!(leer(Evento::Mensaje, b"no es json"), Err(NoSeLee::Json(_))));
        assert!(matches!(leer(Evento::Escribiendo, &ejemplo("mensaje.json")), Err(NoSeLee::EventoFueraDelStream)));
    }

    #[test]
    fn el_tema_se_lee_y_se_arma() {
        let tema = Tema::leer(&format!("chat.{CLIENTE}.papeleria.editado")).unwrap();
        assert_eq!(tema.cliente.to_string(), CLIENTE);
        assert_eq!((tema.lado, tema.evento), (Lado::Papeleria, Evento::Editado));
        assert_eq!(tema.armar(), format!("chat.{CLIENTE}.papeleria.editado"));
    }

    #[test]
    fn temas_que_no_son_del_chat() {
        for tema in [
            "chat",
            "caja.cuenta",
            &format!("chat.{CLIENTE}.papeleria"),
            &format!("chat.{CLIENTE}.papeleria.mensaje.extra"),
            &format!("chat.{CLIENTE}.vecino.mensaje"),
            &format!("chat.{CLIENTE}.cliente.grito"),
            "chat.no-es-uuid.cliente.mensaje",
            &format!("otro.{CLIENTE}.cliente.mensaje"),
        ] {
            assert_eq!(Tema::leer(tema), None, "{tema}");
        }
    }

    #[test]
    fn la_hora_de_la_tienda() {
        let utc = DateTime::parse_from_rfc3339("2026-11-19T22:42:00Z").unwrap();
        assert_eq!(hora_tienda(utc).to_string(), "2026-11-19 17:42:00");
        let tienda = DateTime::parse_from_rfc3339("2026-11-19T17:42:00-05:00").unwrap();
        assert_eq!(con_zona(hora_tienda(utc)), tienda);
        assert_eq!(hora_tienda(tienda).to_string(), "2026-11-19 17:42:00");
    }

    #[test]
    fn llaves_de_un_adjunto() {
        let m: Mensaje = serde_json::from_slice(&ejemplo("mensaje-con-adjuntos.json")).unwrap();
        assert_eq!(m.adjuntos[0].llaves().count(), 2); // original y PDF
        assert_eq!(m.adjuntos[1].llaves().count(), 2); // original y miniatura
    }
}
