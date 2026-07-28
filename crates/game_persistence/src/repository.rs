//! SQL repository — portable queries with `$N` bind parameters.
//!
//! All functions are `async` and take an `&Pool<Any>`. No SQLx macros are used
//! so the crate compiles without a running database.

use sqlx::pool::PoolConnection;
use sqlx::{Any, Pool};

use crate::{
    CommandResult, CreatureBondRow, IdentityRow, InventoryRow, PersistenceError, PlotAssignmentRow,
    PlotDecorationRow, QuestProgressRow,
};

// ---------------------------------------------------------------------------
// Migrations
// ---------------------------------------------------------------------------

/// SQL statements for all tables, executed in order inside a single
/// migration transaction.
const MIGRATIONS: &[&str] = &[
    // 1. identities
    "CREATE TABLE IF NOT EXISTS identities (
        token_hash TEXT    NOT NULL PRIMARY KEY,
        player_id  INTEGER NOT NULL,
        created_at INTEGER NOT NULL
    )",
    // 2. inventory
    "CREATE TABLE IF NOT EXISTS inventory (
        player_id     INTEGER NOT NULL,
        resource_kind TEXT    NOT NULL,
        quantity      INTEGER NOT NULL,
        PRIMARY KEY (player_id, resource_kind)
    )",
    // 3. quest_progress
    "CREATE TABLE IF NOT EXISTS quest_progress (
        player_id INTEGER NOT NULL,
        quest_id  INTEGER NOT NULL,
        progress  REAL    NOT NULL DEFAULT 0.0,
        completed INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (player_id, quest_id)
    )",
    // 4. creature_bond
    "CREATE TABLE IF NOT EXISTS creature_bond (
        player_id     INTEGER NOT NULL,
        creature_kind TEXT    NOT NULL,
        bond_level    INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (player_id, creature_kind)
    )",
    // 5. plot_assignment
    "CREATE TABLE IF NOT EXISTS plot_assignment (
        slot_index INTEGER NOT NULL,
        player_id  INTEGER NOT NULL UNIQUE,
        PRIMARY KEY (slot_index)
    )",
    // 6. plot_decoration
    "CREATE TABLE IF NOT EXISTS plot_decoration (
        player_id     INTEGER NOT NULL,
        decoration_id INTEGER NOT NULL,
        position_x    REAL    NOT NULL,
        position_z    REAL    NOT NULL,
        rotation_y    REAL    NOT NULL DEFAULT 0.0,
        PRIMARY KEY (player_id, decoration_id)
    )",
];

/// Run all pending migrations.
pub(crate) async fn migrate(pool: &Pool<Any>) -> Result<CommandResult, PersistenceError> {
    let mut conn: PoolConnection<Any> = pool
        .acquire()
        .await
        .map_err(|e| PersistenceError::Database(e.to_string()))?;

    for (i, sql) in MIGRATIONS.iter().enumerate() {
        sqlx::query(sql)
            .execute(&mut *conn)
            .await
            .map_err(|e| PersistenceError::Database(format!("migration {i} failed: {e}")))?;
    }

    tracing::info!("database migrations applied ({})", MIGRATIONS.len());
    Ok(CommandResult::Migrated)
}

// ---------------------------------------------------------------------------
// Identities
// ---------------------------------------------------------------------------

pub(crate) async fn resolve_identity(
    pool: &Pool<Any>,
    token_hash: &str,
) -> Result<CommandResult, PersistenceError> {
    // Try to find existing identity.
    let existing: Option<(i64, i64)> =
        sqlx::query_as("SELECT player_id, created_at FROM identities WHERE token_hash = $1")
            .bind(token_hash)
            .fetch_optional(pool)
            .await
            .map_err(map_error)?;

    if let Some((player_id, created_at)) = existing {
        return Ok(CommandResult::Identity(IdentityRow {
            player_id,
            created_at,
        }));
    }

    // Create new identity: auto-assign the next available player_id.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let new_id: i64 = {
        let max: (i64,) =
            sqlx::query_as::<_, (i64,)>("SELECT COALESCE(MAX(player_id), 0) FROM identities")
                .fetch_one(pool)
                .await
                .map_err(map_error)?;
        max.0 + 1
    };

    sqlx::query("INSERT INTO identities (token_hash, player_id, created_at) VALUES ($1, $2, $3)")
        .bind(token_hash)
        .bind(new_id)
        .bind(now)
        .execute(pool)
        .await
        .map_err(map_error)?;

    tracing::debug!(
        "created identity: token_hash={}, player_id={}",
        &token_hash[..8.min(token_hash.len())],
        new_id
    );

    Ok(CommandResult::Identity(IdentityRow {
        player_id: new_id,
        created_at: now,
    }))
}

