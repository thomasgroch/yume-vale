use bevy::prelude::*;
use game_core::constants::{GROUND_Y, RUN_SPEED, WALK_SPEED};
use game_protocol::*;

use crate::components::*;
use crate::events::ActionStarted;

pub fn apply_movement_input(mut query: Query<(&PlayerMovement, &mut Velocity)>) {
    for (movement, mut velocity) in query.iter_mut() {
        let speed = if movement.running {
            RUN_SPEED
        } else {
            WALK_SPEED
        };
        let dir = movement.direction.0;
        velocity.0 = Vec3::new(dir.x * speed, 0.0, dir.z * speed);
    }
}

pub fn integrate_velocity(time: Res<Time>, mut query: Query<(&Velocity, &mut Transform)>) {
    let dt = time.delta_secs();
    for (velocity, mut transform) in query.iter_mut() {
        transform.translation += velocity.0 * dt;
        transform.translation.y = GROUND_Y;
    }
}

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
