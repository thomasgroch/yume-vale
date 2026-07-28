use bevy::app::{App, FixedUpdate, Plugin};

use crate::systems::*;

pub struct CreaturePlugin;

impl Plugin for CreaturePlugin {
    fn build(&self, app: &mut App) {
        // Creature AI and feed systems run each FixedUpdate.
        // sync_creature_position is added by the server plugin after the
        // physics writeback schedule point.
        app.add_systems(FixedUpdate, tick_feed_cooldowns);
        app.add_systems(FixedUpdate, wander_ai);
    }
}
