use sqlx::PgPool;
use uuid::Uuid;

use crate::models::pedido::PedidoItemRequest;

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

    // Usuario anónimo configurado en Settings para órdenes online.
    // Si no existe la clave, usamos UUID vacío (todo ceros).
    let anonymous_id = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'ID_XPLAYA.COM_ANONYMOUS_USER'",
    )
    .fetch_optional(&mut *tx)
    .await?
    .and_then(|v| Uuid::parse_str(&v).ok())
    .unwrap_or(Uuid::nil());

    // Insertar pedido con Estatus=0 (Nuevo) y Origen=1 (EnLinea)
    let pedido_uid = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pedidos (uid, fechacreado, clienteid, userupdatedid, estatus, origen)
         VALUES ($1, NOW(), $2, $3, 0, 1)",
    )
    .bind(pedido_uid)
    .bind(cliente_id)
    .bind(anonymous_id)
    .execute(&mut *tx)
    .await?;

    for item in items {
        sqlx::query(
            "INSERT INTO pedidoitems (pedidoid, productoid, cantidad)
             VALUES ($1, $2, $3)",
        )
        .bind(pedido_uid)
        .bind(item.producto_id)
        .bind(item.cantidad)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok((pedido_uid, cliente_id))
}
