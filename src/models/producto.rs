use serde::Serialize;

#[derive(Serialize)]
pub struct ProductoCard {
    pub nid: i32,
    pub nombre: String,
    pub categoria: String,
    pub precio_venta: String,
    pub unidad_medida: String,
    pub foto_url: Option<String>,
}

#[derive(Serialize)]
pub struct Paginacion {
    pub total_items: i64,
    pub pagina_actual: i32,
    pub total_paginas: i32,
}

#[derive(Serialize)]
pub struct ProductoDetalle {
    pub nid: i32,
    pub nombre: String,
    pub categoria: String,
    pub precio_venta: String,
    pub unidad_medida: String,
    pub stock: String,
    pub fotos: Vec<String>,
    pub videos: Vec<String>,
}
