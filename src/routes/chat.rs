//! Chat de la app con la papelería (`CHAT.md`), solo con sesión de passkey y para quien lo
//! pueda ver (`CHAT_CLIENTES_PRUEBA`, `reglas::chat_visible`).
//!
//! Lo que hace el cliente no se guarda aquí: se publica en NATS (`chat/nats.rs`) y el consumer
//! lo guarda al volver del stream, igual que lo de la caja. La app se entera por el SSE de
//! `/chat/eventos`, que solo trae lo que ya está en `Mensajes`.
//!
//! - `GET /chat`: la pantalla, con los últimos mensajes.
//! - `GET /chat/anteriores?antes={id}`: más historial al subir; sin `antes`, los últimos (la app
//!   se pone al día así al reconectar).
//! - `GET /chat/eventos`: SSE (`vista::EnVivo`), como `routes/cuenta.rs`.
//! - `POST /chat/archivo`: sube un archivo (multipart) y regresa su número; el mensaje que lo
//!   lleva se manda después con `POST /chat/mensaje` (`chat/subidas.rs`).
//! - `GET /chat/archivo/{mensaje}/{n}/{original|pdf|mini}`: un archivo, en stream y con `Range`
//!   (los videos se adelantan). `?bajar=1` lo baja en vez de abrirlo.
//! - `GET /chat/zip/{mensaje}`: "Bajar todo", con carpetas por tipo.
//! - `POST /chat/mensaje`, `/chat/leido`, `/chat/{id}/editar`, `/chat/{id}/eliminar`,
//!   `/chat/escribiendo`.
//! - `GET /chat/sin-leer`: el número del globito del menú.

use std::convert::Infallible;

use axum::{
    extract::{Multipart, Path, Query, State},
    body::Body,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Redirect, Response,
    },
    Json,
};
use chrono::{DateTime, FixedOffset, Utc};
use minijinja::context;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::{
    io::AsyncWriteExt,
    sync::broadcast::{error::RecvError, Receiver},
};
use uuid::Uuid;

use crate::{
    caja::EnPantalla,
    chat::{
        archivos::{self, Clase},
        contrato::{Adjunto, Editado, Eliminado, Entrante, Evento, Lado, Leido, Tema, Tipo, VERSION},
        nats::Cambio as CambioEnVivo,
        reglas::{self, Ajustes, Cambio},
        tarjetas::{self, Link},
    },
    db, impresion,
    minio::BUCKET_CHAT,
    routes::{alta::error_json, app::cliente_sesion},
    AppState,
};

/// Mensajes por página.
const PAGINA: i64 = 50;

