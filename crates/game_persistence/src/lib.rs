//! Bevy-free portable SQLx persistence repository.
//!
//! Architecture
//! ------------
//! A dedicated `std::thread` hosts a Tokio runtime. Communication from the
//! (non-async) caller to the worker happens over a bounded synchronous channel
//! (`std::sync::mpsc::sync_channel`). Each command carries a one-shot response
//! sender; the caller blocks on `recv()` until the worker replies.
//!
//! The repository uses **only** portable SQL types (INTEGER, TEXT, REAL) and
//! `$N` bind parameters so it runs identically on SQLite (local/test) and
//! PostgreSQL (production).

pub mod repository;
pub mod worker;

use std::sync::mpsc;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors returned by the persistence layer.
#[derive(Error, Debug)]
pub enum PersistenceError {
    /// An SQL/database error (connection failure, syntax, etc.).
    #[error("database error: {0}")]
    Database(String),

    /// A constraint violation (UNIQUE, FOREIGN KEY, NOT NULL, etc.).
    #[error("constraint violation: {0}")]
    Constraint(String),

    /// The worker channel has closed (worker crashed / dropped).
    #[error("persistence worker channel closed")]
    ChannelClosed,

    /// The command queue has reached its capacity.
    #[error("command queue full (capacity: {capacity})")]
    QueueFull {
        /// Maximum number of pending commands.
        capacity: usize,
    },
}

// ---------------------------------------------------------------------------
// Row types (public, returned from load operations)
// ---------------------------------------------------------------------------

/// A row from the `identities` table.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentityRow {
    /// The player identifier assigned to this token hash.
    pub player_id: i64,
    /// Unix-epoch seconds when the identity was first created.
    pub created_at: i64,
}

/// A single inventory slot row.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryRow {
    /// The resource kind as a portable string (e.g. "Wood", "Berry").
    pub resource_kind: String,
    /// Number of items of this kind.
    pub quantity: i32,
}

/// A quest-progress row.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestProgressRow {
    /// The quest identifier.
    pub quest_id: i64,
    /// Normalized completion progress (0.0 … 1.0).
    pub progress: f64,
    /// Whether the quest has been completed.
    pub completed: bool,
}

/// A creature-bond row.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureBondRow {
    /// The creature kind as a portable string (e.g. "Fluffball").
    pub creature_kind: String,
    /// The bond level (0 = no bond, increasing).
    pub bond_level: i32,
}

/// A plot-assignment row.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotAssignmentRow {
    /// The slot index (0 … 15) assigned to this player.
    pub slot_index: i64,
}

/// A plot-decoration row.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotDecorationRow {
    /// Decoration instance identifier.
    pub decoration_id: i64,
    /// World-space X coordinate.
    pub position_x: f64,
    /// World-space Z coordinate.
    pub position_z: f64,
    /// Y-axis rotation in radians.
    pub rotation_y: f64,
}

// ---------------------------------------------------------------------------
// Internal command protocol
// ---------------------------------------------------------------------------

/// Variants of work the worker thread can execute.
#[derive(Debug)]
pub enum CommandKind {
    /// Run all pending migrations.
    Migrate,
    /// Find a player identity by token hash, or create one if missing.
    ResolveIdentity { token_hash: String },
    /// Explicitly create an identity (fails on duplicate).
    CreateIdentity { token_hash: String, player_id: i64 },
    /// Replaces the full inventory for a player.
    SaveInventory {
        player_id: i64,
        items: Vec<InventoryRow>,
    },
    /// Loads the full inventory for a player.
    LoadInventory { player_id: i64 },
    /// Upserts a single quest-progress row.
    SaveQuestProgress {
        player_id: i64,
        quest_id: i64,
        progress: f64,
        completed: bool,
    },
    /// Loads a single quest-progress row.
    LoadQuestProgress { player_id: i64, quest_id: i64 },
    /// Upserts a creature-bond row.
    SaveCreatureBond {
        player_id: i64,
        creature_kind: String,
        bond_level: i32,
    },
    /// Loads a creature-bond row.
    LoadCreatureBond {
        player_id: i64,
        creature_kind: String,
    },
    /// Upserts a plot-assignment row.
    SavePlotAssignment { slot_index: i64, player_id: i64 },
    /// Loads a plot assignment by player_id.
    LoadPlotAssignment { player_id: i64 },
    /// Upserts a plot-decoration row.
    SavePlotDecoration {
        player_id: i64,
        decoration: PlotDecorationRow,
    },
    /// Loads all plot decorations for a player.
    LoadPlotDecorations { player_id: i64 },
    /// (testing) Block the worker for `ms` milliseconds.
    #[doc(hidden)]
    TestStall { ms: u64 },
}

