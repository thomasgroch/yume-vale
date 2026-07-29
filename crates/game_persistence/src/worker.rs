//! Dedicated worker thread with its own Tokio runtime.
//!
//! The worker receives commands from a bounded synchronous channel, dispatches
//! them to the async repository, and sends results back via one-shot channels.

use std::sync::mpsc;
use std::thread;

use sqlx::any::AnyPoolOptions;
use sqlx::{Any, Pool};

use crate::repository;
use crate::{Command, CommandKind, PersistenceError, PersistenceHandle};

/// Default capacity of the bounded command channel.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// A persistence worker running on a dedicated thread.
///
/// Spawn this once at application startup. The worker runs a Tokio runtime
/// on a separate `std::thread` and communicates via bounded channels. All
/// SQL operations are sequential (single consumer) which avoids connection
/// pool contention.
///
/// # Shutdown
///
/// Dropping the worker signals the thread to stop and then joins it. You may
/// also call [`shutdown`](Self::shutdown) explicitly for a synchronous wait.
pub struct PersistenceWorker {
    handle: Option<PersistenceHandle>,
    join_handle: Option<thread::JoinHandle<()>>,
    db_url: String,
    capacity: usize,
}

impl PersistenceWorker {
    /// Spawn a new persistence worker.
    ///
    /// `db_url` must be a valid SQLx connection string:
    /// - SQLite: `sqlite:data/yume-vale.db`
    /// - PostgreSQL: `postgres://user:password@host/db`
    ///
    /// `capacity` controls the maximum number of buffered commands. When the
    /// queue is full, callers receive [`PersistenceError::QueueFull`].
    pub fn spawn(db_url: &str, capacity: usize) -> Result<Self, PersistenceError> {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<Command>(capacity);

        let db_url_for_thread = db_url.to_owned();

        let join_handle = thread::Builder::new()
            .name("yume-persistence".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();

                let rt = match rt {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("failed to build persistence runtime: {e}");
                        eprintln!("[persistence] runtime build error: {e}");
                        return;
                    }
                };

                if let Err(e) = rt.block_on(run_worker(&db_url_for_thread, &cmd_rx)) {
                    tracing::error!("persistence worker exited: {e}");
                    // Also print to stderr in case tracing subscriber is not set up.
                    eprintln!("[persistence] worker error: {e}");
                }
            })
            .map_err(|e| PersistenceError::Database(format!("failed to spawn thread: {e}")))?;

        Ok(Self {
            handle: Some(PersistenceHandle::new(cmd_tx, capacity)),
            join_handle: Some(join_handle),
            db_url: db_url.to_owned(),
            capacity,
        })
    }

    /// Get a handle for sending commands to this worker.
    pub fn handle(&self) -> &PersistenceHandle {
        self.handle
            .as_ref()
            .expect("PersistenceWorker handle already taken")
    }

    /// Explicitly shut down the worker and wait for the thread to join.
    pub fn shutdown(&mut self) -> Result<(), PersistenceError> {
        // Drop the handle (closes the sender channel).
        self.handle.take();
        self.join()
    }

    /// Join the worker thread (internal).
    fn join(&mut self) -> Result<(), PersistenceError> {
        if let Some(handle) = self.join_handle.take() {
            handle.join().map_err(|_| {
                PersistenceError::Database("persistence worker thread panicked".into())
            })?;
        }
        Ok(())
    }

    /// The database URL this worker was spawned with.
    pub fn db_url(&self) -> &str {
        &self.db_url
    }

    /// The channel capacity this worker was spawned with.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Drop for PersistenceWorker {
    fn drop(&mut self) {
        // Signal shutdown by dropping the handle (closes the sender).
        self.handle.take();
        // Ignore join errors during drop — thread may already be gone.
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Worker loop: connect to the database and process commands.
async fn run_worker(
    db_url: &str,
    cmd_rx: &mpsc::Receiver<Command>,
) -> Result<(), PersistenceError> {
    sqlx::any::install_default_drivers();

    // Ensure the SQLite database file exists before connecting.
    // On macOS (ARM), the bundled libsqlite3-sys can fail to CREATE a new
    // file in certain directories. Pre-creating an empty file sidesteps this.
    if let Some(sqlite_path) = db_url.strip_prefix("sqlite://") {
        let path = std::path::Path::new(sqlite_path);
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    PersistenceError::Database(format!("failed to create db directory: {e}"))
                })?;
            }
        }
        if !path.exists() {
            std::fs::write(path, []).map_err(|e| {
                PersistenceError::Database(format!("failed to create db file: {e}"))
            })?;
        }
    }

    let pool: Pool<Any> = AnyPoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await
        .map_err(|e| PersistenceError::Database(format!("failed to connect: {e}")))?;

    tracing::info!("persistence worker connected to database");

    // Process commands sequentially.
    while let Ok(cmd) = cmd_rx.recv() {
        let result = handle_command(&pool, cmd.kind).await;

        // Send the result back. If the receiver has dropped (caller gave up),
        // that's fine — we just move on.
        let _ = cmd.response.send(result);
    }

    tracing::info!("persistence worker shutting down");
    Ok(())
}

/// Dispatch a single command to the appropriate repository function.
async fn handle_command(
    pool: &Pool<Any>,
    kind: CommandKind,
) -> Result<crate::CommandResult, PersistenceError> {
    match kind {
        CommandKind::Migrate => repository::migrate(pool).await,
        CommandKind::ResolveIdentity { token_hash } => {
            repository::resolve_identity(pool, &token_hash).await
        }
        CommandKind::CreateIdentity {
            token_hash,
            player_id,
        } => repository::create_identity(pool, &token_hash, player_id).await,
        CommandKind::SaveInventory { player_id, items } => {
            repository::save_inventory(pool, player_id, &items).await
        }
        CommandKind::LoadInventory { player_id } => {
            repository::load_inventory(pool, player_id).await
        }
        CommandKind::SaveCreatureBond {
            player_id,
            creature_kind,
            bond_level,
        } => repository::save_creature_bond(pool, player_id, &creature_kind, bond_level).await,
        CommandKind::LoadCreatureBond {
            player_id,
            creature_kind,
        } => repository::load_creature_bond(pool, player_id, &creature_kind).await,
        CommandKind::SavePlotAssignment {
            slot_index,
            player_id,
        } => repository::save_plot_assignment(pool, slot_index, player_id).await,
        CommandKind::LoadPlotAssignment { player_id } => {
            repository::load_plot_assignment(pool, player_id).await
        }
        CommandKind::SavePlotDecoration {
            player_id,
            decoration,
        } => repository::save_plot_decoration(pool, player_id, &decoration).await,
        CommandKind::LoadPlotDecorations { player_id } => {
            repository::load_plot_decorations(pool, player_id).await
        }
        CommandKind::TestStall { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            Ok(crate::CommandResult::TestStalled)
        }
    }
}
