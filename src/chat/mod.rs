//! Chat de la app con la papelería. Diseño completo en `CHAT.md`.
//!
//! - `contrato`: el formato v1 de los mensajes y los temas de NATS (puro).
//! - `reglas`: qué hacer con cada evento (puro, con pruebas).
//! - `consumer`: lee el stream `CHAT` y guarda en `Mensajes` (adaptador).
//!
//! Las consultas a la tabla están en `db/chat.rs`.

pub mod consumer;
pub mod contrato;
pub mod reglas;

pub use consumer::vigilar;
