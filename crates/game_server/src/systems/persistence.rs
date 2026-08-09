use std::sync::Mutex;
use std::sync::mpsc;

use bevy::prelude::*;
use game_persistence::{CommandResult, PersistenceError, PersistenceHandle};
use game_protocol::channels::ReliableChannel;
use game_protocol::messages::{ActionRejected, InventorySnapshot, inventory_to_snapshot_items};
use lightyear::prelude::MessageSender;

use crate::systems::auth::PersistenceResource;
use crate::systems::connection::ClientPlayer;

// ---------------------------------------------------------------------------
// Pending operation types
// ---------------------------------------------------------------------------

/// Data needed after persistence confirms to apply a deferred ECS mutation.
pub(crate) enum PendingMutation {
    Collect {
        player_entity: Entity,
        node_entity: Entity,
        rows: Vec<game_persistence::InventoryRow>,
        sequence: u64,
    },
}

/// A single operation awaiting persistence acknowledgement.
struct PendingOp {
    rx: Mutex<mpsc::Receiver<Result<CommandResult, PersistenceError>>>,
    client_entity: Entity,
    correlation_id: u64,
    mutation: PendingMutation,
}

// ---------------------------------------------------------------------------
// Coordinator resource
// ---------------------------------------------------------------------------

/// Tracks operations sent to the persistence worker but not yet acknowledged.
#[derive(Resource, Default)]
pub struct PersistenceCoordinator {
    pending: Vec<PendingOp>,
}

// ---------------------------------------------------------------------------
// Startup wiring
// ---------------------------------------------------------------------------

/// Resolve the database URL to use: the config field takes precedence, then
/// the `YUME_DATABASE_URL` env var. `None` means persistence is disabled.
///
/// Split out as a pure function (mirroring `TlsConfig`'s config/env
/// precedence) so the precedence rule is testable without touching the real
/// process environment.
pub(crate) fn resolve_db_url(
    config_value: Option<&str>,
    env_value: Option<&str>,
) -> Option<String> {
    config_value.or(env_value).map(str::to_string)
}

/// Spawn a [`PersistenceWorker`](game_persistence::worker::PersistenceWorker),
/// run migrations, and return the cheap, cloneable [`PersistenceResource`]
/// handle gameplay systems pull via `Option<Res<PersistenceResource>>`.
///
/// The worker itself is intentionally leaked (`Box::leak`) rather than
/// stored in a Bevy resource. `PersistenceWorker::drop` blocks joining its
/// thread, and that thread only exits once *every* `PersistenceHandle`
/// clone — including the one inside the `PersistenceResource` this function
/// returns — has been dropped. Bevy does not guarantee resource drop order,
/// so storing the worker as a sibling resource risks it being dropped
/// (and blocking on join) while `PersistenceResource` is still alive
/// elsewhere in the world, deadlocking app/process shutdown. The worker is
/// meant to live for the entire process anyway ("spawn once at application
/// startup" — see `PersistenceWorker::spawn`'s docs), so leaking it is
/// correct here, not a workaround: the OS reclaims the thread when the
/// process exits.
pub(crate) fn spawn_persistence(db_url: &str) -> Result<PersistenceResource, PersistenceError> {
    let worker = game_persistence::worker::PersistenceWorker::spawn(
        db_url,
        game_persistence::worker::DEFAULT_CHANNEL_CAPACITY,
    )?;
    worker.handle().migrate()?;
    let resource = PersistenceResource(worker.handle().clone());
    Box::leak(Box::new(worker));
    Ok(resource)
}

impl PersistenceCoordinator {
    pub(crate) fn push(
        &mut self,
        rx: mpsc::Receiver<Result<CommandResult, PersistenceError>>,
        client_entity: Entity,
        correlation_id: u64,
        mutation: PendingMutation,
    ) {
        self.pending.push(PendingOp {
            rx: Mutex::new(rx),
            client_entity,
            correlation_id,
            mutation,
        });
    }
}

