use std::sync::Mutex;
use std::sync::mpsc;

use bevy::prelude::*;
use game_persistence::{CommandResult, PersistenceError, PersistenceHandle};
use game_protocol::channels::ReliableChannel;
use game_protocol::messages::{ActionRejected, InventorySnapshot, inventory_to_snapshot_items};
use lightyear::prelude::MessageSender;

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
