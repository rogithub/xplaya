use sqlx::PgPool;
use uuid::Uuid;

use crate::models::pedido::PedidoItemRequest;

/// Pedido de la web pública: crea o reutiliza el cliente por teléfono.
/// Origen=1 (EnLinea) — hay que contactar al cliente para coordinar.
pub async fn crear(
    pool: &PgPool,
    nombre: &str,
    telefono: &str,
    items: &[PedidoItemRequest],
) -> Result<(Uuid, Uuid), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Buscar cliente existente por los últimos 10 dígitos del teléfono.
    // La BD puede tener números con prefijos (+52, 01), por eso usamos right().
    let cliente_id = {
        let existente = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM clientes WHERE right(telefono, 10) = $1 LIMIT 1",
        )
        .bind(telefono)
        .fetch_optional(&mut *tx)
        .await?;

        match existente {
            Some(id) => id,
            None => {
                sqlx::query_scalar::<_, Uuid>(
                    "INSERT INTO clientes (id, telefono, nombre, fechacreado)
                     VALUES (gen_random_uuid(), $1, $2, NOW())
                     RETURNING id",
                )
                .bind(telefono)
                .bind(nombre)
                .fetch_one(&mut *tx)
                .await?
            }
        }
    };

    let pedido_uid = insertar_pedido(&mut tx, cliente_id, items, 1).await?;
    tx.commit().await?;

    Ok((pedido_uid, cliente_id))
}

/// Pedido del kiosko en tienda: sin datos del cliente — el pedido queda a nombre
/// del cliente de sistema ID_CLIENTE_KIOSKO (Settings) y el vendedor asigna el
/// cliente real al cobrar en el POS. Origen=0 (Tienda) — el cliente está presente.
/// Devuelve None si la setting no existe (la BD no está preparada).
pub async fn crear_kiosko(
    pool: &PgPool,
    items: &[PedidoItemRequest],
) -> Result<Option<Uuid>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let cliente_kiosko = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'ID_CLIENTE_KIOSKO'",
    )
    .fetch_optional(&mut *tx)
    .await?
    .and_then(|v| Uuid::parse_str(&v).ok());

    let Some(cliente_id) = cliente_kiosko else {
        return Ok(None);
    };

    let pedido_uid = insertar_pedido(&mut tx, cliente_id, items, 0).await?;
    tx.commit().await?;

    Ok(Some(pedido_uid))
}

/// INSERT común de pedido + items. `origen`: 0=Tienda (kiosko), 1=EnLinea (web).
async fn insertar_pedido(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cliente_id: Uuid,
    items: &[PedidoItemRequest],
    origen: i32,
) -> Result<Uuid, sqlx::Error> {
    // Usuario anónimo configurado en Settings para órdenes online.
    // Si no existe la clave, usamos UUID vacío (todo ceros).
    let anonymous_id = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'ID_XPLAYA.COM_ANONYMOUS_USER'",
    )
    .fetch_optional(&mut **tx)
    .await?
    .and_then(|v| Uuid::parse_str(&v).ok())
    .unwrap_or(Uuid::nil());

    // Estatus=0 (Nuevo)
    let pedido_uid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pedidos (uid, fechacreado, clienteid, userupdatedid, estatus, origen)
         VALUES ($1, NOW(), $2, $3, 0, $4)",
    )
    .bind(pedido_uid)
    .bind(cliente_id)
    .bind(anonymous_id)
    .bind(origen)
    .execute(&mut **tx)
    .await?;

    for item in items {
        sqlx::query(
            "INSERT INTO pedidoitems (pedidoid, productoid, cantidad, presentacionid)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(pedido_uid)
        .bind(item.producto_id)
        .bind(item.cantidad)
        .bind(item.presentacion_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(pedido_uid)
}