pub(crate) async fn create_identity(
    pool: &Pool<Any>,
    token_hash: &str,
    player_id: i64,
) -> Result<CommandResult, PersistenceError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    sqlx::query("INSERT INTO identities (token_hash, player_id, created_at) VALUES ($1, $2, $3)")
        .bind(token_hash)
        .bind(player_id)
        .bind(now)
        .execute(pool)
        .await
        .map_err(map_error)?;

    Ok(CommandResult::IdentityCreated)
}

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

pub(crate) async fn save_inventory(
    pool: &Pool<Any>,
    player_id: i64,
    items: &[InventoryRow],
) -> Result<CommandResult, PersistenceError> {
    let mut conn: PoolConnection<Any> = pool
        .acquire()
        .await
        .map_err(|e| PersistenceError::Database(e.to_string()))?;

    // Full replace: delete old rows, insert new ones.
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

// ---------------------------------------------------------------------------
// Quest progress
// ---------------------------------------------------------------------------

pub(crate) async fn save_quest_progress(
    pool: &Pool<Any>,
    player_id: i64,
    quest_id: i64,
    progress: f64,
    completed: bool,
) -> Result<CommandResult, PersistenceError> {
    let completed_int: i32 = if completed { 1 } else { 0 };

    // UPSERT: Try UPDATE first; if no rows affected, INSERT.
    let updated = sqlx::query(
        "UPDATE quest_progress SET progress = $1, completed = $2 \
         WHERE player_id = $3 AND quest_id = $4",
    )
    .bind(progress)
    .bind(completed_int)
    .bind(player_id)
    .bind(quest_id)
    .execute(pool)
    .await
    .map_err(map_error)?;

    if updated.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO quest_progress (player_id, quest_id, progress, completed) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(player_id)
        .bind(quest_id)
        .bind(progress)
        .bind(completed_int)
        .execute(pool)
        .await
        .map_err(map_error)?;
    }

    Ok(CommandResult::QuestProgressSaved)
}

pub(crate) async fn load_quest_progress(
    pool: &Pool<Any>,
    player_id: i64,
    quest_id: i64,
) -> Result<CommandResult, PersistenceError> {
    let row: Option<(f64, i32)> = sqlx::query_as(
        "SELECT progress, completed FROM quest_progress \
         WHERE player_id = $1 AND quest_id = $2",
    )
    .bind(player_id)
    .bind(quest_id)
    .fetch_optional(pool)
    .await
    .map_err(map_error)?;

    Ok(CommandResult::QuestProgress(row.map(
        |(progress, completed)| QuestProgressRow {
            quest_id,
            progress,
            completed: completed != 0,
        },
    )))
}

// ---------------------------------------------------------------------------
// Creature bond
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Plot assignment
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Plot decoration
// ---------------------------------------------------------------------------

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
            "INSERT INTO plot_decoration (player_id, decoration_id, position_x, position_z, rotation_y) \
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

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Map an `sqlx::Error` to a [`PersistenceError`].
///
/// Checks for constraint-violation patterns (UNIQUE, FOREIGN KEY, NOT NULL)
/// and routes them to `PersistenceError::Constraint`; everything else goes
/// to `PersistenceError::Database`.
fn map_error(e: sqlx::Error) -> PersistenceError {
    match &e {
        sqlx::Error::Database(db_err) => {
            let msg = db_err.message().to_string();
            let lower = msg.to_lowercase();
            if lower.contains("unique")
                || lower.contains("foreign key")
                || lower.contains("not null")
                || lower.contains("primary key")
                || lower.contains("constraint")
            {
                PersistenceError::Constraint(msg)
            } else {
                PersistenceError::Database(msg)
            }
        }
        _ => PersistenceError::Database(e.to_string()),
    }
}