/// Cliente con sesión que puede ver el chat, y los ajustes.
async fn del_chat(state: &AppState, headers: &HeaderMap) -> Result<(Uuid, Ajustes), StatusCode> {
    let cliente = cliente_sesion(state, headers).await.ok_or(StatusCode::UNAUTHORIZED)?;
    let ajustes = db::chat::ajustes(&state.pool).await.map_err(error_bd)?;
    if !reglas::chat_visible(ajustes.clientes_prueba.as_deref(), cliente) {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok((cliente, ajustes))
}

/// Para mostrar "Preguntar por el chat" fuera del chat (`/tienda/{nid}`).
pub async fn visible(state: &AppState, headers: &HeaderMap) -> bool {
    del_chat(state, headers).await.is_ok()
}

/// Al mandar el carrito desde la app (`tienda::crear_pedido`), su cotización aparece en el chat
/// como mensaje automático, si el cliente ve el chat. Si algo falla, solo va al log: el pedido ya
/// se creó y la papelería ya tiene el aviso.
pub async fn cotizacion_del_carrito(state: &AppState, cliente: Uuid, pedido: Uuid) {
    let ajustes = match db::chat::ajustes(&state.pool).await {
        Ok(a) => a,
        Err(e) => return tracing::warn!("chat: cotización {pedido}: {e}"),
    };
    if !reglas::chat_visible(ajustes.clientes_prueba.as_deref(), cliente) {
        return;
    }
    let m = reglas::cotizacion_del_carrito(cliente, pedido, ahora());
    let tema = Tema { cliente, lado: Lado::Automatico, evento: Evento::Mensaje };
    if let Err(e) = state.chat.publicar(&tema, &m, Some(m.id)).await {
        tracing::warn!("chat: no se pudo mandar la cotización {pedido} a {cliente}: {e}");
    }
}

/// `POST /chat/compartir` sin service worker (recién instalada, o lo borró el navegador): lo
/// compartido se pierde, pero el chat abre en vez de un error. Normalmente lo atiende `sw.js`.
///
/// Hay que leer el cuerpo completo aunque se tire: si se contesta mientras la foto sigue
/// subiendo, el servidor corta la conexión y Cloudflare enseña su página de "sitio caído".
pub async fn compartir_sin_sw(body: Body) -> Redirect {
    // Tope por si acaso: lo mismo que acepta /chat/archivo (Cloudflare no deja pasar más).
    const TOPE: usize = (crate::chat::archivos::MB_VIDEO_TOPE as usize + 1) * 1024 * 1024;
    let mut leidos = 0;
    let mut stream = body.into_data_stream();
    while let Some(Ok(trozo)) = stream.next().await {
        leidos += trozo.len();
        if leidos > TOPE {
            break;
        }
    }
    tracing::info!("chat: compartir sin service worker ({leidos} bytes, se pierde)");
    Redirect::to("/chat")
}

fn error_bd(e: sqlx::Error) -> StatusCode {
    tracing::error!("chat: BD: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn ahora() -> DateTime<FixedOffset> {
    Utc::now().fixed_offset()
}

/// Publicar en NATS falló: la app lo deja en "enviando…" y lo vuelve a intentar.
fn sin_nats(e: String) -> Response {
    tracing::warn!("chat: NATS: {e}");
    error_json(StatusCode::SERVICE_UNAVAILABLE, "No se pudo mandar. Lo intentamos de nuevo en un momento.")
}

// ── Pantalla e historial ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PaginaParams {
    /// "Preguntar por el chat" en `/tienda/{nid}`: la tarjeta queda lista arriba del cuadro de
    /// texto y se manda con el primer mensaje.
    producto: Option<i32>,
}

pub async fn pagina(State(state): State<AppState>, headers: HeaderMap, Query(p): Query<PaginaParams>) -> Response {
    let Ok((cliente, ajustes)) = del_chat(&state, &headers).await else {
        return Redirect::to("/").into_response();
    };
    let datos = async {
        let mut mensajes = db::chat::conversacion(&state.pool, cliente, None, PAGINA).await?;
        tarjetas::poner(&state.pool, (&state.config).into(), &mut mensajes).await?;
        let bloqueado = db::chat::bloqueado(&state.pool, cliente).await?;
        let lista = match p.producto {
            Some(nid) => tarjetas::resolver(&state.pool, (&state.config).into(), "producto", &nid.to_string())
                .await?
                .map(|tarjeta| context! { tipo => "producto", ref => nid.to_string(), tarjeta }),
            None => None,
        };
        let sin_foto = db::perfil::de_cliente(&state.pool, cliente).await?.is_some_and(|p| p.fotoperfil.is_none());
        Ok::<_, sqlx::Error>((mensajes, bloqueado, lista, sin_foto))
    };
    let (mensajes, bloqueado, lista, sin_foto) = match datos.await {
        Ok(d) => d,
        Err(e) => return error_bd(e).into_response(),
    };
    let ctx = context! {
        en_app => true,
        nav => "chat",
        abierta => ajustes.papeleria_abierta,
        estado => ajustes.estado,
        // La invitación a poner foto (Mi perfil); solo si se pueden subir.
        invitar_foto => sin_foto && state.minio.is_some(),
        chat => context! {
            mensajes,
            bloqueado,
            minutosEditar => ajustes.minutos_editar,
            hayMas => mensajes.len() as i64 == PAGINA,
            pagina => PAGINA,
            // Para revisar en el celular antes de subir; el servidor los vuelve a revisar.
            archivos => state.minio.is_some(),
            bytesVideo => archivos::limite(Clase::Video, &ajustes),
            bytesDocumento => archivos::limite(Clase::Archivo, &ajustes),
            maxAdjuntos => reglas::MAX_ADJUNTOS,
            lista,
        },
    };
    match state.tmpl.get_template("app/chat.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("Render app/chat.html: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct AnterioresParams {
    antes: Option<Uuid>,
}

pub async fn anteriores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<AnterioresParams>,
) -> Result<Response, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    let mut mensajes = db::chat::conversacion(&state.pool, cliente, p.antes, PAGINA).await.map_err(error_bd)?;
    tarjetas::poner(&state.pool, (&state.config).into(), &mut mensajes).await.map_err(error_bd)?;
    Ok(Json(mensajes).into_response())
}

pub async fn sin_leer(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    let n = db::chat::sin_leer(&state.pool, cliente).await.map_err(error_bd)?;
    Ok(Json(serde_json::json!({ "sinLeer": n })).into_response())
}

// ── En vivo ─────────────────────────────────────────────────────────────────

pub async fn eventos(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (cliente, _) = match del_chat(&state, &headers).await {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    let conexion = Conexion {
        cambios: state.chat.suscribir(),
        cliente,
        // Con el chat abierto tampoco hacen falta avisos por Web Push (etapa 6).
        _en_pantalla: state.cajas.en_pantalla(cliente),
    };
    let stream = futures_util::stream::unfold(conexion, siguiente);
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

struct Conexion {
    cambios: Receiver<CambioEnVivo>,
    cliente: Uuid,
    _en_pantalla: EnPantalla,
}

async fn siguiente(mut c: Conexion) -> Option<(Result<Event, Infallible>, Conexion)> {
    loop {
        let evento = match c.cambios.recv().await {
            Ok((cliente, cambio)) if cliente == c.cliente => {
                let json = serde_json::to_string(&*cambio).unwrap_or_else(|_| "null".into());
                Event::default().event(cambio.nombre()).data(json)
            }
            Ok(_) => continue, // de otro cliente
            // Se atrasó (muchos cambios de golpe): la app vuelve a pedir la conversación.
            Err(RecvError::Lagged(_)) => Event::default().event("recargar").data("null"),
            Err(RecvError::Closed) => return None,
        };
        return Some((Ok(evento), c));
    }
}

// ── Lo que hace el cliente ──────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevoMensaje {
    /// Lo genera el celular: un reintento sin señal no se guarda dos veces.
    id: Uuid,
    #[serde(default)]
    texto: String,
    responde_a: Option<Uuid>,
    /// Los números que dio `POST /chat/archivo`, en el orden en que se muestran.
    #[serde(default)]
    archivos: Vec<Uuid>,
    /// La tarjeta de "Preguntar por el chat". Sin ella, se busca un link en el texto.
    tipo: Option<Tipo>,
    #[serde(rename = "ref")]
    referencia: Option<String>,
}

/// Responde cuando NATS ya lo tiene (el ✓). Se guarda y llega por el SSE un instante después.
pub async fn mandar(State(state): State<AppState>, headers: HeaderMap, Json(m): Json<NuevoMensaje>) -> Result<Response, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    let bloqueado = db::chat::bloqueado(&state.pool, cliente).await.map_err(error_bd)?;
    let adjuntos = match state.subidas.ver(cliente, m.id, &m.archivos) {
        Some(a) => a,
        None => {
            tracing::info!("chat: {cliente} mandó {} con archivos que no están", m.id);
            return Ok(error_json(StatusCode::BAD_REQUEST, "Algunos archivos ya no están. Vuelve a elegirlos."));
        }
    };
    let tarjeta = match (m.tipo, m.referencia) {
        // `del_cliente` revisa que sea producto, recibo o cotización y que el ref no venga vacío.
        (Some(tipo), Some(referencia)) => Some((tipo, referencia)),
        (Some(tipo), None) => Some((tipo, String::new())),
        _ => tarjeta_del_texto(&state, &m.texto).await,
    };
    let borrador = reglas::Borrador { id: m.id, texto: m.texto, responde_a: m.responde_a, adjuntos, tarjeta };
    let mensaje = match reglas::del_cliente(cliente, borrador, bloqueado, ahora()) {
        Ok(msg) => msg,
        Err(motivo) => return Ok(error_json(StatusCode::BAD_REQUEST, motivo)),
    };
    let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Mensaje };
    Ok(match state.chat.publicar(&tema, &mensaje, Some(mensaje.id)).await {
        Ok(()) => {
            state.subidas.publicadas(&m.archivos);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => sin_nats(e),
    })
}

/// Si el texto trae un link a un producto, recibo o cotización de xplaya (también `/r/…`), el
/// mensaje va como esa tarjeta; el texto queda igual. Si la URL corta no se encuentra o falla la
/// BD, se manda solo el texto.
async fn tarjeta_del_texto(state: &AppState, texto: &str) -> Option<(Tipo, String)> {
    let hosts = [state.config.site_host(), state.config.app_host()];
    let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
    match tarjetas::de_link(texto, &hosts)? {
        Link::Tarjeta(tipo, referencia) => Some((tipo, referencia)),
        Link::Corto(code) => {
            let (tipo, id) = match db::short_urls::buscar(&state.pool, &code).await {
                Ok(destino) => destino?,
                Err(e) => {
                    tracing::warn!("chat: URL corta {code}: {e}");
                    return None;
                }
            };
            match tarjetas::de_ruta(&db::short_urls::ruta(&tipo, id)?)? {
                Link::Tarjeta(tipo, referencia) => Some((tipo, referencia)),
                Link::Corto(_) => None,
            }
        }
    }
}

// ── Archivos ────────────────────────────────────────────────────────────────

/// Archivo en `/tmp` que se borra solo al terminar la petición, salga bien o mal.
struct Temporal(std::path::PathBuf);

impl Temporal {
    fn nuevo() -> Self {
        Self(std::env::temp_dir().join(format!("xplaya-chat-{}", Uuid::new_v4())))
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

const MB: u64 = 1024 * 1024;

/// Sube un archivo del chat. Campos en este orden: `mensaje` (el id que generó el celular para
/// el mensaje que lo va a llevar) y `archivo`. El archivo va a disco por pedazos, sin pasar
/// entero por la memoria, y de ahí a MinIO; las miniaturas, las páginas y la conversión de
/// Word a PDF se hacen aquí. Regresa `{archivo, adjunto}`; el adjunto espera en
/// `state.subidas` a su mensaje.
pub async fn subir_archivo(State(state): State<AppState>, headers: HeaderMap, mut multipart: Multipart) -> Result<Response, StatusCode> {
    let (cliente, ajustes) = del_chat(&state, &headers).await?;
    let minio = state.minio.as_ref().ok_or(StatusCode::NOT_FOUND)?.en(BUCKET_CHAT);
    if db::chat::bloqueado(&state.pool, cliente).await.map_err(error_bd)? {
        return Ok(error_json(StatusCode::BAD_REQUEST, "La papelería bloqueó tu chat."));
    }
    const INCOMPLETO: &str = "El archivo no llegó completo. Inténtalo de nuevo.";
    let temporal = Temporal::nuevo();
    let tope = archivos::limite_mayor(&ajustes);
    let mut mensaje: Option<Uuid> = None;
    // Nombre que puso el cliente, bytes y los primeros bytes (para saber qué es).
    let mut recibido: Option<(String, u64, Vec<u8>)> = None;
    loop {
        let mut campo = match multipart.next_field().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(_) => return Ok(error_json(StatusCode::BAD_REQUEST, INCOMPLETO)),
        };
        match campo.name() {
            Some("mensaje") => mensaje = campo.text().await.ok().and_then(|t| t.trim().parse().ok()),
            Some("archivo") if mensaje.is_some() => {
                let original = campo.file_name().unwrap_or("").to_string();
                let mut disco = tokio::fs::File::create(&temporal.0).await.map_err(error_disco)?;
                let (mut bytes, mut inicio) = (0u64, Vec::with_capacity(archivos::BYTES_FIRMA));
                loop {
                    let trozo = match campo.chunk().await {
                        Ok(Some(t)) => t,
                        Ok(None) => break,
                        Err(_) => return Ok(error_json(StatusCode::BAD_REQUEST, INCOMPLETO)),
                    };
                    bytes += trozo.len() as u64;
                    if bytes > tope {
                        return Ok(error_json(StatusCode::PAYLOAD_TOO_LARGE, &format!("El archivo pasa de {} MB.", tope / MB)));
                    }
                    let falta = archivos::BYTES_FIRMA.saturating_sub(inicio.len()).min(trozo.len());
                    inicio.extend_from_slice(&trozo[..falta]);
                    disco.write_all(&trozo).await.map_err(error_disco)?;
                }
                disco.flush().await.map_err(error_disco)?;
                recibido = Some((original, bytes, inicio));
            }
            _ => {}
        }
    }
    let (Some(mensaje), Some((original, bytes, inicio))) = (mensaje, recibido) else {
        return Ok(error_json(StatusCode::BAD_REQUEST, "Elige un archivo."));
    };
    if bytes == 0 {
        return Ok(error_json(StatusCode::BAD_REQUEST, "El archivo está vacío."));
    }
    let tipo = archivos::clasificar(&original, &inicio);
    let limite = archivos::limite(tipo.clase, &ajustes);
    if bytes > limite {
        let que = if tipo.clase == Clase::Video { "El video" } else { "El archivo" };
        return Ok(error_json(StatusCode::PAYLOAD_TOO_LARGE, &format!("{que} pasa de {} MB.", limite / MB)));
    }

    let archivo = Uuid::new_v4();
    let llaves = archivos::llaves(cliente, mensaje, archivo, &tipo.ext);
    if let Err(e) = minio.subir_archivo(&llaves.original, &temporal.0, tipo.mime).await {
        tracing::warn!("chat: subir {}: {e}", llaves.original);
        return Ok(error_json(StatusCode::SERVICE_UNAVAILABLE, "No se pudo subir. Inténtalo de nuevo."));
    }
    let mut adjunto = Adjunto {
        nombre: archivos::nombre_visible(&original, &tipo.ext),
        mime: tipo.mime.to_string(),
        bytes,
        llave: llaves.original.clone(),
        paginas: None,
        pdf: None,
        miniatura: None,
    };
    // Lo de abajo es extra: si falla, el archivo se manda igual (sin miniatura, sin páginas o
    // solo para bajar).
    match tipo.clase {
        Clase::Imagen if tipo.ext != "gif" => {
            if let Some(mini) = miniatura(&state, &temporal).await {
                match minio.subir(&llaves.miniatura, mini, "image/jpeg").await {
                    Ok(()) => adjunto.miniatura = Some(llaves.miniatura),
                    Err(e) => tracing::warn!("chat: miniatura {}: {e}", llaves.miniatura),
                }
            }
        }
        Clase::Pdf => adjunto.paginas = paginas(&state, &temporal).await,
        Clase::Office => {
            let pdf = Temporal::nuevo();
            match a_pdf(&state, &temporal, &tipo.ext, &pdf).await {
                Ok(()) => {
                    adjunto.paginas = paginas(&state, &pdf).await;
                    match minio.subir_archivo(&llaves.pdf, &pdf.0, "application/pdf").await {
                        Ok(()) => adjunto.pdf = Some(llaves.pdf),
                        Err(e) => tracing::warn!("chat: subir {}: {e}", llaves.pdf),
                    }
                }
                Err(e) => tracing::warn!("chat: Gotenberg no convirtió {} ({}): {e}", llaves.original, adjunto.nombre),
            }
        }
        _ => {}
    }
    state.subidas.guardar(archivo, cliente, mensaje, adjunto.clone());
    Ok(Json(serde_json::json!({ "archivo": archivo, "adjunto": adjunto })).into_response())
}

fn error_disco(e: std::io::Error) -> StatusCode {
    tracing::error!("chat: disco: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

/// El mensaje con sus archivos, si es de este cliente y no se eliminó (si no, 404: no se dice si
/// existe).
async fn mensaje_con_archivos(state: &AppState, headers: &HeaderMap, mensaje: Uuid) -> Result<reglas::Guardado, StatusCode> {
    let (cliente, _) = del_chat(state, headers).await?;
    let g = db::chat::guardado(&state.pool, mensaje).await.map_err(error_bd)?.ok_or(StatusCode::NOT_FOUND)?;
    if g.cliente != cliente || g.eliminado {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(g)
}

#[derive(Deserialize)]
pub struct ComoBajar {
    bajar: Option<String>,
}

/// Un archivo de un mensaje: `original`, `pdf` (el Word convertido) o `mini` (miniatura).
pub async fn archivo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((mensaje, n, cual)): Path<(Uuid, usize, String)>,
    Query(como): Query<ComoBajar>,
) -> Result<Response, StatusCode> {
    let g = mensaje_con_archivos(&state, &headers, mensaje).await?;
    let minio = state.minio.as_ref().ok_or(StatusCode::NOT_FOUND)?.en(BUCKET_CHAT);
    let a = g.adjuntos.get(n).ok_or(StatusCode::NOT_FOUND)?;
    let (llave, mime, nombre) = match cual.as_str() {
        "original" => (a.llave.clone(), a.mime.clone(), a.nombre.clone()),
        "pdf" => {
            let base = a.nombre.rsplit_once('.').map_or(a.nombre.as_str(), |(b, _)| b);
            (a.pdf.clone().ok_or(StatusCode::NOT_FOUND)?, "application/pdf".to_string(), format!("{base}.pdf"))
        }
        "mini" => (a.miniatura.clone().ok_or(StatusCode::NOT_FOUND)?, "image/jpeg".to_string(), format!("mini-{}", a.nombre)),
        _ => return Err(StatusCode::NOT_FOUND),
    };
    // Solo `bytes=…`: lo demás no se le pasa a MinIO.
    let rango = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .filter(|r| r.starts_with("bytes=") && r.len() < 100 && r[6..].bytes().all(|b| b.is_ascii_digit() || b == b'-' || b == b','));
    let d = match minio.bajar_stream(&llave, rango).await {
        Ok(d) => d,
        Err(e) if e.contains("404") => return Ok((StatusCode::GONE, "Archivo vencido").into_response()),
        Err(e) if e.contains("416") => return Ok(StatusCode::RANGE_NOT_SATISFIABLE.into_response()),
        Err(e) => {
            tracing::warn!("chat: bajar {llave}: {e}");
            return Err(StatusCode::BAD_GATEWAY);
        }
    };
    let en_linea = archivos::en_linea(&mime) && como.bajar.is_none();
    let tipo = if archivos::en_linea(&mime) { mime.as_str() } else { "application/octet-stream" };
    let mut r = Response::new(Body::from_stream(d.cuerpo.bytes_stream()));
    *r.status_mut() = if d.status == 206 { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK };
    let h = r.headers_mut();
    let mut poner = |nombre: header::HeaderName, valor: &str| {
        if let Ok(v) = HeaderValue::from_str(valor) {
            h.insert(nombre, v);
        }
    };
    poner(header::CONTENT_TYPE, tipo);
    poner(header::CONTENT_DISPOSITION, &archivos::disposicion(en_linea, &nombre));
    poner(header::ACCEPT_RANGES, "bytes");
    // Un archivo del chat no cambia nunca: el celular lo guarda un día.
    poner(header::CACHE_CONTROL, "private, max-age=86400");
    poner(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    if let Some(l) = &d.largo {
        poner(header::CONTENT_LENGTH, l);
    }
    if let Some(cr) = &d.rango {
        poner(header::CONTENT_RANGE, cr);
    }
    Ok(r)
}

/// "Bajar todo": un zip sin compresión (fotos y videos ya vienen comprimidos) que se arma
/// mientras se baja, archivo por archivo desde MinIO, sin juntarlo en memoria ni en disco.
/// Lo vencido se salta.
pub async fn zip(State(state): State<AppState>, headers: HeaderMap, Path(mensaje): Path<Uuid>) -> Result<Response, StatusCode> {
    use async_zip::{base::write::ZipFileWriter, Compression, ZipDateTimeBuilder, ZipEntryBuilder};
    use chrono::{Datelike, Timelike};
    use futures_util::AsyncWriteExt as _;

    let g = mensaje_con_archivos(&state, &headers, mensaje).await?;
    let minio = state.minio.as_ref().ok_or(StatusCode::NOT_FOUND)?.en(BUCKET_CHAT);
    let rutas = archivos::rutas_zip(&g.adjuntos);
    if rutas.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    // Los archivos salen con la fecha del mensaje (hora local, como la guarda el zip).
    let f = g.fecha;
    let fecha = ZipDateTimeBuilder::new()
        .year(f.year())
        .month(f.month())
        .day(f.day())
        .hour(f.hour())
        .minute(f.minute())
        .second(f.second())
        .build();
    let (escritor, lector) = tokio::io::duplex(256 * 1024);
    tokio::spawn(async move {
        let armar = async {
            let mut zip = ZipFileWriter::with_tokio(escritor);
            for (llave, ruta) in rutas {
                let d = match minio.bajar_stream(&llave, None).await {
                    Ok(d) => d,
                    Err(e) if e.contains("404") => continue,
                    Err(e) => return Err(e),
                };
                let mut entrada =
                    zip.write_entry_stream(ZipEntryBuilder::new(ruta.into(), Compression::Stored).last_modification_date(fecha)).await.map_err(|e| e.to_string())?;
                let mut cuerpo = d.cuerpo.bytes_stream();
                while let Some(trozo) = cuerpo.next().await {
                    entrada.write_all(&trozo.map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
                }
                entrada.close().await.map_err(|e| e.to_string())?;
            }
            zip.close().await.map_err(|e| e.to_string())?;
            Ok::<_, String>(())
        };
        // Si se corta (o el celular deja de bajarlo), el zip queda incompleto y el navegador lo
        // marca como fallido.
        if let Err(e) = armar.await {
            tracing::warn!("chat: zip de {mensaje}: {e}");
        }
    });
    let nombre = format!("archivos-{}.zip", g.fecha.format("%Y-%m-%d"));
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (header::CONTENT_DISPOSITION, archivos::disposicion(false, &nombre)),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(lector)),
    )
        .into_response())
}

/// Abre la imagen en memoria: una a la vez (`subidas.pesado`).
async fn miniatura(state: &AppState, temporal: &Temporal) -> Option<Vec<u8>> {
    let _turno = state.subidas.pesado.acquire().await.ok()?;
    let ruta = temporal.0.clone();
    tokio::task::spawn_blocking(move || std::fs::read(ruta).ok().and_then(|b| archivos::miniatura(&b))).await.ok().flatten()
}

/// Páginas de un PDF; `None` si tiene contraseña o no se puede abrir (se manda igual).
async fn paginas(state: &AppState, temporal: &Temporal) -> Option<u32> {
    let _turno = state.subidas.pesado.acquire().await.ok()?;
    let ruta = temporal.0.clone();
    tokio::task::spawn_blocking(move || std::fs::read(ruta).ok().and_then(|b| impresion::contar_paginas(&b).ok())).await.ok().flatten()
}

/// Word, Excel o PowerPoint a PDF con Gotenberg (LibreOffice), de disco a disco.
async fn a_pdf(state: &AppState, origen: &Temporal, ext: &str, destino: &Temporal) -> Result<(), String> {
    let _turno = state.subidas.gotenberg.acquire().await.map_err(|e| e.to_string())?;
    let archivo = tokio::fs::File::open(&origen.0).await.map_err(|e| e.to_string())?;
    let largo = archivo.metadata().await.map_err(|e| e.to_string())?.len();
    let cuerpo = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(archivo));
    // Gotenberg decide cómo convertir por la extensión del nombre.
    let parte = reqwest::multipart::Part::stream_with_length(cuerpo, largo).file_name(format!("documento.{ext}"));
    let resp = state
        .http
        .post(format!("{}/forms/libreoffice/convert", state.config.gotenberg_url))
        .multipart(reqwest::multipart::Form::new().part("files", parte))
        .timeout(std::time::Duration::from_secs(180))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("{}", resp.status()));
    }
    let mut disco = tokio::fs::File::create(&destino.0).await.map_err(|e| e.to_string())?;
    let mut cuerpo = resp.bytes_stream();
    while let Some(trozo) = cuerpo.next().await {
        disco.write_all(&trozo.map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
    }
    disco.flush().await.map_err(|e| e.to_string())
}

/// Cada 10 minutos borra de MinIO los archivos que subieron y nunca se mandaron.
pub fn limpiar_subidas(state: AppState) {
    let Some(minio) = state.minio.as_ref().map(|m| m.en(BUCKET_CHAT)) else { return };
    tokio::spawn(async move {
        let mut cada = tokio::time::interval(std::time::Duration::from_secs(10 * 60));
        loop {
            cada.tick().await;
            for adjunto in state.subidas.vencidas() {
                for llave in adjunto.llaves() {
                    if let Err(e) = minio.borrar(llave).await {
                        tracing::warn!("chat, limpieza de subidas: {e}");
                    }
                }
            }
        }
    });
}

/// El cliente vio la conversación: todo lo que le escribieron hasta ahora queda leído.
pub async fn leido(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Leido };
    let leido = Leido { v: VERSION, cliente_id: cliente, hasta: ahora() };
    Ok(match state.chat.publicar(&tema, &leido, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => sin_nats(e),
    })
}