/// A command sent from `PersistenceHandle` to the worker thread.
pub struct Command {
    pub kind: CommandKind,
    pub response: mpsc::Sender<Result<CommandResult, PersistenceError>>,
}

/// Opaque result variant returned from the worker.
#[derive(Debug)]
pub enum CommandResult {
    Migrated,
    Identity(IdentityRow),
    IdentityCreated,
    InventorySaved,
    Inventory(Vec<InventoryRow>),
    QuestProgressSaved,
    QuestProgress(Option<QuestProgressRow>),
    CreatureBondSaved,
    CreatureBond(Option<CreatureBondRow>),
    PlotAssignmentSaved,
    PlotAssignment(Option<PlotAssignmentRow>),
    PlotDecorationSaved,
    PlotDecorations(Vec<PlotDecorationRow>),
    TestStalled,
}

// ---------------------------------------------------------------------------
// PersistenceHandle
// ---------------------------------------------------------------------------

/// A handle for sending persistence commands to the dedicated worker thread.
///
/// All methods block the calling thread until the worker completes the
/// command. The worker runs its own Tokio runtime on a separate thread so
/// the Bevy schedule is never blocked.
///
/// Cloning is cheap — all clones share the same underlying channel.
#[derive(Clone)]
pub struct PersistenceHandle {
    cmd_tx: mpsc::SyncSender<Command>,
    capacity: usize,
}

// ---------------------------------------------------------------------------
// PendingTransaction (async / non-blocking)
// ---------------------------------------------------------------------------

/// A pending persistence transaction that can be polled for completion.
///
/// Returned by [`PersistenceHandle::send_async`]. Call [`try_recv`](Self::try_recv)
/// to check whether the worker has finished processing this command, or
/// [`recv`](Self::recv) to block until it completes.
#[derive(Debug)]
pub struct PendingTransaction {
    rx: mpsc::Receiver<Result<CommandResult, PersistenceError>>,
}

impl PendingTransaction {
    /// Non-blocking poll. Returns `None` if the worker is still processing.
    ///
    /// Use this in game loops where you cannot block the schedule.
    pub fn try_recv(&mut self) -> Option<Result<CommandResult, PersistenceError>> {
        self.rx.try_recv().ok()
    }

    /// Block the calling thread until the worker completes.
    pub fn recv(self) -> Result<CommandResult, PersistenceError> {
        self.rx
            .recv()
            .map_err(|_| PersistenceError::ChannelClosed)?
    }

    /// Consume the transaction and return the inner receiver.
    ///
    /// Used by the server coordinator to store receivers for polling.
    #[doc(hidden)]
    pub fn into_rx(self) -> mpsc::Receiver<Result<CommandResult, PersistenceError>> {
        self.rx
    }
}

impl PersistenceHandle {
    /// Create a new handle from an existing sender with known capacity.
    ///
    /// Normal users should use [`PersistenceWorker::spawn`](crate::worker::PersistenceWorker::spawn)
    /// instead.
    pub(crate) fn new(cmd_tx: mpsc::SyncSender<Command>, capacity: usize) -> Self {
        Self { cmd_tx, capacity }
    }

    // ------------------------------------------------------------------
    // Internal dispatch helper
    // ------------------------------------------------------------------

