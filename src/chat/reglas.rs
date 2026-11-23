//! Qué hacer con cada evento del stream `CHAT` (`CHAT.md`, "Reglas del código").
//!
//! Todo es puro: recibe el evento, lo que ya está guardado de ese mensaje y los ajustes, y
//! devuelve un `Cambio` sin ejecutarlo. Las horas que cuentan son las del evento (las pone quien
//! publica, xplaya o el POS), no la de cuando se procesa: si xplaya estuvo apagado un rato, una
//! edición hecha a tiempo sigue valiendo.

use chrono::{DateTime, FixedOffset, NaiveDateTime};
use uuid::Uuid;

use super::contrato::{hora_tienda, Adjunto, Editado, Eliminado, Entrante, Evento, Lado, Leido, Mensaje, Tema, Tipo, VERSION};

/// Los ajustes del chat que usan las reglas (tabla `Settings`, claves `CHAT_*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ajustes {
    pub minutos_editar: i64,
    /// `CHAT_CLIENTES_PRUEBA`: quién ve el chat en la app mientras la caja no puede contestar
    /// (`chat_visible`). `None`: no está la fila.
    pub clientes_prueba: Option<String>,
    /// `CHAT_MB_DOCUMENTO`: todo lo que no es video (`archivos::limite`).
    pub mb_documento: u64,
    /// `CHAT_MB_VIDEO`, con tope de `archivos::MB_VIDEO_TOPE`.
    pub mb_video: u64,
    /// `PAPELERIA_ABIERTA`: el switch de la caja. Lo enseña el encabezado del chat. Sin la fila
    /// o con un valor que no sea `0`: abierta.
    pub papeleria_abierta: bool,
    /// `PAPELERIA_ESTADO`: texto libre que la caja pone como estado, como en WhatsApp ("Hoy
    /// cerramos a las 6"). Sale en el encabezado del chat en lugar de "Contestamos pronto/al abrir".
    /// `None` si no está o está vacío.
    pub estado: Option<String>,
    /// `CHAT_MINUTOS_AVISO`: minutos que un mensaje del cliente espera sin leerse en la caja antes
    /// del aviso por ntfy al celular de la papelería (`aviso_a_la_papeleria`). `0`: sin aviso.
    pub minutos_aviso: u64,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self { minutos_editar: 15, clientes_prueba: None, mb_documento: 25, mb_video: 100, papeleria_abierta: true, estado: None, minutos_aviso: 5 }
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
    /// Pedido (`Pedidos.Uid`) al que la papelería ligó este mensaje (etapa 7). Solo de la
    /// papelería o de xplaya: si lo manda el cliente, se descarta.
    pub encargo: Option<Uuid>,
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
    /// Marca entregados (✓✓ grises) los mensajes del cliente hasta esa hora: ya están en una
    /// pantalla de la caja.
    MarcarEntregados { cliente: Uuid, hasta: NaiveDateTime },
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
        Entrante::Entregado(l) => entregado(tema, l),
        Entrante::Editado(e) => editado(tema, e, guardado, ajustes),
        Entrante::Eliminado(e) => eliminado(tema, e, guardado),
        Entrante::Sugerencia => Cambio::Ignorar("las sugerencias del asistente son para después (CHAT.md, etapa 8)"),
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
    if m.adjuntos.len() > MAX_ADJUNTOS {
        return Cambio::Ignorar("demasiados archivos en un mensaje");
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
        encargo: m.encargo_id.filter(|_| matches!(tema.lado, Lado::Papeleria | Lado::Automatico)),
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

/// Solo la papelería dice que le llegó lo del cliente (CHAT.md, etapa 9): lo de la caja no lleva
/// "entregado".
fn entregado(tema: &Tema, l: Leido) -> Cambio {
    if l.cliente_id != tema.cliente {
        return Cambio::Ignorar("el cliente del entregado no es el del tema");
    }
    if tema.lado != Lado::Papeleria {
        return Cambio::Ignorar("solo la papelería marca entregado");
    }
    Cambio::MarcarEntregados { cliente: l.cliente_id, hasta: hora_tienda(l.hasta) }
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

// ── Lo que hace el cliente desde la app ──────────────────────────────────────
//
// Editar y eliminar desde la app no tienen reglas aparte: la ruta arma el evento y le pregunta a
// `aplicar` con lo guardado, igual que hará el consumer cuando llegue del stream.

/// Más que esto no es una plática: se rechaza en lugar de cortarlo.
pub const MAX_CARACTERES: usize = 4000;
/// Archivos en un solo mensaje ("ahí te van estas 60 fotos"). Lo revisan la app y el consumer.
pub const MAX_ADJUNTOS: usize = 100;

/// Lo que el cliente quiere mandar desde la app.
#[derive(Debug, Clone, Default)]
pub struct Borrador {
    /// Lo genera el celular: así un reintento sin señal no se guarda dos veces.
    pub id: Uuid,
    pub texto: String,
    pub responde_a: Option<Uuid>,
    /// Los arma xplaya al subir cada archivo (`POST /chat/archivo`), nunca el celular.
    pub adjuntos: Vec<Adjunto>,
    /// Tarjeta de producto, recibo o cotización, con su `ref`.
    pub tarjeta: Option<(Tipo, String)>,
}

/// El mensaje que el cliente escribió en la app, listo para publicar en
/// `chat.{cliente}.cliente.mensaje`. La fecha la pone xplaya.
pub fn del_cliente(cliente: Uuid, b: Borrador, bloqueado: bool, ahora: DateTime<FixedOffset>) -> Result<Mensaje, &'static str> {
    if bloqueado {
        return Err("La papelería bloqueó tu chat.");
    }
    if b.texto.chars().count() > MAX_CARACTERES {
        return Err("El mensaje es muy largo.");
    }
    if b.adjuntos.len() > MAX_ADJUNTOS {
        return Err("Son demasiados archivos para un mensaje (máximo 100).");
    }
    let (tipo, referencia) = match b.tarjeta {
        None => (Tipo::Texto, None),
        Some((tipo @ (Tipo::Producto | Tipo::Recibo | Tipo::Cotizacion), r)) if !r.trim().is_empty() => (tipo, Some(r)),
        Some(_) => return Err("Esa tarjeta no se puede mandar."),
    };
    let texto = Some(b.texto).filter(|t| !t.trim().is_empty());
    if texto.is_none() && b.adjuntos.is_empty() && referencia.is_none() {
        return Err("Escribe un mensaje.");
    }
    Ok(Mensaje {
        v: VERSION,
        id: b.id,
        cliente_id: cliente,
        tipo,
        texto,
        adjuntos: b.adjuntos,
        responde_a: b.responde_a,
        referencia,
        encargo_id: None,
        fecha: ahora,
    })
}

/// Quién ve el chat en la app (`CHAT_CLIENTES_PRUEBA`): sin la fila, nadie; vacía, todos; si no,
/// los ids separados por coma.
pub fn chat_visible(clientes_prueba: Option<&str>, cliente: Uuid) -> bool {
    match clientes_prueba.map(str::trim) {
        None => false,
        Some("") => true,
        Some(lista) => lista.split(',').any(|id| id.trim().parse::<Uuid>().ok() == Some(cliente)),
    }
}

// ── Papelería cerrada ───────────────────────────────────────────────────────

pub const TEXTO_FUERA_DE_HORARIO: &str = "¡Hola! Ahorita estamos cerrados. Nuestro horario es de lunes a viernes de 6:30 a 18:00 y \
sábados de 8:00 a 14:00. Te contestamos en cuanto abramos.";

/// Para que el id del aviso salga igual cada vez (`aviso_fuera_de_horario`).
const ESPACIO_AVISOS: Uuid = Uuid::from_u128(0x5c1d7e0a_2b3f_4a6c_9d8e_7f6a5b4c3d2e);

/// La respuesta automática a un mensaje del cliente escrito a la hora `ahora` mientras la
/// papelería está cerrada. `cierre` es cuándo se apagó el switch de la caja
/// (`db::chat::cierre_de_la_papeleria`); sin cierre, no hay aviso. Sale un segundo después, para
/// quedar debajo. El id sale del cliente y del cierre: es el mismo para todos los mensajes de
/// ese cierre, así que basta ver si ya está guardado para contestar una sola vez.
pub fn aviso_fuera_de_horario(cliente: Uuid, cierre: Option<NaiveDateTime>, ahora: DateTime<FixedOffset>) -> Option<Mensaje> {
    let cierre = cierre?;
    let id = Uuid::new_v5(&ESPACIO_AVISOS, format!("fuera-de-horario/{cliente}/{cierre}").as_bytes());
    Some(Mensaje {
        v: VERSION,
        id,
        cliente_id: cliente,
        tipo: Tipo::Aviso,
        texto: Some(TEXTO_FUERA_DE_HORARIO.to_string()),
        adjuntos: vec![],
        responde_a: None,
        referencia: None,
        encargo_id: None,
        fecha: ahora + chrono::Duration::seconds(1),
    })
}

/// "Avisar por la app" de un pedido ligado al chat (etapa 7): el mismo aviso, también en la
/// conversación, con la tarjeta del pedido. El id es el de la notificación: si la entrega se
/// reintenta, el mensaje no se repite. Lo publica xplaya como automático, así que no manda otro
/// aviso al celular (`aviso_al_cliente`): el de la notificación basta.
pub fn aviso_de_pedido(notificacion: Uuid, cliente: Uuid, pedido: Uuid, titulo: &str, texto: &str, ahora: DateTime<FixedOffset>) -> Mensaje {
    let texto = [titulo.trim(), texto.trim()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join("\n");
    Mensaje {
        v: VERSION,
        id: notificacion,
        cliente_id: cliente,
        tipo: Tipo::Cotizacion,
        texto: Some(texto).filter(|t| !t.is_empty()),
        adjuntos: vec![],
        responde_a: None,
        referencia: Some(pedido.to_string()),
        encargo_id: Some(pedido),
        fecha: ahora,
    }
}

/// Largo máximo del texto en el aviso del celular (la notificación corta lo demás de todos modos).
const AVISO_MAX: usize = 120;

/// Lo que dice el aviso del celular cuando la papelería escribe y el cliente no tiene la app
/// abierta (etapa 6, Web Push). Solo de lo que escribe la papelería: lo automático sale justo
/// después de que el cliente escribió, y los avisos de "Avisar por la app" ya traen su propio
/// aviso (`push::entregar`). Como en WhatsApp: el texto, o qué mandó si no trae texto.
pub fn aviso_al_cliente(n: &Nuevo) -> Option<String> {
    if n.autor != Lado::Papeleria {
        return None;
    }
    let que = match n.adjuntos.as_slice() {
        [] => None,
        [a] => Some(match a.mime.split('/').next() {
            _ if a.pdf.is_some() || a.mime == "application/pdf" => "📄 Documento",
            Some("image") => "📷 Foto",
            Some("video") => "🎥 Video",
            Some("audio") => "🎤 Nota de voz",
            _ => "📎 Archivo",
        }.to_string()),
        varios => Some(format!("📎 {} archivos", varios.len())),
    };
    let texto = n.texto.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(|t| {
        // Un solo renglón, y cortado por caracteres (no por bytes: hay acentos y emoji).
        let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.chars().count() > AVISO_MAX {
            format!("{}…", t.chars().take(AVISO_MAX).collect::<String>().trim_end())
        } else {
            t
        }
    });
    let tarjeta = match n.tipo {
        Tipo::Producto => Some("🛍️ Te mandó un producto"),
        Tipo::Recibo => Some("🧾 Te mandó tu recibo"),
        Tipo::Cotizacion => Some("🧾 Te mandó una cotización"),
        Tipo::Texto | Tipo::Aviso => None,
    };
    match (que, texto) {
        (Some(q), Some(t)) => Some(format!("{} {t}", q.split_once(' ').map_or(q.as_str(), |(icono, _)| icono))),
        (Some(q), None) => Some(q),
        (None, Some(t)) => Some(t),
        (None, None) => tarjeta.map(str::to_string),
    }
}

/// Un cliente con mensajes que la caja no ha leído (`db::chat::sin_contestar`).
#[derive(Debug, Clone, PartialEq)]
pub struct SinContestar {
    pub cliente: Uuid,
    /// `NombreChat` si lo puso en Mi perfil; si no, el de `Clientes`.
    pub nombre: String,
    pub cuantos: i64,
    /// El más reciente: su texto y si trae archivos.
    pub ultimo_texto: Option<String>,
    pub ultimo_con_archivos: bool,
}

/// Qué mensajes avisar en esta revisión: los que su racha sin leer empezó entre `(antes, ahora]`
/// menos `minutos`. La revisión corre cada minuto; con la ventana desde la revisión anterior,
/// cada racha cae en una sola vuelta, aunque una se atrase. `None`: avisos apagados (`0`).
pub fn ventana_de_aviso(antes: NaiveDateTime, ahora: NaiveDateTime, minutos: u64) -> Option<(NaiveDateTime, NaiveDateTime)> {
    if minutos == 0 || ahora <= antes {
        return None;
    }
    let espera = chrono::Duration::minutes(minutos as i64);
    Some((antes - espera, ahora - espera))
}

/// Título y texto del aviso por ntfy al celular de la papelería: "Chat sin contestar: Ana" ·
/// "¿Tienen opalina? · 3 sin leer, el primero hace 5 min".
pub fn aviso_a_la_papeleria(s: &SinContestar, minutos: u64) -> (String, String) {
    let primer_nombre = s.nombre.split_whitespace().next().unwrap_or("un cliente");
    let ultimo = s.ultimo_texto.as_deref().map(str::trim).filter(|t| !t.is_empty())
        .map(|t| {
            let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
            if t.chars().count() > AVISO_MAX {
                format!("{}…", t.chars().take(AVISO_MAX).collect::<String>().trim_end())
            } else {
                t
            }
        })
        .unwrap_or_else(|| if s.ultimo_con_archivos { "📎 Mandó archivos".into() } else { "Mensaje nuevo".into() });
    let cuantos = if s.cuantos == 1 { "1 sin leer".to_string() } else { format!("{} sin leer", s.cuantos) };
    (
        format!("Chat sin contestar: {primer_nombre}"),
        format!("{ultimo}\n{cuantos}, el primero hace {minutos} min"),
    )
}

/// La cotización del carrito en el chat (`routes::chat::cotizacion_del_carrito`): al mandar su
/// pedido desde la app, el cliente lo ve como tarjeta. El id sale del pedido, así que si se
/// publica dos veces se guarda una sola.
pub fn cotizacion_del_carrito(cliente: Uuid, pedido: Uuid, ahora: DateTime<FixedOffset>) -> Mensaje {
    Mensaje {
        v: VERSION,
        id: Uuid::new_v5(&ESPACIO_AVISOS, format!("cotizacion/{pedido}").as_bytes()),
        cliente_id: cliente,
        tipo: Tipo::Cotizacion,
        texto: Some("Recibimos tu pedido.".to_string()),
        adjuntos: vec![],
        responde_a: None,
        referencia: Some(pedido.to_string()),
        encargo_id: None,
        fecha: ahora,
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

    #[test]
    fn entregado_solo_lo_marca_la_papeleria() {
        let l = Leido { v: 1, cliente_id: CLIENTE, hasta: hora("17:45:00") };
        let entregar = |lado| aplicar(&tema(lado, Evento::Entregado), Entrante::Entregado(l.clone()), None, 1, &Ajustes::default());
        assert_eq!(
            entregar(Lado::Papeleria),
            Cambio::MarcarEntregados { cliente: CLIENTE, hasta: hora_tienda(hora("17:45:00")) }
        );
        for lado in [Lado::Cliente, Lado::Automatico, Lado::Asistente] {
            assert!(matches!(entregar(lado), Cambio::Ignorar(_)), "{lado:?}");
        }
        let de_otro = Leido { cliente_id: OTRO, ..l.clone() };
        let ajeno = aplicar(&tema(Lado::Papeleria, Evento::Entregado), Entrante::Entregado(de_otro), None, 1, &Ajustes::default());
        assert!(matches!(ajeno, Cambio::Ignorar(_)));
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
        let cambio = aplicar(&tema(Lado::Cliente, Evento::Editado), Entrante::Editado(e), Some(&g), 1, &Ajustes { minutos_editar: 30, ..Ajustes::default() });
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

    // ── App ──

    fn texto(t: &str) -> Borrador {
        Borrador { id: ID, texto: t.to_string(), ..Borrador::default() }
    }

    fn foto(n: u128) -> Adjunto {
        let llave = format!("{CLIENTE}/{ID}/{}.jpg", Uuid::from_u128(n));
        Adjunto { nombre: "foto.jpg".into(), mime: "image/jpeg".into(), bytes: 1, llave, paginas: None, pdf: None, miniatura: None }
    }

    #[test]
    fn el_cliente_no_manda_vacio_largo_ni_bloqueado() {
        let ahora = hora("17:42:00");
        let m = del_cliente(CLIENTE, Borrador { responde_a: Some(OTRO), ..texto(" Hola ") }, false, ahora).unwrap();
        assert_eq!((m.v, m.tipo, m.texto.as_deref(), m.responde_a, m.fecha), (1, Tipo::Texto, Some(" Hola "), Some(OTRO), ahora));
        assert!(del_cliente(CLIENTE, texto("  \n "), false, ahora).is_err());
        assert!(del_cliente(CLIENTE, texto(&"a".repeat(MAX_CARACTERES)), false, ahora).is_ok());
        assert!(del_cliente(CLIENTE, texto(&"ñ".repeat(MAX_CARACTERES + 1)), false, ahora).is_err());
        assert_eq!(del_cliente(CLIENTE, texto("Hola"), true, ahora), Err("La papelería bloqueó tu chat."));
        // Lo que sale de aquí lo acepta el consumer al volver del stream.
        let m = del_cliente(CLIENTE, texto("Hola"), false, ahora).unwrap();
        assert!(matches!(aplicar_sin_guardado(Lado::Cliente, m), Cambio::Guardar(_)));
    }

    #[test]
    fn el_cliente_manda_fotos_sin_texto_hasta_cien() {
        let ahora = hora("17:42:00");
        let fotos = Borrador { adjuntos: (0..60).map(foto).collect(), ..texto("") };
        let m = del_cliente(CLIENTE, fotos, false, ahora).unwrap();
        assert_eq!((m.tipo, m.texto.as_deref(), m.adjuntos.len()), (Tipo::Texto, None, 60));
        assert!(matches!(aplicar_sin_guardado(Lado::Cliente, m.clone()), Cambio::Guardar(_)));

        let muchas = Borrador { adjuntos: (0..=MAX_ADJUNTOS as u128).map(foto).collect(), ..texto("") };
        assert!(del_cliente(CLIENTE, muchas, false, ahora).is_err());
        // Y si alguien las publica directo en NATS, el consumer tampoco las guarda.
        let mut m = m;
        m.adjuntos = (0..=MAX_ADJUNTOS as u128).map(foto).collect();
        assert_eq!(aplicar_sin_guardado(Lado::Cliente, m), Cambio::Ignorar("demasiados archivos en un mensaje"));
    }

    #[test]
    fn el_cliente_manda_tarjetas_pero_no_avisos() {
        let ahora = hora("17:42:00");
        let producto = Borrador { tarjeta: Some((Tipo::Producto, "1234".into())), ..texto("") };
        let m = del_cliente(CLIENTE, producto, false, ahora).unwrap();
        assert_eq!((m.tipo, m.referencia.as_deref(), m.texto.as_deref()), (Tipo::Producto, Some("1234"), None));
        assert!(matches!(aplicar_sin_guardado(Lado::Cliente, m), Cambio::Guardar(_)));

        let aviso = Borrador { tarjeta: Some((Tipo::Aviso, "x".into())), ..texto("Hola") };
        assert!(del_cliente(CLIENTE, aviso, false, ahora).is_err());
        let sin_ref = Borrador { tarjeta: Some((Tipo::Recibo, " ".into())), ..texto("Hola") };
        assert!(del_cliente(CLIENTE, sin_ref, false, ahora).is_err());
    }

    #[test]
    fn quien_ve_el_chat() {
        assert!(!chat_visible(None, CLIENTE), "sin la fila, nadie");
        assert!(chat_visible(Some(" "), CLIENTE), "vacía, todos");
        assert!(chat_visible(Some(&format!("{OTRO}, {CLIENTE}")), CLIENTE));
        assert!(!chat_visible(Some(&OTRO.to_string()), CLIENTE));
        assert!(!chat_visible(Some("basura"), CLIENTE));
    }

    fn el(fecha: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(fecha, "%Y-%m-%d %H:%M").unwrap()
    }

    fn a_las(fecha: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(&format!("{fecha}:00-05:00")).unwrap()
    }

    #[test]
    fn un_aviso_por_cierre() {
        let ayer = Some(el("2026-11-16 18:00"));
        let hoy = Some(el("2026-11-17 18:05"));
        assert_eq!(aviso_fuera_de_horario(CLIENTE, None, a_las("2026-11-16T12:00")), None, "abierta: nada");
        let noche = aviso_fuera_de_horario(CLIENTE, ayer, a_las("2026-11-16T20:00")).unwrap();
        let madrugada = aviso_fuera_de_horario(CLIENTE, ayer, a_las("2026-11-17T05:00")).unwrap();
        let siguiente = aviso_fuera_de_horario(CLIENTE, hoy, a_las("2026-11-17T19:00")).unwrap();
        assert_eq!(noche.id, madrugada.id, "mismo cierre, mismo aviso");
        assert_ne!(noche.id, siguiente.id);
        assert_ne!(noche.id, aviso_fuera_de_horario(OTRO, ayer, a_las("2026-11-16T20:00")).unwrap().id);
        assert_eq!((noche.tipo, noche.fecha), (Tipo::Aviso, a_las("2026-11-16T20:00") + chrono::Duration::seconds(1)));
        // El consumer lo acepta del lado automático (y de ningún otro).
        let t = |lado| Tema { cliente: CLIENTE, lado, evento: Evento::Mensaje };
        let ajustes = Ajustes::default();
        assert!(matches!(aplicar(&t(Lado::Automatico), Entrante::Mensaje(noche.clone()), None, 1, &ajustes), Cambio::Guardar(_)));
        assert!(matches!(aplicar(&t(Lado::Cliente), Entrante::Mensaje(noche), None, 1, &ajustes), Cambio::Ignorar(_)));
    }

    #[test]
    fn una_cotizacion_por_pedido() {
        let ahora = a_las("2026-11-19T12:00");
        let c = cotizacion_del_carrito(CLIENTE, ID, ahora);
        assert_eq!(c.id, cotizacion_del_carrito(CLIENTE, ID, ahora + chrono::Duration::seconds(5)).id);
        assert_ne!(c.id, cotizacion_del_carrito(CLIENTE, OTRO, ahora).id);
        assert_eq!((c.tipo, c.referencia.as_deref()), (Tipo::Cotizacion, Some(ID.to_string().as_str())));
        let t = Tema { cliente: CLIENTE, lado: Lado::Automatico, evento: Evento::Mensaje };
        assert!(matches!(aplicar(&t, Entrante::Mensaje(c), None, 1, &Ajustes::default()), Cambio::Guardar(_)));
    }

    #[test]
    fn el_encargo_solo_lo_liga_la_papeleria() {
        let t = |lado| Tema { cliente: CLIENTE, lado, evento: Evento::Mensaje };
        let mut m = cotizacion_del_carrito(CLIENTE, ID, a_las("2026-11-21T12:00"));
        m.encargo_id = Some(ID);
        let encargo = |lado| match aplicar(&t(lado), Entrante::Mensaje(m.clone()), None, 1, &Ajustes::default()) {
            Cambio::Guardar(n) => n.encargo,
            otro => panic!("{otro:?}"),
        };
        assert_eq!(encargo(Lado::Papeleria), Some(ID));
        assert_eq!(encargo(Lado::Automatico), Some(ID));
        assert_eq!(encargo(Lado::Cliente), None, "el cliente no liga pedidos");
    }

    #[test]
    fn el_aviso_del_pedido_en_el_chat() {
        let ahora = a_las("2026-11-21T12:00");
        let m = aviso_de_pedido(OTRO, CLIENTE, ID, "¡Tu pedido está listo! 🎉", "Pedido No. 7 · Total: $50.00\n", ahora);
        assert_eq!(m.id, OTRO, "el id de la notificación: no se repite al reintentar");
        assert_eq!((m.tipo, m.referencia.as_deref(), m.encargo_id), (Tipo::Cotizacion, Some(ID.to_string().as_str()), Some(ID)));
        assert_eq!(m.texto.as_deref(), Some("¡Tu pedido está listo! 🎉\nPedido No. 7 · Total: $50.00"));
        let t = Tema { cliente: CLIENTE, lado: Lado::Automatico, evento: Evento::Mensaje };
        match aplicar(&t, Entrante::Mensaje(m), None, 1, &Ajustes::default()) {
            Cambio::Guardar(n) => assert_eq!(aviso_al_cliente(&n), None, "sin segundo aviso al celular"),
            otro => panic!("{otro:?}"),
        }
    }

    fn de_la_papeleria(texto: Option<&str>, adjuntos: Vec<Adjunto>) -> Nuevo {
        Nuevo {
            id: ID, cliente: CLIENTE, autor: Lado::Papeleria, tipo: Tipo::Texto,
            texto: texto.map(str::to_string), adjuntos, referencia: None, responde_a: None, encargo: None,
            secuencia: 1, fecha: hora("10:00:00").naive_local(),
        }
    }

    fn con_mime(mime: &str) -> Adjunto {
        Adjunto { mime: mime.into(), pdf: None, ..adjunto(CLIENTE) }
    }

    #[test]
    fn aviso_al_cliente_solo_de_la_papeleria() {
        let mut n = de_la_papeleria(Some("Ya están tus copias"), vec![]);
        assert_eq!(aviso_al_cliente(&n).as_deref(), Some("Ya están tus copias"));
        n.autor = Lado::Cliente;
        assert_eq!(aviso_al_cliente(&n), None);
        n.autor = Lado::Automatico;
        assert_eq!(aviso_al_cliente(&n), None);
    }

    #[test]
    fn aviso_al_cliente_en_un_renglon_y_cortado() {
        let n = de_la_papeleria(Some("  Hola\n\nya   quedó  "), vec![]);
        assert_eq!(aviso_al_cliente(&n).as_deref(), Some("Hola ya quedó"));
        let largo = "ñ".repeat(200);
        let aviso = aviso_al_cliente(&de_la_papeleria(Some(&largo), vec![])).unwrap();
        assert_eq!(aviso.chars().count(), AVISO_MAX + 1);
        assert!(aviso.ends_with('…'));
    }

    #[test]
    fn aviso_al_cliente_dice_que_mando() {
        let solo = |a: Adjunto| aviso_al_cliente(&de_la_papeleria(None, vec![a])).unwrap();
        assert_eq!(solo(con_mime("image/jpeg")), "📷 Foto");
        assert_eq!(solo(con_mime("video/mp4")), "🎥 Video");
        assert_eq!(solo(con_mime("audio/webm")), "🎤 Nota de voz");
        assert_eq!(solo(con_mime("application/pdf")), "📄 Documento");
        assert_eq!(solo(adjunto(CLIENTE)), "📄 Documento"); // Word ya convertido
        assert_eq!(solo(con_mime("application/zip")), "📎 Archivo");
        let varios = de_la_papeleria(None, vec![con_mime("image/png"), con_mime("image/png")]);
        assert_eq!(aviso_al_cliente(&varios).as_deref(), Some("📎 2 archivos"));
        // Con texto: el ícono y el texto, como en WhatsApp.
        let foto = de_la_papeleria(Some("Así quedó"), vec![con_mime("image/png")]);
        assert_eq!(aviso_al_cliente(&foto).as_deref(), Some("📷 Así quedó"));
    }

    #[test]
    fn aviso_al_cliente_de_una_tarjeta() {
        let mut n = de_la_papeleria(None, vec![]);
        assert_eq!(aviso_al_cliente(&n), None);
        n.tipo = Tipo::Producto;
        assert_eq!(aviso_al_cliente(&n).as_deref(), Some("🛍️ Te mandó un producto"));
        n.texto = Some("¿Este?".into());
        assert_eq!(aviso_al_cliente(&n).as_deref(), Some("¿Este?"));
    }

    fn sin_contestar(texto: Option<&str>, archivos: bool, cuantos: i64) -> SinContestar {
        SinContestar {
            cliente: CLIENTE, nombre: "Ana López".into(), cuantos,
            ultimo_texto: texto.map(str::to_string), ultimo_con_archivos: archivos,
        }
    }

    #[test]
    fn aviso_a_la_papeleria_con_texto_y_cuantos() {
        let (titulo, texto) = aviso_a_la_papeleria(&sin_contestar(Some("¿Tienen\nopalina?"), false, 3), 5);
        assert_eq!(titulo, "Chat sin contestar: Ana");
        assert_eq!(texto, "¿Tienen opalina?\n3 sin leer, el primero hace 5 min");
        let (_, texto) = aviso_a_la_papeleria(&sin_contestar(None, true, 1), 5);
        assert_eq!(texto, "📎 Mandó archivos\n1 sin leer, el primero hace 5 min");
    }

    #[test]
    fn ventana_de_aviso_cubre_desde_la_revision_anterior() {
        let h = |t: &str| hora(t).naive_local();
        assert_eq!(ventana_de_aviso(h("10:00:00"), h("10:01:00"), 5), Some((h("09:55:00"), h("09:56:00"))));
        // Si una vuelta se atrasó, la siguiente cubre todo lo que faltó.
        assert_eq!(ventana_de_aviso(h("10:00:00"), h("10:03:30"), 5), Some((h("09:55:00"), h("09:58:30"))));
        assert_eq!(ventana_de_aviso(h("10:00:00"), h("10:01:00"), 0), None);
        assert_eq!(ventana_de_aviso(h("10:01:00"), h("10:01:00"), 5), None);
    }
}
