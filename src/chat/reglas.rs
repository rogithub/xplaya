//! Qué hacer con cada evento del stream `CHAT` (`CHAT.md`, "Reglas del código").
//!
//! Todo es puro: recibe el evento, lo que ya está guardado de ese mensaje y los ajustes, y
//! devuelve un `Cambio` sin ejecutarlo. Las horas que cuentan son las del evento (las pone quien
//! publica, xplaya o el POS), no la de cuando se procesa: si xplaya estuvo apagado un rato, una
//! edición hecha a tiempo sigue valiendo.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
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
}

impl Default for Ajustes {
    fn default() -> Self {
        Self { minutos_editar: 15, clientes_prueba: None, mb_documento: 25, mb_video: 100 }
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

// ── Horario de la tienda ─────────────────────────────────────────────────────

/// Apertura y cierre de un día (hora de la tienda). Domingo, cerrado.
fn horario(dia: Weekday) -> Option<(NaiveTime, NaiveTime)> {
    let h = |hora, minuto| NaiveTime::from_hms_opt(hora, minuto, 0).expect("hora válida");
    match dia {
        Weekday::Sun => None,
        Weekday::Sat => Some((h(8, 0), h(14, 0))),
        _ => Some((h(6, 30), h(18, 0))),
    }
}

pub fn abierto(ahora: NaiveDateTime) -> bool {
    horario(ahora.weekday()).is_some_and(|(abre, cierra)| (abre..cierra).contains(&ahora.time()))
}

/// Cuándo cerró la tienda por última vez, si ahora está cerrada.
pub fn inicio_del_cierre(ahora: NaiveDateTime) -> Option<NaiveDateTime> {
    if abierto(ahora) {
        return None;
    }
    let hoy: NaiveDate = ahora.date();
    (0..8)
        .filter_map(|atras| hoy.checked_sub_days(chrono::Days::new(atras)))
        .filter_map(|dia| horario(dia.weekday()).map(|(_, cierra)| dia.and_time(cierra)))
        .find(|cierre| *cierre <= ahora)
}

pub const TEXTO_FUERA_DE_HORARIO: &str = "¡Hola! Ahorita estamos cerrados. Nuestro horario es de lunes a viernes de 6:30 a 18:00 y \
sábados de 8:00 a 14:00. Te contestamos en cuanto abramos.";

/// Para que el id del aviso salga igual cada vez (`aviso_fuera_de_horario`).
const ESPACIO_AVISOS: Uuid = Uuid::from_u128(0x5c1d7e0a_2b3f_4a6c_9d8e_7f6a5b4c3d2e);

/// La respuesta automática a un mensaje del cliente escrito a la hora `ahora` cuando la tienda
/// está cerrada, o `None` si está abierta. Sale un segundo después, para quedar debajo. El id sale del cliente y de la hora del cierre: es el mismo para todos los
/// mensajes de ese cierre, así que basta ver si ya está guardado para contestar una sola vez.
pub fn aviso_fuera_de_horario(cliente: Uuid, ahora: DateTime<FixedOffset>) -> Option<Mensaje> {
    let cierre = inicio_del_cierre(hora_tienda(ahora))?;
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

    #[test]
    fn horario_de_la_tienda() {
        // 2026-11-16 es lunes; 21 sábado; 22 domingo.
        assert!(!abierto(el("2026-11-16 06:29")));
        assert!(abierto(el("2026-11-16 06:30")));
        assert!(abierto(el("2026-11-16 17:59")));
        assert!(!abierto(el("2026-11-16 18:00")));
        assert!(!abierto(el("2026-11-21 07:59")));
        assert!(abierto(el("2026-11-21 08:00")));
        assert!(abierto(el("2026-11-21 13:59")));
        assert!(!abierto(el("2026-11-21 14:00")));
        assert!(!abierto(el("2026-11-22 12:00")));
    }

    #[test]
    fn cuando_cerro() {
        assert_eq!(inicio_del_cierre(el("2026-11-16 12:00")), None);
        assert_eq!(inicio_del_cierre(el("2026-11-16 20:00")), Some(el("2026-11-16 18:00")));
        assert_eq!(inicio_del_cierre(el("2026-11-17 05:00")), Some(el("2026-11-16 18:00")), "de madrugada cuenta el de ayer");
        assert_eq!(inicio_del_cierre(el("2026-11-21 15:00")), Some(el("2026-11-21 14:00")));
        assert_eq!(inicio_del_cierre(el("2026-11-22 12:00")), Some(el("2026-11-21 14:00")), "domingo: desde el sábado");
        assert_eq!(inicio_del_cierre(el("2026-11-23 06:00")), Some(el("2026-11-21 14:00")), "lunes temprano: también");
        assert_eq!(inicio_del_cierre(el("2026-11-21 07:00")), Some(el("2026-11-20 18:00")), "sábado temprano: desde el viernes");
    }

    fn a_las(fecha: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(&format!("{fecha}:00-05:00")).unwrap()
    }

    #[test]
    fn un_aviso_por_cierre() {
        assert_eq!(aviso_fuera_de_horario(CLIENTE, a_las("2026-11-16T12:00")), None);
        let noche = aviso_fuera_de_horario(CLIENTE, a_las("2026-11-16T20:00")).unwrap();
        let madrugada = aviso_fuera_de_horario(CLIENTE, a_las("2026-11-17T05:00")).unwrap();
        let siguiente = aviso_fuera_de_horario(CLIENTE, a_las("2026-11-17T19:00")).unwrap();
        assert_eq!(noche.id, madrugada.id, "mismo cierre, mismo aviso");
        assert_ne!(noche.id, siguiente.id);
        assert_ne!(noche.id, aviso_fuera_de_horario(OTRO, a_las("2026-11-16T20:00")).unwrap().id);
        assert_eq!((noche.tipo, noche.fecha), (Tipo::Aviso, a_las("2026-11-16T20:00") + chrono::Duration::seconds(1)));
        // El consumer lo acepta del lado automático (y de ningún otro).
        let t = |lado| Tema { cliente: CLIENTE, lado, evento: Evento::Mensaje };
        let ajustes = Ajustes::default();
        assert!(matches!(aplicar(&t(Lado::Automatico), Entrante::Mensaje(noche.clone()), None, 1, &ajustes), Cambio::Guardar(_)));
        assert!(matches!(aplicar(&t(Lado::Cliente), Entrante::Mensaje(noche), None, 1, &ajustes), Cambio::Ignorar(_)));
    }
}