    /// Dispatch a `CommandKind` and wait for the result.
    fn dispatch(&self, kind: CommandKind) -> Result<CommandResult, PersistenceError> {
        let (tx, rx) = mpsc::channel();
        let cmd = Command { kind, response: tx };
        self.cmd_tx.try_send(cmd).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => PersistenceError::QueueFull {
                capacity: self.capacity,
            },
            mpsc::TrySendError::Disconnected(_) => PersistenceError::ChannelClosed,
        })?;
        rx.recv().map_err(|_| PersistenceError::ChannelClosed)?
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Run all pending database migrations.
    ///
    /// Safe to call multiple times — migrations are idempotent.
    pub fn migrate(&self) -> Result<(), PersistenceError> {
        self.dispatch(CommandKind::Migrate)?;
        Ok(())
    }

    /// Find a player identity by `token_hash`, or create one atomically.
    ///
    /// Returns the identity row (including the player_id, which is auto-assigned
    /// on creation).
    pub fn resolve_identity(&self, token_hash: &str) -> Result<IdentityRow, PersistenceError> {
        match self.dispatch(CommandKind::ResolveIdentity {
            token_hash: token_hash.to_owned(),
        })? {
            CommandResult::Identity(row) => Ok(row),
            _ => Err(PersistenceError::Database(
                "unexpected result from resolve_identity".into(),
            )),
        }
    }

    /// Explicitly create an identity with a specific `player_id`.
    ///
    /// Returns an error if the token_hash already exists.
    pub fn create_identity(
        &self,
        token_hash: &str,
        player_id: i64,
    ) -> Result<(), PersistenceError> {
        self.dispatch(CommandKind::CreateIdentity {
            token_hash: token_hash.to_owned(),
            player_id,
        })?;
        Ok(())
    }

    /// Replace the full inventory for a player.
    ///
    /// This is a full-replace operation: existing rows for this player are
    /// deleted and the provided rows are inserted.
    pub fn save_inventory(
        &self,
        player_id: i64,
        items: &[InventoryRow],
    ) -> Result<(), PersistenceError> {
        self.dispatch(CommandKind::SaveInventory {
            player_id,
            items: items.to_vec(),
        })?;
        Ok(())
    }

    /// Load the full inventory for a player.
    pub fn load_inventory(&self, player_id: i64) -> Result<Vec<InventoryRow>, PersistenceError> {
        match self.dispatch(CommandKind::LoadInventory { player_id })? {
            CommandResult::Inventory(rows) => Ok(rows),
            _ => Err(PersistenceError::Database(
                "unexpected result from load_inventory".into(),
            )),
        }
    }

    /// Upsert a single quest-progress row.
    pub fn save_quest_progress(
        &self,
        player_id: i64,
        quest_id: i64,
        progress: f64,
        completed: bool,
    ) -> Result<(), PersistenceError> {
        self.dispatch(CommandKind::SaveQuestProgress {
            player_id,
            quest_id,
            progress,
            completed,
        })?;
        Ok(())
    }

    /// Load a single quest-progress row.
    pub fn load_quest_progress(
        &self,
        player_id: i64,
        quest_id: i64,
    ) -> Result<Option<QuestProgressRow>, PersistenceError> {
        match self.dispatch(CommandKind::LoadQuestProgress {
            player_id,
            quest_id,
        })? {
            CommandResult::QuestProgress(row) => Ok(row),
            _ => Err(PersistenceError::Database(
                "unexpected result from load_quest_progress".into(),
            )),
        }
    }

    /// Upsert a creature-bond row.
    pub fn save_creature_bond(
        &self,
        player_id: i64,
        creature_kind: &str,
        bond_level: i32,
    ) -> Result<(), PersistenceError> {
        self.dispatch(CommandKind::SaveCreatureBond {
            player_id,
            creature_kind: creature_kind.to_owned(),
            bond_level,
        })?;
        Ok(())
    }

    /// Load a creature-bond row.
    pub fn load_creature_bond(
        &self,
        player_id: i64,
        creature_kind: &str,
    ) -> Result<Option<CreatureBondRow>, PersistenceError> {
        match self.dispatch(CommandKind::LoadCreatureBond {
            player_id,
            creature_kind: creature_kind.to_owned(),
        })? {
            CommandResult::CreatureBond(row) => Ok(row),
            _ => Err(PersistenceError::Database(
                "unexpected result from load_creature_bond".into(),
            )),
        }
    }

    /// Upsert a plot-assignment row.
    pub fn save_plot_assignment(
        &self,
        slot_index: i64,
        player_id: i64,
    ) -> Result<(), PersistenceError> {
        self.dispatch(CommandKind::SavePlotAssignment {
            slot_index,
            player_id,
        })?;
        Ok(())
    }

    /// Load a plot assignment by player_id.
    pub fn load_plot_assignment(
        &self,
        player_id: i64,
    ) -> Result<Option<PlotAssignmentRow>, PersistenceError> {
        match self.dispatch(CommandKind::LoadPlotAssignment { player_id })? {
            CommandResult::PlotAssignment(row) => Ok(row),
            _ => Err(PersistenceError::Database(
                "unexpected result from load_plot_assignment".into(),
            )),
        }
    }

    /// Upsert a single plot-decoration row.
    pub fn save_plot_decoration(
        &self,
        player_id: i64,
        decoration: &PlotDecorationRow,
    ) -> Result<(), PersistenceError> {
        self.dispatch(CommandKind::SavePlotDecoration {
            player_id,
            decoration: decoration.clone(),
        })?;
        Ok(())
    }

    /// Load all plot decorations for a player.
    pub fn load_plot_decorations(
        &self,
        player_id: i64,
    ) -> Result<Vec<PlotDecorationRow>, PersistenceError> {
        match self.dispatch(CommandKind::LoadPlotDecorations { player_id })? {
            CommandResult::PlotDecorations(rows) => Ok(rows),
            _ => Err(PersistenceError::Database(
                "unexpected result from load_plot_decorations".into(),
            )),
        }
    }

    // ------------------------------------------------------------------
    // Non-blocking / async send
    // ------------------------------------------------------------------

    /// Send a command and return immediately with a [`PendingTransaction`].
    ///
    /// Unlike the blocking `dispatch` method, this does not wait for the
    /// worker to finish processing. The caller should poll
    /// [`PendingTransaction::try_recv`] in a later frame to get the result.
    ///
    /// The bounded queue still applies — if the queue is full this returns
    /// [`PersistenceError::QueueFull`] without sending.
    pub fn send_async(&self, kind: CommandKind) -> Result<PendingTransaction, PersistenceError> {
        let (tx, rx) = mpsc::channel();
        let cmd = Command { kind, response: tx };
        self.cmd_tx.try_send(cmd).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => PersistenceError::QueueFull {
                capacity: self.capacity,
            },
            mpsc::TrySendError::Disconnected(_) => PersistenceError::ChannelClosed,
        })?;
        Ok(PendingTransaction { rx })
    }

    // --- test helpers ----------------------------------------------------

    /// (testing) Block the worker for `ms` milliseconds.
    #[doc(hidden)]
    pub fn _test_stall(&self, ms: u64) -> Result<(), PersistenceError> {
        self.dispatch(CommandKind::TestStall { ms })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------

/// SHA-256 hash of an identity token.
///
/// The raw token is **never** stored in the database, logs, or any persistent
/// store — only the hex-encoded SHA-256 digest is persisted.
pub fn hash_token(token: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    let hash = hasher.finalize();
    hex_encode(&hash)
}

/// Minimal hex encoder for a byte slice (avoids pulling in `hex` crate).
fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// Utility impls
// ---------------------------------------------------------------------------

impl std::fmt::Debug for PersistenceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistenceHandle").finish_non_exhaustive()
    }
}
