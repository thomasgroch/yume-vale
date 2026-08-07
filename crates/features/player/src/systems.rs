use bevy::prelude::*;
use game_protocol::*;

use crate::components::*;
use crate::events::ActionStarted;
#[cfg(feature = "physics")]
pub use crate::physics::apply_predicted_movement;

pub fn process_actions(query: Query<(&Player, &ReplicatedPlayerInput)>, mut commands: Commands) {
    for (player, input) in query.iter() {
        if let Some(action) = input.0.action {
            commands.trigger(ActionStarted {
                player_id: player.id,
                action,
            });
        }
    }
}
