//! Pure game-state types (no logic, no reducers).

use crate::id::*;
use crate::inventory::ItemKind;
use serde::{Deserialize, Serialize};

/// Globally unique plot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlotId(pub u64);

impl std::fmt::Display for PlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PlotId({})", self.0)
    }
}

/// State of a single resource node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceNodeState {
    pub available: bool,
    /// Tick at which this node becomes available again (if not available).
    pub respawn_at_tick: u64,
}

/// Bond level with a creature.
pub type BondLevel = u32;

/// What sits on a housing plot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlotContent {
    Empty,
    Item(ItemKind),
}

/// Ownership and content of a single plot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotState {
    pub owner: PlayerId,
    pub content: PlotContent,
}
