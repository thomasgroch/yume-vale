//! Creature-bond queries: upsert and load.

use sqlx::{Any, Pool};

use crate::{CommandResult, CreatureBondRow, PersistenceError, repository::map_error};

pub(crate) async fn save_creature_bond(
    pool: &Pool<Any>,
    player_id: i64,
    creature_kind: &str,
    bond_level: i32,
) -> Result<CommandResult, PersistenceError> {
    let updated = sqlx::query(
        "UPDATE creature_bond SET bond_level = $1 \
         WHERE player_id = $2 AND creature_kind = $3",
    )
    .bind(bond_level)
    .bind(player_id)
    .bind(creature_kind)
    .execute(pool)
    .await
    .map_err(map_error)?;

    if updated.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO creature_bond (player_id, creature_kind, bond_level) \
             VALUES ($1, $2, $3)",
        )
        .bind(player_id)
        .bind(creature_kind)
        .bind(bond_level)
        .execute(pool)
        .await
        .map_err(map_error)?;
    }

    Ok(CommandResult::CreatureBondSaved)
}

pub(crate) async fn load_creature_bond(
    pool: &Pool<Any>,
    player_id: i64,
    creature_kind: &str,
) -> Result<CommandResult, PersistenceError> {
    let row: Option<(i32,)> = sqlx::query_as(
        "SELECT bond_level FROM creature_bond \
         WHERE player_id = $1 AND creature_kind = $2",
    )
    .bind(player_id)
    .bind(creature_kind)
    .fetch_optional(pool)
    .await
    .map_err(map_error)?;

    Ok(CommandResult::CreatureBond(row.map(|(bond_level,)| {
        CreatureBondRow {
            creature_kind: creature_kind.to_owned(),
            bond_level,
        }
    })))
}
