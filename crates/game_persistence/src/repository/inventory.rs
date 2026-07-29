//! Inventory queries: full-replace save and load.

use sqlx::pool::PoolConnection;
use sqlx::{Any, Pool};

use crate::{CommandResult, InventoryRow, PersistenceError, repository::map_error};

pub(crate) async fn save_inventory(
    pool: &Pool<Any>,
    player_id: i64,
    items: &[InventoryRow],
) -> Result<CommandResult, PersistenceError> {
    let mut conn: PoolConnection<Any> = pool
        .acquire()
        .await
        .map_err(|e| PersistenceError::Database(e.to_string()))?;

    sqlx::query("DELETE FROM inventory WHERE player_id = $1")
        .bind(player_id)
        .execute(&mut *conn)
        .await
        .map_err(map_error)?;

    for item in items {
        sqlx::query(
            "INSERT INTO inventory (player_id, resource_kind, quantity) VALUES ($1, $2, $3)",
        )
        .bind(player_id)
        .bind(&item.resource_kind)
        .bind(item.quantity)
        .execute(&mut *conn)
        .await
        .map_err(map_error)?;
    }

    Ok(CommandResult::InventorySaved)
}

pub(crate) async fn load_inventory(
    pool: &Pool<Any>,
    player_id: i64,
) -> Result<CommandResult, PersistenceError> {
    let rows: Vec<(String, i32)> = sqlx::query_as(
        "SELECT resource_kind, quantity FROM inventory WHERE player_id = $1 ORDER BY resource_kind",
    )
    .bind(player_id)
    .fetch_all(pool)
    .await
    .map_err(map_error)?;

    Ok(CommandResult::Inventory(
        rows.into_iter()
            .map(|(kind, qty)| InventoryRow {
                resource_kind: kind,
                quantity: qty,
            })
            .collect(),
    ))
}
