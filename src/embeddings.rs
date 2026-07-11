use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct EmbedRequest<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

/// Embebe la consulta del usuario vía bge-m3 (POST /embed).
/// Fail-open: cualquier falla (timeout, red, status, JSON) devuelve None
/// y la búsqueda sigue funcionando solo con la vía normal — nunca se
/// propaga un error al cliente por culpa del servicio de embeddings.
pub async fn embed_query(
    http: &reqwest::Client,
    base_url: &str,
    text: &str,
) -> Option<Vec<f32>> {
    let respuesta = http
        .post(format!("{}/embed", base_url.trim_end_matches('/')))
        .json(&EmbedRequest { text })
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .and_then(|r| r.error_for_status());

    let respuesta = match respuesta {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("bge-embeddings no disponible: {}", e);
            return None;
        }
    };

    match respuesta.json::<EmbedResponse>().await {
        Ok(body) => Some(body.embedding),
        Err(e) => {
            tracing::warn!("Respuesta inválida de bge-embeddings: {}", e);
            None
        }
    }
}
