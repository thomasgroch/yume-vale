//! SQL repository — portable queries with `$N` bind parameters.
//!
//! All functions are `async` and take an `&Pool<Any>`. No SQLx macros are used
//! so the crate compiles without a running database.

pub(crate) mod bond;
pub(crate) mod identity;
pub(crate) mod inventory;
pub(crate) mod plot;

pub(crate) use bond::*;
pub(crate) use identity::*;
pub(crate) use inventory::*;
pub(crate) use plot::*;

use sqlx::pool::PoolConnection;
use sqlx::{Any, Pool};

use crate::{CommandResult, PersistenceError};

// ---------------------------------------------------------------------------
// Migrations
// ---------------------------------------------------------------------------

/// SQL statements for all tables, executed in order inside a single
/// migration transaction.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS identities (
        token_hash TEXT    NOT NULL PRIMARY KEY,
        player_id  INTEGER NOT NULL,
        created_at INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS inventory (
        player_id     INTEGER NOT NULL,
        resource_kind TEXT    NOT NULL,
        quantity      INTEGER NOT NULL,
        PRIMARY KEY (player_id, resource_kind)
    )",
    "CREATE TABLE IF NOT EXISTS creature_bond (
        player_id     INTEGER NOT NULL,
        creature_kind TEXT    NOT NULL,
        bond_level    INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (player_id, creature_kind)
    )",
    "CREATE TABLE IF NOT EXISTS plot_assignment (
        slot_index INTEGER NOT NULL,
        player_id  INTEGER NOT NULL UNIQUE,
        PRIMARY KEY (slot_index)
    )",
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
// Error mapping
// ---------------------------------------------------------------------------

/// Map an `sqlx::Error` to a [`PersistenceError`].
///
/// Checks for constraint-violation patterns (UNIQUE, FOREIGN KEY, NOT NULL)
/// and routes them to `PersistenceError::Constraint`; everything else goes
/// to `PersistenceError::Database`.
pub(crate) fn map_error(e: sqlx::Error) -> PersistenceError {
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
