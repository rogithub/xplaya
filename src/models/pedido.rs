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
