//! Plot queries: assignment and decoration upsert + load.

use sqlx::{Any, Pool};

use crate::{
    CommandResult, PersistenceError, PlotAssignmentRow, PlotDecorationRow, repository::map_error,
};

pub(crate) async fn save_plot_assignment(
    pool: &Pool<Any>,
    slot_index: i64,
    player_id: i64,
) -> Result<CommandResult, PersistenceError> {
    let updated = sqlx::query("UPDATE plot_assignment SET player_id = $1 WHERE slot_index = $2")
        .bind(player_id)
        .bind(slot_index)
        .execute(pool)
        .await
        .map_err(map_error)?;

    if updated.rows_affected() == 0 {
        sqlx::query("INSERT INTO plot_assignment (slot_index, player_id) VALUES ($1, $2)")
            .bind(slot_index)
            .bind(player_id)
            .execute(pool)
            .await
            .map_err(map_error)?;
    }

    Ok(CommandResult::PlotAssignmentSaved)
}

pub(crate) async fn load_plot_assignment(
    pool: &Pool<Any>,
    player_id: i64,
) -> Result<CommandResult, PersistenceError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT slot_index FROM plot_assignment WHERE player_id = $1")
            .bind(player_id)
            .fetch_optional(pool)
            .await
            .map_err(map_error)?;

    Ok(CommandResult::PlotAssignment(
        row.map(|(slot_index,)| PlotAssignmentRow { slot_index }),
    ))
}

pub(crate) async fn save_plot_decoration(
    pool: &Pool<Any>,
    player_id: i64,
    decoration: &PlotDecorationRow,
) -> Result<CommandResult, PersistenceError> {
    let updated = sqlx::query(
        "UPDATE plot_decoration SET position_x = $1, position_z = $2, rotation_y = $3 \
         WHERE player_id = $4 AND decoration_id = $5",
    )
    .bind(decoration.position_x)
    .bind(decoration.position_z)
    .bind(decoration.rotation_y)
    .bind(player_id)
    .bind(decoration.decoration_id)
    .execute(pool)
    .await
    .map_err(map_error)?;

    if updated.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO plot_decoration \
             (player_id, decoration_id, position_x, position_z, rotation_y) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(player_id)
        .bind(decoration.decoration_id)
        .bind(decoration.position_x)
        .bind(decoration.position_z)
        .bind(decoration.rotation_y)
        .execute(pool)
        .await
        .map_err(map_error)?;
    }

    Ok(CommandResult::PlotDecorationSaved)
}

pub(crate) async fn load_plot_decorations(
    pool: &Pool<Any>,
    player_id: i64,
) -> Result<CommandResult, PersistenceError> {
    let rows: Vec<(i64, f64, f64, f64)> = sqlx::query_as(
        "SELECT decoration_id, position_x, position_z, rotation_y \
         FROM plot_decoration WHERE player_id = $1 ORDER BY decoration_id",
    )
    .bind(player_id)
    .fetch_all(pool)
    .await
    .map_err(map_error)?;

    Ok(CommandResult::PlotDecorations(
        rows.into_iter()
            .map(|(id, x, z, rot)| PlotDecorationRow {
                decoration_id: id,
                position_x: x,
                position_z: z,
                rotation_y: rot,
            })
            .collect(),
    ))
}
