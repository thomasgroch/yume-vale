use bevy::prelude::*;
use game_core::actions::ActionKind;
use game_core::constants::INTERACT_RADIUS;
use game_core::id::CreatureId;
use game_core::inventory::ItemKind;
use game_protocol::channels::ReliableChannel;
use game_protocol::messages::{
    ActionIntent, ActionRejected, BondEntry, BondSnapshot, InventorySnapshot,
    inventory_to_snapshot_items,
};
use lightyear::prelude::{MessageReceiver, MessageSender};
use player::Player;
use quests::components::ResourceCollectedEvent;
use resources::components::{
    ActionSequence, InteractionCooldown, PlayerInventory, ResourceNode, ResourceNodeStatus,
};
use resources::systems::{CollectValidation, validate_collect};
use tracing::warn;

use super::auth::PersistenceResource;
use super::connection::ClientPlayer;
use super::persistence::{PersistenceCoordinator, inventory_to_rows, persist_collect};

/// Processes `ActionIntent` messages with transactional persistence.
#[allow(clippy::type_complexity)]
pub fn handle_action_intent(
    mut commands: Commands,
    mut receivers: Query<(Entity, &mut MessageReceiver<ActionIntent>, &ClientPlayer)>,
    mut players: Query<(
        &Player,
        &Transform,
        Option<&mut PlayerInventory>,
        Option<&mut InteractionCooldown>,
        Option<&mut ActionSequence>,
    )>,
    mut nodes: Query<(
        Entity,
        &ResourceNode,
        &mut ResourceNodeStatus,
        &mut game_protocol::ResourceNodeState,
    )>,
    mut creatures: Query<(
        Entity,
        &creatures::Creature,
        &Transform,
        &mut creatures::FeedCooldown,
    )>,
    mut inventory_senders: Query<&mut MessageSender<InventorySnapshot>>,
    mut bond_senders: Query<&mut MessageSender<BondSnapshot>>,
    mut coordinator: Option<ResMut<PersistenceCoordinator>>,
    persistence: Option<Res<PersistenceResource>>,
    mut rejected_senders: Query<&mut MessageSender<ActionRejected>>,
) {
    let persistence_handle = persistence.as_ref().map(|p| p.handle().clone());

    for (link_entity, mut receiver, client_player) in receivers.iter_mut() {
        for intent in receiver.receive() {
            match intent.kind {
                ActionKind::Collect => {
                    let evt = if let (Some(handle), Some(coord)) =
                        (&persistence_handle, &mut coordinator)
                    {
                        handle_action_collect(
                            &intent,
                            link_entity,
                            client_player,
                            &mut players,
                            &mut nodes,
                            &mut inventory_senders,
                            handle,
                            coord,
                            &mut rejected_senders,
                        )
                    } else {
                        handle_action_collect_immediate(
                            &intent,
                            link_entity,
                            client_player,
                            &mut players,
                            &mut nodes,
                            &mut inventory_senders,
                        )
                    };
                    if let Some(evt) = evt {
                        commands.trigger(evt);
                    }
                }
                ActionKind::Feed => handle_feed(
                    &intent,
                    link_entity,
                    client_player,
                    &mut players,
                    &mut creatures,
                    &mut bond_senders,
                    &mut inventory_senders,
                ),
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Non-persistence path (used in tests without persistence worker)
// ---------------------------------------------------------------------------

/// Returns `Some(ResourceCollectedEvent)` on success, `None` on failure.
fn handle_action_collect_immediate(
    intent: &ActionIntent,
    link_entity: Entity,
    client_player: &ClientPlayer,
    players: &mut Query<(
        &Player,
        &Transform,
        Option<&mut PlayerInventory>,
        Option<&mut InteractionCooldown>,
        Option<&mut ActionSequence>,
    )>,
    nodes: &mut Query<(
        Entity,
        &ResourceNode,
        &mut ResourceNodeStatus,
        &mut game_protocol::ResourceNodeState,
    )>,
    inventory_senders: &mut Query<&mut MessageSender<InventorySnapshot>>,
) -> Option<ResourceCollectedEvent> {
    let player_entity = client_player.player_entity;
    let Ok((_player, player_transform, mut inventory, mut cooldown, mut sequence)) =
        players.get_mut(player_entity)
    else {
        warn!("collect_immediate: player missing components");
        return None;
    };

    let player_pos = player_transform.translation;
    let target_idx = intent.target_id.map(|id| id as usize);

    let Some((_e, node, mut node_status, mut rep_state)) =
        target_idx.and_then(|idx| nodes.iter_mut().find(|(_, n, _, _)| n.node_index == idx))
    else {
        warn!("collect_immediate: node not found");
        return None;
    };

    if validate_collect(
        player_pos,
        Some(node),
        Some(&*node_status),
        inventory.as_deref(),
        cooldown.as_deref(),
        sequence.as_deref(),
        intent.sequence,
    ) != CollectValidation::Success
    {
        return None;
    }

    let resource_kind = node.kind;
    let yield_amount = node.yield_amount;

    node_status.depleted = true;
    node_status.respawn_timer = node.respawn_seconds;
    rep_state.depleted = true;
    rep_state.respawn_progress = 0.0;

    let ik = ItemKind::Resource(resource_kind);
    if let Some(ref mut inv) = inventory {
        let _ = inv.inventory.add(ik, yield_amount);
    }
    if let Some(ref mut cd) = cooldown {
        cd.active = true;
        cd.elapsed = 0.0;
    }
    if let Some(ref mut seq) = sequence {
        seq.last_sequence = intent.sequence;
    }

    send_inventory_snapshot(link_entity, inventory.as_deref(), inventory_senders);

    Some(ResourceCollectedEvent {
        player_id: client_player.player_id,
        resource_kind,
        amount: yield_amount,
    })
}

// ---------------------------------------------------------------------------
// Transactional persistence path
// ---------------------------------------------------------------------------

/// Returns `Some(ResourceCollectedEvent)` on success, `None` on failure.
#[allow(clippy::too_many_arguments)]
fn handle_action_collect(
    intent: &ActionIntent,
    link_entity: Entity,
    client_player: &ClientPlayer,
    players: &mut Query<(
        &Player,
        &Transform,
        Option<&mut PlayerInventory>,
        Option<&mut InteractionCooldown>,
        Option<&mut ActionSequence>,
    )>,
    nodes: &mut Query<(
        Entity,
        &ResourceNode,
        &mut ResourceNodeStatus,
        &mut game_protocol::ResourceNodeState,
    )>,
    _inventory_senders: &mut Query<&mut MessageSender<InventorySnapshot>>,
    persistence: &game_persistence::PersistenceHandle,
    coordinator: &mut PersistenceCoordinator,
    rejected_senders: &mut Query<&mut MessageSender<ActionRejected>>,
) -> Option<ResourceCollectedEvent> {
    let player_entity = client_player.player_entity;

    let Ok((_player, player_transform, inventory, cooldown, sequence)) =
        players.get_mut(player_entity)
    else {
        warn!("collect: player missing components");
        return None;
    };

    let player_pos = player_transform.translation;
    let target_idx = intent.target_id.map(|id| id as usize);

    // Find the node — includes Entity for deferred mutation.
    let Some((node_entity, node, _node_status, _rep_state)) = target_idx.and_then(|idx| {
        nodes
            .iter()
            .find(|(_, n, _, _)| n.node_index == idx)
            .map(|(e, n, ns, r)| (e, n, ns, r))
    }) else {
        warn!("collect: node not found");
        return None;
    };

    // Read-only validation using immutable refs.
    if validate_collect(
        player_pos,
        Some(node),
        Some(_node_status),
        inventory.as_deref(),
        cooldown.as_deref(),
        sequence.as_deref(),
        intent.sequence,
    ) != CollectValidation::Success
    {
        return None;
    }

    let resource_kind = node.kind;
    let yield_amount = node.yield_amount;

    // Compute new inventory state (without mutating ECS yet).
    let mut new_inv = inventory
        .as_deref()
        .map(|inv| inv.inventory.clone())
        .unwrap_or_default();
    let ik = ItemKind::Resource(resource_kind);
    let _ = new_inv.add(ik, yield_amount);

    // Convert to persistence rows.
    let rows = {
        let pp = PlayerInventory { inventory: new_inv };
        inventory_to_rows(&pp)
    };

    // Send async persistence command. ECS is NOT mutated here.
    if let Err(e) = persist_collect(
        persistence,
        coordinator,
        link_entity,
        intent.sequence,
        player_entity,
        node_entity,
        client_player.player_id.get() as i64,
        rows,
    ) {
        warn!("collect persist_collect failed: {e}");
        if let Ok(mut sender) = rejected_senders.get_mut(link_entity) {
            sender.send::<ReliableChannel>(ActionRejected {
                sequence: intent.sequence,
                reason: format!("persistence error: {e}"),
            });
        }
        return None;
    }

    Some(ResourceCollectedEvent {
        player_id: client_player.player_id,
        resource_kind,
        amount: yield_amount,
    })
}

// ---------------------------------------------------------------------------
// Feed (unchanged from original)
// ---------------------------------------------------------------------------

fn handle_feed(
    intent: &ActionIntent,
    link_entity: Entity,
    client_player: &ClientPlayer,
    players: &mut Query<(
        &Player,
        &Transform,
        Option<&mut PlayerInventory>,
        Option<&mut InteractionCooldown>,
        Option<&mut ActionSequence>,
    )>,
    creatures: &mut Query<(
        Entity,
        &creatures::Creature,
        &Transform,
        &mut creatures::FeedCooldown,
    )>,
    bond_senders: &mut Query<&mut MessageSender<BondSnapshot>>,
    inventory_senders: &mut Query<&mut MessageSender<InventorySnapshot>>,
) {
    let player_entity = client_player.player_entity;
    let Ok((_player, player_transform, mut inventory, _cooldown, mut _sequence)) =
        players.get_mut(player_entity)
    else {
        warn!("Feed from player without player components");
        return;
    };
    let player_pos = player_transform.translation;

    let target_cid = match intent.target_id.map(CreatureId::new) {
        Some(cid) => cid,
        None => {
            warn!("Feed without target creature ID");
            return;
        }
    };

    let mut found_creature = None;
    for item in creatures.iter_mut() {
        let (_, c, _, _) = &item;
        if c.id == target_cid {
            found_creature = Some(item);
            break;
        }
    }
    let (_creature_entity, creature, creature_transform, mut feed_cooldown) = match found_creature {
        Some(d) => d,
        None => {
            warn!("Feed target not found");
            return;
        }
    };

    let dist = player_pos.distance(creature_transform.translation);
    if dist > INTERACT_RADIUS {
        warn!("Feed out of range");
        return;
    }
    if feed_cooldown.remaining_ticks > 0 {
        warn!("Feed cooldown active");
        return;
    }

    let food_kind = creature.food_kind;
    let expected_item = ItemKind::Resource(food_kind);
    let food_slot = inventory.as_ref().and_then(|inv| {
        inv.inventory.slots.iter().position(|slot| {
            slot.as_ref()
                .is_some_and(|stack| stack.kind == expected_item)
        })
    });
    let slot_idx = match food_slot {
        Some(idx) => idx,
        None => {
            warn!("No {:?} in inventory", food_kind);
            return;
        }
    };
    if let Some(ref mut inv) = inventory {
        let _ = inv.inventory.remove(slot_idx, 1);
    }
    feed_cooldown.remaining_ticks = creatures::FEED_COOLDOWN_TICKS;

    let bond_level: u32 = 1;
    if let Ok(mut sender) = bond_senders.get_mut(link_entity) {
        sender.send::<ReliableChannel>(BondSnapshot {
            bonds: vec![BondEntry {
                target_player: client_player.player_id.get(),
                bond_level,
            }],
        });
    }
    send_inventory_snapshot(link_entity, inventory.as_deref(), inventory_senders);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn send_inventory_snapshot(
    link_entity: Entity,
    inventory: Option<&PlayerInventory>,
    senders: &mut Query<&mut MessageSender<InventorySnapshot>>,
) {
    if let Ok(mut sender) = senders.get_mut(link_entity) {
        if let Some(inv) = inventory {
            sender.send::<ReliableChannel>(InventorySnapshot {
                items: inventory_to_snapshot_items(&inv.inventory),
            });
        }
    }
}

/// Adds inventory and cooldown components to any player entity that lacks them.
pub fn initialize_player_components(
    mut commands: Commands,
    query: Query<Entity, (With<Player>, Without<PlayerInventory>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).insert((
            PlayerInventory::default(),
            InteractionCooldown::default(),
            ActionSequence::default(),
        ));
    }
}

pub fn tick_player_cooldowns(time: Res<Time>, mut cooldowns: Query<&mut InteractionCooldown>) {
    let dt = time.delta_secs();
    for mut cd in cooldowns.iter_mut() {
        if cd.active {
            cd.elapsed += dt;
        }
    }
}
