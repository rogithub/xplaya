use serde::Serialize;

/// Tile de navegación del kiosko — una FamiliaSemantica con su conteo
/// de productos visibles.
#[derive(Serialize, sqlx::FromRow)]
pub struct FamiliaTile {
    pub id: uuid::Uuid,
    pub nombre: String,
    pub total_productos: i64,
}
