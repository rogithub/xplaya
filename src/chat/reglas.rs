//! Qué hacer con cada evento del stream `CHAT` (`CHAT.md`, "Reglas del código").
//!
//! Todo es puro: recibe el evento, lo que ya está guardado de ese mensaje y los ajustes, y
//! devuelve un `Cambio` sin ejecutarlo. Las horas que cuentan son las del evento (las pone quien
//! publica, xplaya o el POS), no la de cuando se procesa: si xplaya estuvo apagado un rato, una
//! edición hecha a tiempo sigue valiendo.

use chrono::NaiveDateTime;
use uuid::Uuid;

use super::contrato::{hora_tienda, Adjunto, Editado, Eliminado, Entrante, Evento, Lado, Leido, Mensaje, Tema, Tipo};

/// Los ajustes del chat que usan las reglas (tabla `Settings`, claves `CHAT_*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ajustes {
    pub minutos_editar: i64,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self { minutos_editar: 15 }
    }
}

/// Lo que ya está en `Mensajes` del mensaje al que se refiere el evento.
#[derive(Debug, Clone, PartialEq)]
pub struct Guardado {
    pub cliente: Uuid,
    pub autor: Lado,
    pub fecha: NaiveDateTime,
    pub eliminado: bool,
    pub adjuntos: Vec<Adjunto>,
    pub secuencia: Option<u64>,
}

