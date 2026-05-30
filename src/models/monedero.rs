use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct Terminos {
    pub dias_vigencia: i32,
    pub porcentaje: String,
    pub fecha_actualizacion: String,
}

#[derive(Serialize)]
pub struct VentaRecibo {
    pub id: Uuid,
    pub fecha: String,
    pub hora: String,
    pub cliente_id: Option<Uuid>,
    pub total: String,
    pub pago: String,
    pub pago_monedero: String,
    pub pago_tarjeta: String,
    pub pago_transferencia: String,
    pub pago_dolares: String,
    pub tipo_cambio_dolares: String,
    pub cambio: String,
    pub productos: Vec<ProductoRecibo>,
    pub monedero_generado: String,
    pub saldo_monedero: String,
    pub saldo_anterior: String,
    pub tiene_monedero: bool,
    pub hay_actividad_monedero: bool,
}

#[derive(Serialize)]
pub struct ProductoRecibo {
    pub nombre: String,
    pub cantidad: String,
    pub precio_unitario: String,
    pub subtotal: String,
    pub es_ingreso_trasladado: bool,
}

#[derive(Serialize)]
pub struct Cotizacion {
    pub uid: Uuid,
    pub fecha: String,
    pub hora: String,
    pub cliente_id: Option<Uuid>,
    pub tiene_monedero: bool,
    pub productos: Vec<ProductoCotizacion>,
    pub total: String,
    pub monedero_potencial: String,
    pub porcentaje_monedero: String,
}

#[derive(Serialize)]
pub struct ProductoCotizacion {
    pub nombre: String,
    pub cantidad: String,
    pub precio: String,
    pub subtotal: String,
}

#[derive(Serialize)]
pub struct ClienteMonedero {
    pub nombre: String,
    pub miembro_desde: String,
    pub saldo: String,
    pub historial: Vec<HistorialEntry>,
    pub alerta: Option<AlertaVencimiento>,
}

#[derive(Serialize)]
pub struct HistorialEntry {
    pub ajuste_id: Uuid,
    pub fecha: String,
    pub dinero_generado: String,
    pub dinero_gastado: String,
    pub disponible_neto: String,
    pub fecha_expiracion: String,
    pub estado: String,
}

#[derive(Serialize)]
pub struct AlertaVencimiento {
    pub monto: String,
    pub fecha: String,
}
