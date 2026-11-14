//! Autoservicio de impresión, paso 4: imprime lo que la caja autorizó.
//!
//! La caja elige impresora en el POS y el trabajo queda `autorizado` (paso 3). Aquí se toma
//! (`imprimiendo`), se baja el PDF de MinIO, se manda a la Kyocera por IPP (`ipp.rs`) y se espera a
//! que la impresora diga que terminó. Entonces se crea el pedido para cobrarlo (`impreso`), se le
//! avisa al cliente en su celular y se registra en Loki. Si algo falla: `error` con el motivo, y la
//! cajera puede mandarlo a la otra impresora.
//!
//! Dos caminos para enterarse, como las notificaciones: NATS `impresion.autorizada` (al instante,
//! lo publica el POS) y una revisión de la tabla cada 20 s. `db::impresiones::tomar` garantiza que
//! solo uno lo toma.

use std::time::{Duration, Instant};

use futures_util::StreamExt;
use uuid::Uuid;

use crate::{db, db::impresiones::PorImprimir, impresion, ipp, push, AppState};

const SUBJECT: &str = "impresion.autorizada";
const CADA: Duration = Duration::from_secs(20);
/// Hasta cuánto se espera a que la impresora termine.
const ESPERA_MAX: Duration = Duration::from_secs(15 * 60);
const CADA_CONSULTA: Duration = Duration::from_secs(3);

