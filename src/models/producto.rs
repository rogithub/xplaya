use serde::Serialize;

#[derive(Serialize)]
pub struct Presentacion {
    pub id: uuid::Uuid,
    pub nombre: String,
    pub factor: String,
    pub precio_venta: String,
}

#[derive(Serialize)]
pub struct ProductoCard {
    pub nid: i32,
    pub nombre: String,
    pub categoria: String,
    pub precio_venta: String,
    pub unidad_medida: String,
    pub foto_url: Option<String>,
    pub presentaciones: Vec<Presentacion>,
}

#[derive(Serialize)]
pub struct Paginacion {
    pub total_items: i64,
    pub pagina_actual: i32,
    pub total_paginas: i32,
    /// Números de página a mostrar. None = puntos suspensivos (…).
    /// Serializa como null en JSON → falsy en Minijinja.
    pub paginas: Vec<Option<i32>>,
}

impl Paginacion {
    pub fn new(total_items: i64, pagina_actual: i32, total_paginas: i32) -> Self {
        let paginas = Self::calcular(pagina_actual, total_paginas);
        Self { total_items, pagina_actual, total_paginas, paginas }
    }

    fn calcular(actual: i32, total: i32) -> Vec<Option<i32>> {
        if total <= 1 {
            return vec![];
        }
        // Con 7 páginas o menos caben todas sin elipsis
        if total <= 7 {
            return (1..=total).map(Some).collect();
        }
        // Ventana de ±1 alrededor de la página actual, acotada a [2, total-1]
        // para no solapar con las páginas extremas que siempre se muestran.
        let win_start = (actual - 1).max(2);
        let win_end = (actual + 1).min(total - 1);

        let mut paginas: Vec<Option<i32>> = vec![Some(1)];
        if win_start > 2 {
            paginas.push(None); // …
        }
        for n in win_start..=win_end {
            paginas.push(Some(n));
        }
        if win_end < total - 1 {
            paginas.push(None); // …
        }
        paginas.push(Some(total));
        paginas
    }
}

#[derive(Serialize)]
pub struct ComponenteKit {
    pub nombre: String,
    pub cantidad: String,
}

#[derive(Serialize)]
pub struct ProductoDetalle {
    pub nid: i32,
    pub id: uuid::Uuid,
    pub nombre: String,
    pub categoria: String,
    pub precio_venta: String,
    pub unidad_medida: String,
    pub stock: String,
    pub fotos: Vec<String>,
    pub videos: Vec<String>,
    pub presentaciones: Vec<Presentacion>,
    pub componentes: Vec<ComponenteKit>,
    pub marca: Option<String>,
    pub modelo: Option<String>,
    pub descripcion: Option<String>,
}
