//! MinIO de xplaya: los PDFs del autoservicio de impresión, en el bucket privado
//! `papeleria-impresiones` (se borran a los 7 días; MinIO los borra a los 8 por si acaso), y las
//! imágenes de los anuncios del kiosko, en `papeleria-anuncios` (`Minio::en(BUCKET_ANUNCIOS)`), y
//! los archivos del chat, en `papeleria-chat` (90 días; `CHAT.md`).
//!
//! Cliente S3 mínimo con la firma SigV4 hecha a mano, como en imagina (`pdfeitor/src/r2.rs`):
//! path-style (`{endpoint}/{bucket}/{llave}`), sin SDK de AWS. El usuario `xplaya-impresiones`
//! solo puede leer, escribir y borrar en esos tres buckets (`sellar-minio-xplaya.sh`,
//! `anuncios-minio.sh`, `chat-minio.sh`).
//!
//! Los archivos del chat (hasta 95 MB) no pasan por la memoria: `subir_archivo` manda el archivo
//! temporal como stream y `bajar_stream` devuelve el cuerpo de MinIO para pasarlo tal cual al
//! celular, con `Range` para que un video se pueda adelantar.

use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

use crate::config::Config;

pub const BUCKET: &str = "papeleria-impresiones";
pub const BUCKET_ANUNCIOS: &str = "papeleria-anuncios";
pub const BUCKET_CHAT: &str = "papeleria-chat";
/// En vez del SHA-256 del cuerpo, para mandarlo como stream sin leerlo dos veces. S3 y MinIO lo
/// aceptan; la firma sigue cubriendo método, llave, encabezados y fecha.
const SIN_FIRMA_CUERPO: &str = "UNSIGNED-PAYLOAD";
/// Bajar un video a un celular con poca señal tarda más que los 60 s del cliente.
const TIMEOUT_STREAM: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Lo que regresa MinIO al bajar en stream: el status (200, o 206 con `Range`), los
/// encabezados que le sirven al celular y el cuerpo.
pub struct Descarga {
    pub status: u16,
    pub largo: Option<String>,
    pub rango: Option<String>,
    pub tipo: Option<String>,
    pub cuerpo: reqwest::Response,
}
/// En las firmas; MinIO acepta cualquiera si no tiene región configurada.
const REGION: &str = "us-east-1";

#[derive(Clone)]
pub struct Minio {
    http: reqwest::Client,
    /// Sin barra final: `http://minio.minio-system.svc.cluster.local:9000`.
    endpoint: String,
    /// `Host` de `endpoint`, para la firma.
    host: String,
    usuario: String,
    password: String,
    bucket: &'static str,
}

impl Minio {
    /// `None` sin usuario: el autoservicio de impresión queda apagado.
    pub fn desde_config(cfg: &Config) -> Option<Self> {
        if cfg.minio_usuario.is_empty() || cfg.minio_password.is_empty() {
            return None;
        }
        let endpoint = cfg.minio_endpoint.trim_end_matches('/').to_string();
        let host = endpoint.split_once("://").map_or(endpoint.as_str(), |(_, h)| h).to_string();
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .ok()?;
        Some(Self {
            http,
            endpoint,
            host,
            usuario: cfg.minio_usuario.clone(),
            password: cfg.minio_password.clone(),
            bucket: BUCKET,
        })
    }

    /// El mismo cliente, en otro bucket.
    pub fn en(&self, bucket: &'static str) -> Self {
        Self { bucket, ..self.clone() }
    }

    pub async fn subir(&self, llave: &str, bytes: Vec<u8>, content_type: &str) -> Result<(), String> {
        let firma = self.firmar("PUT", llave, &sha256_hex(&bytes), Some(content_type));
        let resp = self
            .http
            .put(self.url(llave))
            .headers(firma)
            .header("content-type", content_type)
            .body(bytes)
            .send()
            .await
            .map_err(|e| format!("MinIO PUT {llave}: {e}"))?;
        revisar(resp, "PUT", llave).await.map(|_| ())
    }

