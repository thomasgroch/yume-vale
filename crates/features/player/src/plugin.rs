use bevy::app::{App, FixedUpdate, Plugin};
#[cfg(feature = "physics")]
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::SystemSet;
#[cfg(feature = "physics")]
use bevy::ecs::schedule::common_conditions::resource_exists;
use lightyear::prelude::*;

#[cfg(feature = "physics")]
use crate::physics::JumpLatch;

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

        app.add_systems(FixedUpdate, process_actions);
        #[cfg(feature = "physics")]
        app.local_rollback::<JumpLatch>();
        #[cfg(feature = "physics")]
        app.add_systems(
            FixedUpdate,
            apply_predicted_movement
                .in_set(PlayerMovementSet)
                .run_if(resource_exists::<avian3d::collider_tree::ColliderTrees>),
        );
    }
}
