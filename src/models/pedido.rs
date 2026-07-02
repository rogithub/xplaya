use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CrearPedidoRequest {
    pub nombre: String,
    pub telefono: String,
    pub items: Vec<PedidoItemRequest>,
}

#[derive(Deserialize)]
pub struct PedidoItemRequest {
    pub producto_id: Uuid,
    pub cantidad: Decimal,
    pub presentacion_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct PedidoCreadoResponse {
    pub pedido_uid: Uuid,
    pub cliente_id: Uuid,
}

/// Pedido desde el kiosko — sin datos del cliente: el vendedor los captura al
/// cobrar en el POS. La autorización viaja en la cookie HttpOnly, no en el body.
#[derive(Deserialize)]
pub struct KioskoPedidoRequest {
    pub items: Vec<PedidoItemRequest>,
}

#[derive(Serialize)]
pub struct KioskoPedidoResponse {
    pub pedido_uid: Uuid,
}