// ---------------------------------------------------------------------------
// Convert ECS inventory to persistence rows
// ---------------------------------------------------------------------------

pub(crate) fn inventory_to_rows(
    inventory: &resources::components::PlayerInventory,
) -> Vec<game_persistence::InventoryRow> {
    inventory
        .inventory
        .slots
        .iter()
        .filter_map(|slot| {
            slot.as_ref().map(|stack| {
                let kind_str = match &stack.kind {
                    game_core::inventory::ItemKind::Resource(r) => format!("{r:?}"),
                };
                game_persistence::InventoryRow {
                    resource_kind: kind_str,
                    quantity: stack.quantity as i32,
                }
            })
        })
        .collect()
}

/// Convert persistence rows back to an ECS Inventory.
pub(crate) fn rows_to_inventory(
    rows: &[game_persistence::InventoryRow],
) -> game_core::inventory::Inventory {
    let mut inv = game_core::inventory::Inventory::default();
    for row in rows {
        let kind = match row.resource_kind.as_str() {
            "Wood" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Wood)
            }
            "Stone" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Stone)
            }
            "Berry" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Berry)
            }
            "Crystal" => game_core::inventory::ItemKind::Resource(
                game_core::resources::ResourceKind::Crystal,
            ),
            "Flower" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Flower)
            }
            "Fiber" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Fiber)
            }
            "Mushroom" => game_core::inventory::ItemKind::Resource(
                game_core::resources::ResourceKind::Mushroom,
            ),
            "Sap" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Sap)
            }
            _ => continue,
        };
        let _ = inv.add(kind, row.quantity as u32);
    }
    inv
}