/// Un mensaje nuevo, listo para guardarse.
#[derive(Debug, Clone, PartialEq)]
pub struct Nuevo {
    pub id: Uuid,
    pub cliente: Uuid,
    pub autor: Lado,
    pub tipo: Tipo,
    pub texto: Option<String>,
    pub adjuntos: Vec<Adjunto>,
    pub referencia: Option<String>,
    pub responde_a: Option<Uuid>,
    /// Secuencia en el stream `CHAT`, para quitarlo de ahí si se elimina.
    pub secuencia: u64,
    pub fecha: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cambio {
    Guardar(Nuevo),
    /// Marca leídos los mensajes del cliente (si leyó la papelería) o los que le escribieron al
    /// cliente (si leyó él), hasta esa hora.
    MarcarLeidos { cliente: Uuid, del_cliente: bool, hasta: NaiveDateTime },
    Editar { id: Uuid, texto: String, fecha: NaiveDateTime },
    /// Borra texto y adjuntos del mensaje; `llaves` son los archivos a borrar de MinIO y
    /// `secuencia`, el mensaje a quitar del stream.
    Eliminar { id: Uuid, fecha: NaiveDateTime, llaves: Vec<String>, secuencia: Option<u64> },
    /// No se hace nada; el motivo va al log.
    Ignorar(&'static str),
}

/// Decide qué hacer con un evento del stream.
///
/// - `guardado`: lo que hay en la BD del mensaje con el `id` del evento (para `mensaje`, `editado`
///   y `eliminado`; `None` si no existe).
/// - `secuencia`: la del evento en el stream.
pub fn aplicar(tema: &Tema, entrante: Entrante, guardado: Option<&Guardado>, secuencia: u64, ajustes: &Ajustes) -> Cambio {
    match entrante {
        Entrante::Mensaje(m) => mensaje(tema, m, guardado, secuencia),
        Entrante::Leido(l) => leido(tema, l),
        Entrante::Editado(e) => editado(tema, e, guardado, ajustes),
        Entrante::Eliminado(e) => eliminado(tema, e, guardado),
        Entrante::Sugerencia => Cambio::Ignorar("las sugerencias del asistente llegan en la etapa 8"),
    }
}

fn mensaje(tema: &Tema, m: Mensaje, guardado: Option<&Guardado>, secuencia: u64) -> Cambio {
    if tema.evento != Evento::Mensaje {
        return Cambio::Ignorar("el cuerpo es un mensaje pero el tema no");
    }
    if m.cliente_id != tema.cliente {
        return Cambio::Ignorar("el cliente del mensaje no es el del tema");
    }
    if guardado.is_some() {
        return Cambio::Ignorar("repetido: ya estaba guardado");
    }
    // Cada archivo tiene que estar en la carpeta de este cliente: si no, al eliminar el mensaje se
    // podrían borrar archivos de otro.
    let carpeta = format!("{}/", m.cliente_id);
    if m.adjuntos.iter().flat_map(Adjunto::llaves).any(|llave| !llave.starts_with(&carpeta)) {
        return Cambio::Ignorar("un archivo no está en la carpeta del cliente");
    }
    let texto = m.texto.filter(|t| !t.trim().is_empty());
    let referencia = m.referencia.filter(|r| !r.trim().is_empty());
    match m.tipo {
        Tipo::Texto if texto.is_none() && m.adjuntos.is_empty() => return Cambio::Ignorar("mensaje vacío"),
        Tipo::Aviso if tema.lado != Lado::Automatico => return Cambio::Ignorar("los avisos solo los manda xplaya"),
        Tipo::Aviso if texto.is_none() => return Cambio::Ignorar("aviso sin texto"),
        Tipo::Producto | Tipo::Recibo | Tipo::Cotizacion if referencia.is_none() => {
            return Cambio::Ignorar("tarjeta sin ref");
        }
        _ => {}
    }
    Cambio::Guardar(Nuevo {
        id: m.id,
        cliente: m.cliente_id,
        autor: tema.lado,
        tipo: m.tipo,
        texto,
        adjuntos: m.adjuntos,
        referencia,
        responde_a: m.responde_a,
        secuencia,
        fecha: hora_tienda(m.fecha),
    })
}

fn leido(tema: &Tema, l: Leido) -> Cambio {
    if l.cliente_id != tema.cliente {
        return Cambio::Ignorar("el cliente del leído no es el del tema");
    }
    let del_cliente = match tema.lado {
        Lado::Cliente => false,
        Lado::Papeleria => true,
        Lado::Automatico | Lado::Asistente => return Cambio::Ignorar("solo leen el cliente y la papelería"),
    };
    Cambio::MarcarLeidos { cliente: l.cliente_id, del_cliente, hasta: hora_tienda(l.hasta) }
}

fn editado(tema: &Tema, e: Editado, guardado: Option<&Guardado>, ajustes: &Ajustes) -> Cambio {
    let Some(g) = guardado else {
        return Cambio::Ignorar("editar: el mensaje no existe");
    };
    if let Some(motivo) = ajeno(tema, e.cliente_id, g) {
        return Cambio::Ignorar(motivo);
    }
    if g.eliminado {
        return Cambio::Ignorar("editar: el mensaje ya se eliminó");
    }
    let texto = e.texto.trim();
    if texto.is_empty() {
        return Cambio::Ignorar("editar: texto vacío (para quitarlo, se elimina)");
    }
    let fecha = hora_tienda(e.fecha);
    if (fecha - g.fecha).num_seconds() > ajustes.minutos_editar * 60 {
        return Cambio::Ignorar("editar: ya pasó el tiempo para editar");
    }
    Cambio::Editar { id: e.id, texto: texto.to_string(), fecha }
}

fn eliminado(tema: &Tema, e: Eliminado, guardado: Option<&Guardado>) -> Cambio {
    let Some(g) = guardado else {
        return Cambio::Ignorar("eliminar: el mensaje no existe");
    };
    if let Some(motivo) = ajeno(tema, e.cliente_id, g) {
        return Cambio::Ignorar(motivo);
    }
    if g.eliminado {
        return Cambio::Ignorar("eliminar: ya estaba eliminado");
    }
    Cambio::Eliminar {
        id: e.id,
        fecha: hora_tienda(e.fecha),
        llaves: g.adjuntos.iter().flat_map(Adjunto::llaves).map(str::to_string).collect(),
        secuencia: g.secuencia,
    }
}

/// Un lado solo edita o elimina sus propios mensajes, y solo los de ese cliente.
fn ajeno(tema: &Tema, cliente_del_evento: Uuid, g: &Guardado) -> Option<&'static str> {
    if cliente_del_evento != tema.cliente || g.cliente != tema.cliente {
        Some("el mensaje es de otro cliente")
    } else if g.autor != tema.lado {
        Some("el mensaje es del otro lado")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    const CLIENTE: Uuid = Uuid::from_u128(0x8a1e5c2e_3f4b_4c6d_9e8f_0a1b2c3d4e5f);
    const OTRO: Uuid = Uuid::from_u128(0x11111111_1111_4111_8111_111111111111);
    const ID: Uuid = Uuid::from_u128(0x0193f2c4_7a1b_7c2d_8e3f_4a5b6c7d8e9f);

    fn tema(lado: Lado, evento: Evento) -> Tema {
        Tema { cliente: CLIENTE, lado, evento }
    }

    fn hora(t: &str) -> chrono::DateTime<chrono::FixedOffset> {
        DateTime::parse_from_rfc3339(&format!("2026-11-19T{t}-05:00")).unwrap()
    }

    fn adjunto(cliente: Uuid) -> Adjunto {
        Adjunto {
            nombre: "tesis.docx".into(),
            mime: "application/msword".into(),
            bytes: 10,
            llave: format!("{cliente}/{ID}/tesis.docx"),
            paginas: Some(3),
            pdf: Some(format!("{cliente}/{ID}/tesis.pdf")),
            miniatura: None,
        }
    }

    fn mensaje(tipo: Tipo, texto: Option<&str>) -> Mensaje {
        Mensaje {
            v: 1,
            id: ID,
            cliente_id: CLIENTE,
            tipo,
            texto: texto.map(Into::into),
            adjuntos: vec![],
            responde_a: None,
            referencia: None,
            encargo_id: None,
            fecha: hora("17:42:00"),
        }
    }

    fn guardado(autor: Lado) -> Guardado {
        Guardado {
            cliente: CLIENTE,
            autor,
            fecha: hora_tienda(hora("17:42:00")),
            eliminado: false,
            adjuntos: vec![adjunto(CLIENTE)],
            secuencia: Some(7),
        }
    }

    fn aplicar_sin_guardado(lado: Lado, m: Mensaje) -> Cambio {
        aplicar(&tema(lado, Evento::Mensaje), Entrante::Mensaje(m), None, 42, &Ajustes::default())
    }

    #[test]
    fn un_mensaje_nuevo_se_guarda_con_el_autor_del_tema() {
        let Cambio::Guardar(n) = aplicar_sin_guardado(Lado::Cliente, mensaje(Tipo::Texto, Some(" Hola "))) else {
            panic!("se esperaba Guardar");
        };
        assert_eq!((n.autor, n.secuencia, n.cliente), (Lado::Cliente, 42, CLIENTE));
        assert_eq!(n.texto.as_deref(), Some(" Hola ")); // el texto se guarda como lo escribió
        assert_eq!(n.fecha.to_string(), "2026-11-19 17:42:00");
    }

    #[test]
    fn un_mensaje_repetido_se_ignora() {
        let g = guardado(Lado::Cliente);
        let cambio = aplicar(&tema(Lado::Cliente, Evento::Mensaje), Entrante::Mensaje(mensaje(Tipo::Texto, Some("Hola"))), Some(&g), 43, &Ajustes::default());
        assert_eq!(cambio, Cambio::Ignorar("repetido: ya estaba guardado"));
    }

    #[test]
    fn el_cliente_del_mensaje_tiene_que_ser_el_del_tema() {
        let mut m = mensaje(Tipo::Texto, Some("Hola"));
        m.cliente_id = OTRO;
        assert!(matches!(aplicar_sin_guardado(Lado::Cliente, m), Cambio::Ignorar(_)));
    }

    #[test]
    fn mensajes_vacios_o_incompletos() {
        assert!(matches!(aplicar_sin_guardado(Lado::Cliente, mensaje(Tipo::Texto, None)), Cambio::Ignorar(_)));
        assert!(matches!(aplicar_sin_guardado(Lado::Cliente, mensaje(Tipo::Texto, Some("   "))), Cambio::Ignorar(_)));
        assert!(matches!(aplicar_sin_guardado(Lado::Papeleria, mensaje(Tipo::Producto, None)), Cambio::Ignorar(_)));
        // Solo archivos, sin texto: sí vale.
        let mut m = mensaje(Tipo::Texto, None);
        m.adjuntos = vec![adjunto(CLIENTE)];
        assert!(matches!(aplicar_sin_guardado(Lado::Cliente, m), Cambio::Guardar(_)));
        // Tarjeta con su ref.
        let mut m = mensaje(Tipo::Recibo, None);
        m.referencia = Some("923b53eb-d3bc-43c5-8d7f-e75a45153520".into());
        assert!(matches!(aplicar_sin_guardado(Lado::Papeleria, m), Cambio::Guardar(_)));
    }

    #[test]
    fn los_avisos_solo_los_manda_xplaya() {
        assert!(matches!(aplicar_sin_guardado(Lado::Papeleria, mensaje(Tipo::Aviso, Some("Ya está tu pedido"))), Cambio::Ignorar(_)));
        assert!(matches!(aplicar_sin_guardado(Lado::Automatico, mensaje(Tipo::Aviso, Some("Ya está tu pedido"))), Cambio::Guardar(_)));
    }

    #[test]
    fn un_archivo_de_otro_cliente_no_se_acepta() {
        let mut m = mensaje(Tipo::Texto, Some("mira"));
        m.adjuntos = vec![adjunto(OTRO)];
        assert_eq!(aplicar_sin_guardado(Lado::Cliente, m), Cambio::Ignorar("un archivo no está en la carpeta del cliente"));
        // Tampoco con el PDF convertido apuntando a otro lado.
        let mut a = adjunto(CLIENTE);
        a.pdf = Some(format!("{OTRO}/x.pdf"));
        let mut m = mensaje(Tipo::Texto, Some("mira"));
        m.adjuntos = vec![a];
        assert!(matches!(aplicar_sin_guardado(Lado::Cliente, m), Cambio::Ignorar(_)));
    }

    #[test]
    fn leido_marca_lo_del_otro_lado() {
        let l = Leido { v: 1, cliente_id: CLIENTE, hasta: hora("17:45:00") };
        let leyo_la_papeleria = aplicar(&tema(Lado::Papeleria, Evento::Leido), Entrante::Leido(l.clone()), None, 1, &Ajustes::default());
        assert_eq!(
            leyo_la_papeleria,
            Cambio::MarcarLeidos { cliente: CLIENTE, del_cliente: true, hasta: hora_tienda(hora("17:45:00")) }
        );
        let leyo_el_cliente = aplicar(&tema(Lado::Cliente, Evento::Leido), Entrante::Leido(l.clone()), None, 1, &Ajustes::default());
        assert!(matches!(leyo_el_cliente, Cambio::MarcarLeidos { del_cliente: false, .. }));
        let asistente = aplicar(&tema(Lado::Asistente, Evento::Leido), Entrante::Leido(l), None, 1, &Ajustes::default());
        assert!(matches!(asistente, Cambio::Ignorar(_)));
    }

    fn editar(lado: Lado, a_las: &str, g: Option<&Guardado>) -> Cambio {
        let e = Editado { v: 1, cliente_id: CLIENTE, id: ID, texto: "Son 2 juegos".into(), fecha: hora(a_las) };
        aplicar(&tema(lado, Evento::Editado), Entrante::Editado(e), g, 1, &Ajustes::default())
    }

    #[test]
    fn editar_dentro_de_los_15_minutos() {
        let g = guardado(Lado::Cliente);
        assert!(matches!(editar(Lado::Cliente, "17:57:00", Some(&g)), Cambio::Editar { .. })); // justo 15 min
        assert_eq!(editar(Lado::Cliente, "17:58:00", Some(&g)), Cambio::Ignorar("editar: ya pasó el tiempo para editar"));
    }

    #[test]
    fn el_tiempo_para_editar_sale_de_los_ajustes() {
        let g = guardado(Lado::Cliente);
        let e = Editado { v: 1, cliente_id: CLIENTE, id: ID, texto: "x".into(), fecha: hora("18:10:00") };
        let cambio = aplicar(&tema(Lado::Cliente, Evento::Editado), Entrante::Editado(e), Some(&g), 1, &Ajustes { minutos_editar: 30 });
        assert!(matches!(cambio, Cambio::Editar { .. }));
    }

    #[test]
    fn solo_se_edita_lo_propio_y_lo_que_existe() {
        let g = guardado(Lado::Cliente);
        assert_eq!(editar(Lado::Papeleria, "17:43:00", Some(&g)), Cambio::Ignorar("el mensaje es del otro lado"));
        assert_eq!(editar(Lado::Cliente, "17:43:00", None), Cambio::Ignorar("editar: el mensaje no existe"));
        let mut de_otro = guardado(Lado::Cliente);
        de_otro.cliente = OTRO;
        assert_eq!(editar(Lado::Cliente, "17:43:00", Some(&de_otro)), Cambio::Ignorar("el mensaje es de otro cliente"));
        let mut borrado = guardado(Lado::Cliente);
        borrado.eliminado = true;
        assert!(matches!(editar(Lado::Cliente, "17:43:00", Some(&borrado)), Cambio::Ignorar(_)));
    }

    fn eliminar(lado: Lado, g: Option<&Guardado>) -> Cambio {
        let e = Eliminado { v: 1, cliente_id: CLIENTE, id: ID, fecha: hora("20:00:00") };
        aplicar(&tema(lado, Evento::Eliminado), Entrante::Eliminado(e), g, 1, &Ajustes::default())
    }

    #[test]
    fn eliminar_sin_limite_de_tiempo_y_con_sus_archivos() {
        let g = guardado(Lado::Papeleria);
        let Cambio::Eliminar { id, llaves, secuencia, .. } = eliminar(Lado::Papeleria, Some(&g)) else {
            panic!("se esperaba Eliminar");
        };
        assert_eq!((id, secuencia), (ID, Some(7)));
        assert_eq!(llaves, vec![format!("{CLIENTE}/{ID}/tesis.docx"), format!("{CLIENTE}/{ID}/tesis.pdf")]);
    }

    #[test]
    fn solo_se_elimina_lo_propio_una_vez() {
        let g = guardado(Lado::Papeleria);
        assert_eq!(eliminar(Lado::Cliente, Some(&g)), Cambio::Ignorar("el mensaje es del otro lado"));
        assert!(matches!(eliminar(Lado::Papeleria, None), Cambio::Ignorar(_)));
        let mut ya = g.clone();
        ya.eliminado = true;
        assert_eq!(eliminar(Lado::Papeleria, Some(&ya)), Cambio::Ignorar("eliminar: ya estaba eliminado"));
    }
}
