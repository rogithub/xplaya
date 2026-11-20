//! `Clientes.NombreChat` y `Clientes.FotoPerfil` (`perfil.rs`). Solo los escribe xplaya.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Perfil {
    /// El de la caja (`Clientes.Nombre`): de sugerencia si no ha puesto el suyo.
    pub nombre: String,
    pub nombrechat: Option<String>,
    /// Llave en `papeleria-perfiles`.
    pub fotoperfil: Option<String>,
}

pub async fn de_cliente(pool: &PgPool, cliente: Uuid) -> Result<Option<Perfil>, sqlx::Error> {
    sqlx::query_as("SELECT nombre, nombrechat, fotoperfil FROM clientes WHERE id = $1")
        .bind(cliente)
        .fetch_optional(pool)
        .await
}

pub async fn guardar_nombre(pool: &PgPool, cliente: Uuid, nombre: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE clientes SET nombrechat = $2 WHERE id = $1")
        .bind(cliente)
        .bind(nombre)
        .execute(pool)
        .await?;
    Ok(())
}

/// Pone la foto nueva (`None`: la quita) y devuelve la llave de la anterior, para borrarla.
pub async fn guardar_foto(pool: &PgPool, cliente: Uuid, llave: Option<&str>) -> Result<Option<String>, sqlx::Error> {
    let anterior: Option<(Option<String>,)> = sqlx::query_as(
        "UPDATE clientes c SET fotoperfil = $2
         FROM (SELECT id, fotoperfil FROM clientes WHERE id = $1 FOR UPDATE) antes
         WHERE c.id = antes.id
         RETURNING antes.fotoperfil",
    )
    .bind(cliente)
    .bind(llave)
    .fetch_optional(pool)
    .await?;
    Ok(anterior.and_then(|(a,)| a))
}