/// "Vaciar chat": desde ahora la app solo le enseña lo nuevo. Solo de su lado; la caja ve todo.
pub async fn vaciar(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    db::chat::vaciar(&state.pool, cliente, crate::chat::contrato::hora_tienda(ahora())).await.map_err(error_bd)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Edicion {
    texto: String,
}

/// Se revisa con las mismas reglas que usará el consumer, para contestar el error al momento.
pub async fn editar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(e): Json<Edicion>,
) -> Result<Response, StatusCode> {
    let (cliente, ajustes) = del_chat(&state, &headers).await?;
    if db::chat::bloqueado(&state.pool, cliente).await.map_err(error_bd)? {
        return Ok(error_json(StatusCode::BAD_REQUEST, "La papelería bloqueó tu chat."));
    }
    if e.texto.chars().count() > reglas::MAX_CARACTERES {
        return Ok(error_json(StatusCode::BAD_REQUEST, "El mensaje es muy largo."));
    }
    let editado = Editado { v: VERSION, cliente_id: cliente, id, texto: e.texto, fecha: ahora() };
    let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Editado };
    let guardado = db::chat::guardado(&state.pool, id).await.map_err(error_bd)?;
    if let Cambio::Ignorar(motivo) = reglas::aplicar(&tema, Entrante::Editado(editado.clone()), guardado.as_ref(), 0, &ajustes) {
        tracing::info!("chat: {cliente} no pudo editar {id}: {motivo}");
        return Ok(error_json(StatusCode::BAD_REQUEST, "Este mensaje ya no se puede editar."));
    }
    Ok(match state.chat.publicar(&tema, &editado, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => sin_nats(e),
    })
}

