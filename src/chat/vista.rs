//! Lo que la app recibe del chat: los mensajes como se dibujan y los eventos en vivo del SSE
//! (`routes/chat.rs`). Solo tipos para JSON; no es parte del contrato de NATS (ese es
//! `contrato.rs`) y puede cambiar sin subir `v`.

use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

use super::reglas::Nuevo;

/// Un mensaje como lo dibuja la app.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vista {
    pub id: Uuid,
    /// `cliente`, `papeleria`, `automatico` o `asistente`.
    pub autor: &'static str,
    pub tipo: &'static str,
    pub texto: Option<String>,
    pub responde_a: Option<Uuid>,
    /// Inicio del texto citado. En vivo va vacío: la app lo busca en lo que ya tiene.
    pub cita: Option<String>,
    /// Hora de la tienda.
    pub fecha: NaiveDateTime,
    pub leido: bool,
    pub editado: bool,
    pub eliminado: bool,
}

impl From<&Nuevo> for Vista {
    fn from(n: &Nuevo) -> Self {
        Vista {
            id: n.id,
            autor: n.autor.texto(),
            tipo: n.tipo.texto(),
            texto: n.texto.clone(),
            responde_a: n.responde_a,
            cita: None,
            fecha: n.fecha,
            leido: false,
            editado: false,
            eliminado: false,
        }
    }
}

/// Un cambio en la conversación de un cliente, para los celulares que la tienen abierta. El
/// nombre del evento SSE es `evento`; el resto va de datos.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "evento", rename_all = "lowercase")]
pub enum EnVivo {
    Mensaje(Vista),
    /// `delCliente`: la papelería leyó lo del cliente (✓✓); si no, el cliente leyó lo de la
    /// papelería (desde otro celular o pestaña).
    #[serde(rename_all = "camelCase")]
    Leido { del_cliente: bool, hasta: NaiveDateTime },
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
    }
}
