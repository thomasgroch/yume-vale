//! Quest system — "A Berry Good Start" and future quests.
//!
//! This crate tracks quest progress, handles group-wide exactly-once credit,
//! grants rewards, and sends authoritative `QuestSnapshot` messages to clients.
//!
//! ## Architecture
//!
//! - **Components** ([`components`]): [`PlayerQuests`] per player entity,
//!   [`ResourceCollectedEvent`] as the ingestion event.
//! - **Systems** ([`systems`]): activation, event processing, snapshot,
//!   persistence.
//! - **Plugin** ([`QuestPlugin`]): wires everything into a Bevy app.

pub mod components;
pub mod systems;

pub use components::*;
pub use systems::*;

use bevy::prelude::*;
use game_core::world_config::QuestConfig;

/// Bevy plugin that enables quest tracking.
///
/// Requires:
/// - A [`QuestDefs`] resource (populated from the world config quests).
/// - An observer registered for [`ResourceCollectedEvent`] (supplied by
///   [`on_resource_collected`] in the collect handler).
/// - Optionally a [`QuestPersistence`] resource for database persistence.
pub struct QuestPlugin {
    /// The quest definitions extracted from the world config.
    pub quests: Vec<QuestConfig>,
}

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        // Register the quest event observer.
        // In Bevy 0.19, events are consumed via observers (On<Event>), not
        // EventReader. The observer runs when commands.trigger(event) is called.
        app.add_observer(systems::on_resource_collected);

        // Insert the quest definitions resource.
        let all_quests = self.quests.clone();
        app.insert_resource(QuestDefs {
            configs: all_quests,
        });
    }
}

/// Convenience function: insert the default `QuestPersistence` resource
/// with an optional handle. Call after `QuestPlugin` if persistence is
/// needed.
pub fn insert_quest_persistence(
    app: &mut App,
    handle: Option<game_persistence::PersistenceHandle>,
) {
    app.insert_resource(QuestPersistence(handle));
}