/// Send a SaveInventory command and register a pending collect mutation.
pub(crate) fn persist_collect(
    persistence: &PersistenceHandle,
    coordinator: &mut PersistenceCoordinator,
    client_entity: Entity,
    correlation_id: u64,
    player_entity: Entity,
    node_entity: Entity,
    player_id: i64,
    rows: Vec<game_persistence::InventoryRow>,
) -> Result<(), PersistenceError> {
    let pt = persistence.send_async(game_persistence::CommandKind::SaveInventory {
        player_id,
        items: rows.clone(),
    })?;
    coordinator.push(
        pt.into_rx(),
        client_entity,
        correlation_id,
        PendingMutation::Collect {
            player_entity,
            node_entity,
            rows,
            sequence: correlation_id,
        },
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Processing system
// ---------------------------------------------------------------------------

/// Poll pending transactions. On success applies the deferred ECS mutation.
#[allow(clippy::type_complexity)]
pub fn process_pending_transactions(
    coordinator: Option<ResMut<PersistenceCoordinator>>,
    mut node_status_query: Query<(
        &mut resources::components::ResourceNodeStatus,
        &mut game_protocol::ResourceNodeState,
    )>,
    mut player_state_query: Query<(
        Option<&mut resources::components::PlayerInventory>,
        Option<&mut resources::components::InteractionCooldown>,
        Option<&mut resources::components::ActionSequence>,
    )>,
    mut inventory_senders: Query<&mut MessageSender<InventorySnapshot>>,
    mut rejected_senders: Query<&mut MessageSender<ActionRejected>>,
    client_players: Query<&ClientPlayer>,
) {
    let Some(mut coordinator) = coordinator else {
        return;
    };
    let mut completed: Vec<usize> = Vec::new();

    for (i, op) in coordinator.pending.iter_mut().enumerate() {
        let result = {
            let rx = op.rx.lock().expect("persistence coordinator lock");
            match rx.try_recv() {
                Ok(Ok(r)) => Some(Ok(r)),
                Ok(Err(e)) => Some(Err(e)),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(PersistenceError::ChannelClosed)),
            }
        };

        let Some(result) = result else {
            continue;
        };

        match result {
            Ok(_command_result) => {
                let PendingMutation::Collect {
                    player_entity,
                    node_entity,
                    rows,
                    sequence,
                } = &op.mutation;
                {
                    // Deplete the resource node.
                    if let Ok((mut status, mut rep_state)) = node_status_query.get_mut(*node_entity)
                    {
                        status.depleted = true;
                        rep_state.depleted = true;
                        rep_state.respawn_progress = 0.0;
                    }

                    // Apply the persisted inventory state to ECS.
                    if let Ok((inv_opt, cooldown_opt, seq_opt)) =
                        player_state_query.get_mut(*player_entity)
                    {
                        if let Some(mut inv) = inv_opt {
                            inv.inventory = rows_to_inventory(rows);
                        }
                        if let Some(mut cd) = cooldown_opt {
                            cd.active = true;
                            cd.elapsed = 0.0;
                        }
                        if let Some(mut seq) = seq_opt {
                            seq.last_sequence = *sequence;
                        }
                    }

                    // Send inventory snapshot.
                    if let Ok(client_player) = client_players.get(op.client_entity) {
                        if let Ok(mut sender) = inventory_senders.get_mut(op.client_entity) {
                            if let Ok((Some(inv), _, _)) =
                                player_state_query.get(client_player.player_entity)
                            {
                                sender.send::<ReliableChannel>(InventorySnapshot {
                                    items: inventory_to_snapshot_items(&inv.inventory),
                                });
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if let Ok(mut sender) = rejected_senders.get_mut(op.client_entity) {
                    sender.send::<ReliableChannel>(ActionRejected {
                        sequence: op.correlation_id,
                        reason: format!("persistence error: {e}"),
                    });
                }
            }
        }

        completed.push(i);
    }

    for i in completed.iter().rev() {
        coordinator.pending.swap_remove(*i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_db_url_prefers_config_over_env() {
        assert_eq!(
            resolve_db_url(Some("postgres://config"), Some("postgres://env")),
            Some("postgres://config".to_string())
        );
    }

    #[test]
    fn resolve_db_url_falls_back_to_env() {
        assert_eq!(
            resolve_db_url(None, Some("postgres://env")),
            Some("postgres://env".to_string())
        );
    }

    #[test]
    fn resolve_db_url_none_when_neither_set() {
        assert_eq!(resolve_db_url(None, None), None);
    }

    /// End-to-end: spawning persistence against a real (temp) database
    /// migrates it and returns a working, cloneable handle — this is the
    /// path `ServerPlugin::build` exercises when `YUME_DATABASE_URL` (or
    /// `ServerConfig::db_url`) is set. Regression test for the bug where
    /// the server read the env var nowhere and every deploy ran fully
    /// ephemeral despite the configured Postgres/SealedSecret infra.
    #[test]
    fn spawn_persistence_activates_and_resolves_identities() {
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("spawn_persistence.db");
        let url = format!("sqlite://{}", db_path.display());

        let resource = spawn_persistence(&url).expect("spawn persistence");

        // The resource's handle is a live, cloneable connection to the
        // worker — exactly what gameplay systems pull via
        // `Option<Res<PersistenceResource>>`. Round-trip an identity to
        // confirm migrations actually ran and the worker is live, not a
        // stub.
        let handle = resource.handle().clone();
        let created = handle
            .resolve_identity("spawn_persistence_test_token_hash")
            .expect("resolve identity");
        let player_id = created.player_id;

        let resolved_again = handle
            .resolve_identity("spawn_persistence_test_token_hash")
            .expect("resolve identity again");
        assert_eq!(
            resolved_again.player_id, player_id,
            "same token hash should resolve to the same player id"
        );
    }

    #[test]
    fn spawn_persistence_errors_on_invalid_url() {
        let err = spawn_persistence("not-a-real-scheme://nope");
        assert!(err.is_err(), "invalid db url should fail to spawn");
    }
}
