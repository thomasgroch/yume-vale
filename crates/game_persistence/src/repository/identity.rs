//! Identity queries: find-or-create and explicit creation.

use sqlx::{Any, Pool};

use crate::{CommandResult, IdentityRow, PersistenceError, repository::map_error};

pub(crate) async fn resolve_identity(
    pool: &Pool<Any>,
    token_hash: &str,
) -> Result<CommandResult, PersistenceError> {
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