    /// Sube un archivo del disco sin cargarlo en memoria.
    pub async fn subir_archivo(&self, llave: &str, ruta: &std::path::Path, content_type: &str) -> Result<(), String> {
        let archivo = tokio::fs::File::open(ruta).await.map_err(|e| format!("abrir {}: {e}", ruta.display()))?;
        let largo = archivo.metadata().await.map_err(|e| format!("leer {}: {e}", ruta.display()))?.len();
        let cuerpo = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(archivo));
        let firma = self.firmar("PUT", llave, SIN_FIRMA_CUERPO, Some(content_type));
        let resp = self
            .http
            .put(self.url(llave))
            .headers(firma)
            .header("content-type", content_type)
            .header("content-length", largo)
            .timeout(TIMEOUT_STREAM)
            .body(cuerpo)
            .send()
            .await
            .map_err(|e| format!("MinIO PUT {llave}: {e}"))?;
        revisar(resp, "PUT", llave).await.map(|_| ())
    }

    /// Baja en stream. `rango` es el `Range` que mandó el celular (`bytes=0-`), tal cual: va
    /// sin firmar, MinIO no lo pide en la firma. Un 404 regresa `Err` con "404" (archivo vencido).
    pub async fn bajar_stream(&self, llave: &str, rango: Option<&str>) -> Result<Descarga, String> {
        let firma = self.firmar("GET", llave, &sha256_hex(b""), None);
        let mut peticion = self.http.get(self.url(llave)).headers(firma).timeout(TIMEOUT_STREAM);
        if let Some(r) = rango {
            peticion = peticion.header("range", r);
        }
        let resp = peticion.send().await.map_err(|e| format!("MinIO GET {llave}: {e}"))?;
        let resp = revisar(resp, "GET", llave).await?;
        let encabezado = |nombre: &str| resp.headers().get(nombre).and_then(|v| v.to_str().ok()).map(str::to_string);
        Ok(Descarga {
            status: resp.status().as_u16(),
            largo: encabezado("content-length"),
            rango: encabezado("content-range"),
            tipo: encabezado("content-type"),
            cuerpo: resp,
        })
    }

    pub async fn bajar(&self, llave: &str) -> Result<Vec<u8>, String> {
        let firma = self.firmar("GET", llave, &sha256_hex(b""), None);
        let resp = self
            .http
            .get(self.url(llave))
            .headers(firma)
            .send()
            .await
            .map_err(|e| format!("MinIO GET {llave}: {e}"))?;
        let resp = revisar(resp, "GET", llave).await?;
        resp.bytes().await.map(|b| b.to_vec()).map_err(|e| format!("MinIO GET {llave}: {e}"))
    }

    /// Borrar algo que ya no existe no es error (S3 responde 204 igual).
    pub async fn borrar(&self, llave: &str) -> Result<(), String> {
        let firma = self.firmar("DELETE", llave, &sha256_hex(b""), None);
        let resp = self
            .http
            .delete(self.url(llave))
            .headers(firma)
            .send()
            .await
            .map_err(|e| format!("MinIO DELETE {llave}: {e}"))?;
        revisar(resp, "DELETE", llave).await.map(|_| ())
    }

    fn url(&self, llave: &str) -> String {
        format!("{}/{}/{llave}", self.endpoint, self.bucket)
    }

    /// Encabezados firmados (SigV4). Las llaves son `{cliente}/{trabajo}.pdf` o
    /// `{cliente}/{mensaje}/original.pdf`: solo caracteres que no hay que codificar en la ruta.
    fn firmar(&self, metodo: &str, llave: &str, sha_cuerpo: &str, content_type: Option<&str>) -> reqwest::header::HeaderMap {
        let fecha_hora = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let fecha = &fecha_hora[..8];
        let (nombres, encabezados) = match content_type {
            Some(ct) => (
                "content-type;host;x-amz-content-sha256;x-amz-date",
                format!("content-type:{ct}\nhost:{}\nx-amz-content-sha256:{sha_cuerpo}\nx-amz-date:{fecha_hora}\n", self.host),
            ),
            None => (
                "host;x-amz-content-sha256;x-amz-date",
                format!("host:{}\nx-amz-content-sha256:{sha_cuerpo}\nx-amz-date:{fecha_hora}\n", self.host),
            ),
        };
        let canonica = format!("{metodo}\n/{}/{llave}\n\n{encabezados}\n{nombres}\n{sha_cuerpo}", self.bucket);
        let alcance = format!("{fecha}/{REGION}/s3/aws4_request");
        let a_firmar = format!("AWS4-HMAC-SHA256\n{fecha_hora}\n{alcance}\n{}", sha256_hex(canonica.as_bytes()));
        let llave_firma = ["s3", "aws4_request"].iter().fold(
            hmac(hmac(format!("AWS4{}", self.password).as_bytes(), fecha.as_bytes()).as_slice(), REGION.as_bytes()),
            |k, parte| hmac(&k, parte.as_bytes()),
        );
        let firma = hex(&hmac(&llave_firma, a_firmar.as_bytes()));

        let mut h = reqwest::header::HeaderMap::new();
        let mut poner = |nombre: &'static str, valor: String| {
            if let Ok(v) = valor.parse() {
                h.insert(nombre, v);
            }
        };
        poner("x-amz-date", fecha_hora.clone());
        poner("x-amz-content-sha256", sha_cuerpo.to_string());
        poner(
            "authorization",
            format!("AWS4-HMAC-SHA256 Credential={}/{alcance},SignedHeaders={nombres},Signature={firma}", self.usuario),
        );
        h
    }
}

