use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use lightyear::prelude::*;

use crate::components::*;
use crate::systems::*;

pub struct PlayerPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerMovementSet;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Register Player and PlayerName for Lightyear replication.
        app.component::<Player>().replicate();
        app.component::<PlayerName>().replicate();

        app.add_systems(FixedUpdate, apply_movement_input.in_set(PlayerMovementSet))
            .add_systems(
                FixedUpdate,
                (integrate_velocity
                    .after(apply_movement_input)
                    .in_set(PlayerMovementSet),),
            )
            .add_systems(FixedUpdate, process_actions);
    }
}