/// "Eliminar para todos". Con el chat bloqueado también se puede.
pub async fn eliminar(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>) -> Result<Response, StatusCode> {
    let (cliente, ajustes) = del_chat(&state, &headers).await?;
    let eliminado = Eliminado { v: VERSION, cliente_id: cliente, id, fecha: ahora() };
    let tema = Tema { cliente, lado: Lado::Cliente, evento: Evento::Eliminado };
    let guardado = db::chat::guardado(&state.pool, id).await.map_err(error_bd)?;
    if let Cambio::Ignorar(motivo) = reglas::aplicar(&tema, Entrante::Eliminado(eliminado.clone()), guardado.as_ref(), 0, &ajustes) {
        tracing::info!("chat: {cliente} no pudo eliminar {id}: {motivo}");
        return Ok(error_json(StatusCode::BAD_REQUEST, "Este mensaje no se puede eliminar."));
    }
    Ok(match state.chat.publicar(&tema, &eliminado, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => sin_nats(e),
    })
}

/// La app lo manda como mucho cada 3 s mientras el cliente escribe.
pub async fn escribiendo(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, StatusCode> {
    let (cliente, _) = del_chat(&state, &headers).await?;
    state.chat.escribiendo(cliente).await;
    Ok(StatusCode::NO_CONTENT)
}
