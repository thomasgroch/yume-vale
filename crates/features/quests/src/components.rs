//! Components and events for the quest system.

use bevy::prelude::*;
use game_core::id::{PlayerId, QuestId};
use game_core::resources::ResourceKind;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Bevy events
// ---------------------------------------------------------------------------

/// Emitted after a successful resource collection.
///
/// Consumed by the quest system to increment matching quest progress.
#[derive(Event, Debug, Clone)]
pub struct ResourceCollectedEvent {
    pub player_id: PlayerId,
    pub resource_kind: ResourceKind,
    pub amount: u32,
}

// ---------------------------------------------------------------------------
// Component: quest progress per player
// ---------------------------------------------------------------------------

/// Progress on a single quest objective.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestProgressData {
    pub quest_id: QuestId,
    pub objective_index: usize,
    pub current: u32,
    pub target: u32,
    pub completed: bool,
    pub reward_granted: bool,
}

/// Component attached to player entities tracking all active quest progress.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct PlayerQuests {
    pub quests: Vec<QuestProgressData>,
}

impl PlayerQuests {
    /// Find progress for a specific quest by id.
    pub fn find_quest(&self, quest_id: QuestId) -> Option<&QuestProgressData> {
        self.quests.iter().find(|q| q.quest_id == quest_id)
    }

    /// Find mutable progress for a specific quest by id.
    pub fn find_quest_mut(&mut self, quest_id: QuestId) -> Option<&mut QuestProgressData> {
        self.quests.iter_mut().find(|q| q.quest_id == quest_id)
    }
}

// ---------------------------------------------------------------------------
// Resource: quest definitions (populated from WorldConfig)
// ---------------------------------------------------------------------------

/// The quest definitions extracted from the world config.
/// Populated by the server plugin; consumed by quest systems.
#[derive(Resource)]
pub struct QuestDefs {
    pub configs: Vec<game_core::world_config::QuestConfig>,
}

// ---------------------------------------------------------------------------
// Resource: persistence handle (optional)
// ---------------------------------------------------------------------------

/// Optional persistence handle for saving/loading quest progress.
#[derive(Resource, Clone)]
pub struct QuestPersistence(pub Option<game_persistence::PersistenceHandle>);