pub fn vigilar(state: AppState) {
    if state.minio.is_none() {
        return;
    }
    let http = ipp::cliente();
    {
        let (state, http) = (state.clone(), http.clone());
        tokio::spawn(async move {
            let mut cada = tokio::time::interval(CADA);
            loop {
                cada.tick().await;
                revisar(&state, &http).await;
            }
        });
    }
    if state.config.nats_usuario.is_empty() {
        tracing::info!("autoservicio: sin NATS_USUARIO, solo la revisión cada {} s", CADA.as_secs());
        return;
    }
    tokio::spawn(async move {
        loop {
            if let Err(e) = escuchar(&state, &http).await {
                tracing::warn!("autoservicio: NATS: {e}; reintento en 10 s");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

async fn revisar(state: &AppState, http: &reqwest::Client) {
    match db::impresiones::interrumpidos(&state.pool).await {
        Ok(0) => {}
        Ok(n) => tracing::warn!("autoservicio: {n} trabajo(s) interrumpido(s) pasan a error"),
        Err(e) => tracing::warn!("autoservicio: {e}"),
    }
    match db::impresiones::autorizados(&state.pool).await {
        Ok(ids) => {
            for id in ids {
                tokio::spawn(imprimir(state.clone(), http.clone(), id));
            }
        }
        Err(e) => tracing::warn!("autoservicio: {e}"),
    }
}

async fn escuchar(state: &AppState, http: &reqwest::Client) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cfg = &state.config;
    let cliente = async_nats::ConnectOptions::with_user_and_password(cfg.nats_usuario.clone(), cfg.nats_password.clone())
        .name("xplaya-autoservicio")
        .connect(&cfg.nats_url)
        .await?;
    let mut mensajes = cliente.subscribe(SUBJECT).await?;
    tracing::info!("autoservicio: escuchando NATS ({SUBJECT})");
    while let Some(m) = mensajes.next().await {
        let id = serde_json::from_slice::<serde_json::Value>(&m.payload)
            .ok()
            .and_then(|v| v.get("trabajoId")?.as_str()?.parse::<Uuid>().ok());
        if let Some(id) = id {
            tokio::spawn(imprimir(state.clone(), http.clone(), id));
        }
    }
    Ok(())
}

/// Un trabajo, de principio a fin. Si otro ya lo tomó, no hace nada.
async fn imprimir(state: AppState, http: reqwest::Client, id: Uuid) {
    let t = match db::impresiones::tomar(&state.pool, id).await {
        Ok(Some(t)) => t,
        Ok(None) => return,
        Err(e) => return tracing::warn!("autoservicio {id}: {e}"),
    };
    tracing::info!("autoservicio {id}: imprimiendo en {}", t.impresora);
    match mandar(&state, &http, &t).await {
        Ok(producto_id) => match db::impresiones::terminar(&state.pool, &t, producto_id).await {
            Ok(pedido) => {
                tracing::info!("autoservicio {id}: impreso, pedido {pedido}");
                tokio::spawn(avisar_cliente(state.clone(), t.cliente_id));
                registrar_en_loki(&state, &t).await;
            }
            // Ya salió: la cajera lo ve como error y lo cobra a mano, pero no se reimprime solo.
            Err(e) => {
                tracing::error!("autoservicio {id}: salió pero no se guardó el pedido: {e}");
                let _ = db::impresiones::fallar(&state.pool, id, "Salió, pero no se creó el pedido: cóbralo a mano.").await;
            }
        },
        Err(motivo) => {
            tracing::warn!("autoservicio {id}: {motivo}");
            if let Err(e) = db::impresiones::fallar(&state.pool, id, &motivo).await {
                tracing::error!("autoservicio {id}: {e}");
            }
        }
    }
}

/// Manda el PDF y espera a que salga. `Ok`: el producto con que se cobra. `Err`: el motivo para
/// la cajera.
async fn mandar(state: &AppState, http: &reqwest::Client, t: &PorImprimir) -> Result<Uuid, String> {
    let impresoras = db::impresiones::impresoras(&state.pool).await.map_err(|e| format!("BD: {e}"))?;
    let imp = impresoras
        .iter()
        .find(|i| i.clave == t.impresora)
        .ok_or_else(|| format!("La impresora {} ya no está en el autoservicio.", t.impresora))?;
    let minio = state.minio.as_ref().ok_or("El autoservicio está apagado.")?;
    let pdf = minio.bajar(&t.objeto).await.map_err(|e| {
        tracing::error!("autoservicio {}: {e}", t.id);
        "No encontramos el PDF (¿ya se borró?).".to_string()
    })?;
    let trabajo = ipp::Trabajo {
        nombre: &t.archivo,
        paginas: impresion::rangos(&t.paginas, t.paginas_pdf as u32)?,
        copias: t.copias as u32,
        doble_cara: t.doble_cara,
    };
    let job = match ipp::imprimir(http, &imp.uri, &trabajo, &pdf).await {
        Ok(job) => job,
        Err(e) => return Err(con_razones(http, &imp.uri, e).await),
    };
    esperar(http, &imp.uri, job).await?;
    Ok(imp.producto_id)
}

/// Hasta que la impresora diga que terminó (9), lo canceló (7) o lo abortó (8).
async fn esperar(http: &reqwest::Client, uri: &str, job: i32) -> Result<(), String> {
    let inicio = Instant::now();
    let mut fallas = 0;
    loop {
        tokio::time::sleep(CADA_CONSULTA).await;
        match ipp::estado_trabajo(http, uri, job).await {
            Ok((9, _)) => return Ok(()),
            Ok((7, _)) => return Err("Se canceló en la impresora.".into()),
            Ok((8, razones)) => {
                let e = ipp::explicar(&razones).map(str::to_string).unwrap_or_else(|| "La impresora no lo pudo imprimir.".into());
                return Err(con_razones(http, uri, e).await);
            }
            Ok(_) => fallas = 0,
            // Unas cuantas fallas seguidas al preguntar no detienen nada: la impresora sigue.
            Err(e) => {
                fallas += 1;
                if fallas >= 10 {
                    return Err(format!("{e} Revisa si salió antes de mandarlo otra vez."));
                }
            }
        }
        if inicio.elapsed() > ESPERA_MAX {
            return Err(con_razones(http, uri, "La impresora no terminó en 15 minutos. Revisa si salió.".into()).await);
        }
    }
}

/// Agrega lo que diga la impresora (sin papel, atorado…) al motivo.
async fn con_razones(http: &reqwest::Client, uri: &str, motivo: String) -> String {
    match ipp::explicar(&ipp::razones_impresora(http, uri).await) {
        Some(razon) if !motivo.contains(razon) => format!("{razon} {motivo}"),
        _ => motivo,
    }
}

async fn avisar_cliente(state: AppState, cliente_id: Uuid) {
    let Some(vapid) = state.vapid.clone() else { return };
    let cuerpo = serde_json::json!({
        "titulo": "🖨️ Tu impresión está lista",
        "texto": "En un momento te llevamos tus hojas y lo pagas ahí.",
        "url": "/imprimir",
        "tag": "imprimir",
    });
    if let Err(e) = push::mandar_del_momento(&state, &vapid, cliente_id, cuerpo, Duration::from_secs(30 * 60)).await {
        tracing::warn!("autoservicio: aviso a {cliente_id}: {e}");
    }
}

/// Como los trabajos de la PC de la papelería (k3s-manifests, "Eventos de la PC de la papelería
/// → Loki"): labels `job` y `tipo`; lo demás en la línea JSON. `impresora` con el nombre del label
/// de SNMP ("Kyocera MA5500ifx" → "kyocera-ma5500ifx").
async fn registrar_en_loki(state: &AppState, t: &PorImprimir) {
    if state.config.loki_url.is_empty() {
        return;
    }
    let impresora = db::impresiones::impresoras(&state.pool)
        .await
        .ok()
        .and_then(|v| v.into_iter().find(|i| i.clave == t.impresora))
        .map_or_else(|| t.impresora.to_lowercase(), |i| i.nombre.to_lowercase().replace(' ', "-"));
    let linea = serde_json::json!({
        "cliente": db::impresiones::nombre_cliente(&state.pool, t.cliente_id).await.unwrap_or_default(),
        "impresora": impresora,
        "paginas": t.caras,
        "copias": t.copias,
        "color": "bn",
        "descripcion": format!("Autoservicio (app): {}{}", t.archivo, if t.doble_cara { ", doble cara" } else { "" }),
    });
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let cuerpo = serde_json::json!({
        "streams": [{ "stream": { "job": "papeleria-pc", "tipo": "impresion" }, "values": [[ns.to_string(), linea.to_string()]] }]
    });
    match state.http.post(&state.config.loki_url).json(&cuerpo).timeout(Duration::from_secs(10)).send().await {
        Ok(r) if r.status().is_success() => {}
        Ok(r) => tracing::warn!("autoservicio: Loki respondió {}", r.status()),
        Err(e) => tracing::warn!("autoservicio: Loki: {e}"),
    }
}
