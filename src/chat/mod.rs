//! Chat de la app con la papelería. Diseño completo en `CHAT.md`.
//!
//! - `archivos`: qué es cada archivo que manda el cliente, cuánto pesa y su llave en MinIO (puro).
//! - `contrato`: el formato v1 de los mensajes y los temas de NATS (puro).
//! - `reglas`: qué hacer con cada evento (puro, con pruebas).
//! - `consumer`: lee el stream `CHAT` y guarda en `Mensajes` (adaptador).
//! - `nats`: publica lo que hace el cliente y reparte los cambios en vivo a la app (adaptador).
//! - `sin_contestar`: avisa por ntfy a la papelería si nadie lee un mensaje en la caja (adaptador).
//! - `subidas`: archivos ya subidos que esperan su mensaje (en memoria).
//! - `vista`: los mensajes y eventos como los recibe la app (JSON).
//!
//! Las consultas a la tabla están en `db/chat.rs`.

pub mod archivos;
pub mod consumer;
pub mod contrato;
pub mod nats;
pub mod reglas;
pub mod sin_contestar;
pub mod subidas;
pub mod tarjetas;
pub mod vista;

pub use consumer::vigilar;
pub use nats::Chat;
pub use subidas::Subidas;
