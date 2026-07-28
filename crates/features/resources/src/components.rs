use bevy::prelude::*;
use game_core::constants::INTERACT_COOLDOWN_S;
use game_core::inventory::Inventory;
use game_core::resources::ResourceKind;

// ---------------------------------------------------------------------------
// Resource node components (server-side)
// ---------------------------------------------------------------------------

/// Static configuration of a resource node (never changes after spawn).
#[derive(Component, Debug, Clone)]
pub struct ResourceNode {
    /// Flat index into the deterministic GameState resource_nodes vec.
    pub node_index: usize,
    /// Which kind of resource this node yields.
    pub kind: ResourceKind,
    /// Amount given per successful collect.
    pub yield_amount: u32,
    /// Seconds before the node respawns after depletion.
    pub respawn_seconds: f32,
    /// Position of this node in world space.
    pub position: Vec3,
}

/// Mutable status of a resource node.
/// Only exists on the server; clients see the replicated `ResourceNodeState`.
#[derive(Component, Debug, Clone)]
pub struct ResourceNodeStatus {
    pub depleted: bool,
    /// Remaining seconds until respawn (only meaningful when depleted).
    pub respawn_timer: f32,
}

// ---------------------------------------------------------------------------
// Player inventory component (server-side)
// ---------------------------------------------------------------------------

/// Server-side player inventory, wrapping `game_core::inventory::Inventory`.
/// Added to the player entity by `ResourcesPlugin`.
#[derive(Component, Debug, Clone, Default)]
pub struct PlayerInventory {
    pub inventory: Inventory,
}

// ---------------------------------------------------------------------------
// Cooldown & sequence tracking (server-side)
// ---------------------------------------------------------------------------

/// Tracks the last interaction time per player for cooldown enforcement.
#[derive(Component, Debug, Clone)]
pub struct InteractionCooldown {
    /// Real (wall-clock) seconds elapsed since the last interaction.
    pub elapsed: f32,
    /// Whether the cooldown is active.
    pub active: bool,
}

impl Default for InteractionCooldown {
    fn default() -> Self {
        Self {
            elapsed: INTERACT_COOLDOWN_S + 1.0, // Start ready
            active: false,
        }
    }
}

/// Monotonically-increasing sequence counter for action deduplication.
#[derive(Component, Debug, Clone, Default)]
pub struct ActionSequence {
    pub last_sequence: u64,
}
