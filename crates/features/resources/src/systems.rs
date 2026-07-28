use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::constants::INTERACT_RADIUS;
use game_core::world_config::WorldConfig;
use game_protocol::ResourceNodeState;

use crate::components::*;

// ---------------------------------------------------------------------------
// Spawn (called from game_server adapter system)
// ---------------------------------------------------------------------------

/// Spawns one entity per resource node defined in the world config.
///
/// Each entity carries:
/// - `ResourceNode` (static config)
/// - `ResourceNodeState` (replicated component)
/// - `ResourceNodeStatus` (mutable server state)
/// - An avian collision cylinder for proximity queries
/// - `Replicate` marker for lightyear replication
pub fn spawn_resource_nodes(mut commands: Commands, world_config: &WorldConfig) {
    let mut flat_index = 0usize;

    for res_cfg in &world_config.resources {
        for pos in &res_cfg.positions {
            let position = *pos;

            let replicated = ResourceNodeState {
                resource_id: flat_index as u64,
                kind: res_cfg.kind,
                position_x: position.x,
                position_y: position.y,
                position_z: position.z,
                depleted: false,
                respawn_progress: 1.0,
            };

            commands.spawn((
                ResourceNode {
                    node_index: flat_index,
                    kind: res_cfg.kind,
                    yield_amount: res_cfg.yield_amount,
                    respawn_seconds: res_cfg.respawn_seconds,
                    position,
                },
                ResourceNodeStatus {
                    depleted: false,
                    respawn_timer: 0.0,
                },
                replicated,
                RigidBody::Static,
                Collider::cylinder(0.5, 0.5),
                Transform::from_translation(position),
                lightyear::prelude::Replicate::default(),
            ));

            flat_index += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Collection validation (pure function)
// ---------------------------------------------------------------------------

/// Outcome of validating a collect action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectValidation {
    Success,
    OutOfRange,
    NodeNotAvailable,
    InventoryFull,
    CooldownActive,
    StaleSequence,
    UnknownNode,
}

/// Validate a collect action. Pure function — no ECS access needed.
#[allow(clippy::too_many_arguments)]
pub fn validate_collect(
    player_pos: Vec3,
    node: Option<&ResourceNode>,
    node_status: Option<&ResourceNodeStatus>,
    inventory: Option<&PlayerInventory>,
    cooldown: Option<&InteractionCooldown>,
    sequence: Option<&ActionSequence>,
    intent_sequence: u64,
) -> CollectValidation {
    // Must have a target node
    let node = match node {
        Some(n) => n,
        None => return CollectValidation::UnknownNode,
    };

    // Node must be available
    if let Some(status) = node_status {
        if status.depleted {
            return CollectValidation::NodeNotAvailable;
        }
    }

    // Range check
    let dist = player_pos.distance(node.position);
    if dist > INTERACT_RADIUS {
        return CollectValidation::OutOfRange;
    }

    // Cooldown check
    if let Some(cd) = cooldown {
        if cd.active && cd.elapsed < game_core::constants::INTERACT_COOLDOWN_S {
            return CollectValidation::CooldownActive;
        }
    }

    // Monotonic sequence check
    if let Some(seq) = sequence {
        if intent_sequence <= seq.last_sequence {
            return CollectValidation::StaleSequence;
        }
    }

    // Inventory capacity check
    if let Some(inv) = inventory {
        if inv.inventory.is_full() {
            return CollectValidation::InventoryFull;
        }
    }

    CollectValidation::Success
}

// ---------------------------------------------------------------------------
// Respawn
// ---------------------------------------------------------------------------

/// Advances resource node respawn timers.
/// Runs every fixed update tick.
pub fn tick_resource_respawn(
    time: Res<Time>,
    mut nodes: Query<(
        &ResourceNode,
        &mut ResourceNodeStatus,
        &mut ResourceNodeState,
    )>,
) {
    let dt = time.delta_secs();
    for (node, mut status, mut rep_state) in nodes.iter_mut() {
        if status.depleted {
            status.respawn_timer -= dt;
            if status.respawn_timer <= 0.0 {
                status.depleted = false;
                status.respawn_timer = 0.0;
                rep_state.depleted = false;
                rep_state.respawn_progress = 1.0;
            } else {
                let total = node.respawn_seconds;
                if total > 0.0 {
                    rep_state.respawn_progress = 1.0 - (status.respawn_timer / total);
                }
            }
        }
    }
}
