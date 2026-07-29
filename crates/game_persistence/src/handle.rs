//! [`PersistenceHandle`] and [`PendingTransaction`] — the public API for
//! sending commands to the persistence worker.

use std::sync::mpsc;

use super::*;

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

impl std::fmt::Debug for PersistenceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistenceHandle").finish_non_exhaustive()
    }
}