async fn revisar(resp: reqwest::Response, metodo: &str, llave: &str) -> Result<reqwest::Response, String> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let cuerpo = resp.text().await.unwrap_or_default();
    Err(format!("MinIO {metodo} {llave}: {status} {}", cuerpo.chars().take(300).collect::<String>()))
}

fn hmac(llave: &[u8], datos: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(llave).expect("HMAC acepta cualquier largo");
    mac.update(datos);
    mac.finalize().into_bytes().to_vec()
}

fn sha256_hex(datos: &[u8]) -> String {
    hex(&Sha256::digest(datos))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    /// Contra el MinIO de desarrollo (`.env`): `cargo test minio -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn subir_bajar_borrar() {
        dotenvy::dotenv().ok();
        let cfg = crate::config::Config {
            database_url: String::new(),
            ..crate::config::Config::from_env()
        };
        let m = super::Minio::desde_config(&cfg).expect("MINIO_USUARIO en .env");
        let llave = format!("prueba/{}.pdf", uuid::Uuid::new_v4());
        m.subir(&llave, b"%PDF-prueba".to_vec(), "application/pdf").await.unwrap();
        assert_eq!(m.bajar(&llave).await.unwrap(), b"%PDF-prueba");
        m.borrar(&llave).await.unwrap();
        assert!(m.bajar(&llave).await.unwrap_err().contains("404"));
        m.borrar(&llave).await.unwrap();   // borrar lo que ya no existe no es error
    }

    /// Lo mismo con el archivo en stream y un `Range`: `cargo test minio -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn subir_archivo_y_bajar_rango() {
        dotenvy::dotenv().ok();
        let cfg = crate::config::Config {
            database_url: String::new(),
            ..crate::config::Config::from_env()
        };
        let m = super::Minio::desde_config(&cfg).expect("MINIO_USUARIO en .env").en(super::BUCKET_CHAT);
        let ruta = std::env::temp_dir().join(format!("xplaya-prueba-{}", uuid::Uuid::new_v4()));
        std::fs::write(&ruta, b"0123456789").unwrap();
        let llave = format!("prueba/{}/original.bin", uuid::Uuid::new_v4());
        m.subir_archivo(&llave, &ruta, "application/octet-stream").await.unwrap();
        std::fs::remove_file(&ruta).unwrap();

        let d = m.bajar_stream(&llave, Some("bytes=2-5")).await.unwrap();
        assert_eq!(d.status, 206);
        assert_eq!(d.rango.as_deref(), Some("bytes 2-5/10"));
        assert_eq!(d.cuerpo.bytes().await.unwrap().as_ref(), b"2345");
        m.borrar(&llave).await.unwrap();
        assert!(m.bajar_stream(&llave, None).await.err().unwrap().contains("404"));
    }
}
