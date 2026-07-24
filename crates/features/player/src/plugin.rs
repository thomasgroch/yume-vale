use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_tnua::TnuaUserControlsSystems;
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

        app.add_systems(
            FixedUpdate,
            (
                feed_walk_basis
                    .in_set(TnuaUserControlsSystems)
                    .in_set(PlayerMovementSet),
                process_actions,
            ),
        );
    }
}
