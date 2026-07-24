use bevy::prelude::*;
use bevy_tnua::builtins::TnuaBuiltinJump;
use bevy_tnua::prelude::*;
use game_core::constants::{RUN_SPEED, WALK_SPEED};
use game_protocol::*;

use crate::components::*;
use crate::events::ActionStarted;
use crate::scheme::YumeScheme;

/// Feeds the Tnua walk basis from the current `PlayerMovement` input state.
/// `desired_motion` is normalized direction × speed fraction (config speed is
/// `RUN_SPEED`, so walking is the walk/run ratio); `desired_forward` turns the
/// character toward its movement direction. A held `jump` keeps the `Jump`
/// action fed (Tnua handles ground/coyote checks).
pub fn feed_walk_basis(mut query: Query<(&PlayerMovement, &mut TnuaController<YumeScheme>)>) {
    for (movement, mut controller) in query.iter_mut() {
        controller.initiate_action_feeding();
        let dir = movement.direction.0;
        let speed_factor = if movement.running {
            1.0
        } else {
            WALK_SPEED / RUN_SPEED
        };
        controller.basis = TnuaBuiltinWalk {
            desired_motion: dir * speed_factor,
            desired_forward: Dir3::new(dir).ok(),
        };
        if movement.jump {
            controller.action(YumeScheme::Jump(TnuaBuiltinJump::default()));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::math::Direction;

    fn app_with_feed() -> App {
        let mut app = App::new();
        app.add_systems(Update, feed_walk_basis);
        app
    }

    #[test]
    fn feed_walk_basis_walks_at_run_fraction() {
        let mut app = app_with_feed();
        let entity = app
            .world_mut()
            .spawn((
                PlayerMovement {
                    direction: Direction::from_xz(1.0, 0.0).unwrap(),
                    running: false,
                    jump: false,
                },
                TnuaController::<YumeScheme>::default(),
            ))
            .id();
        app.update();

        let controller = app
            .world()
            .get::<TnuaController<YumeScheme>>(entity)
            .unwrap();
        let basis: &TnuaBuiltinWalk = &controller.basis;
        assert!((basis.desired_motion.x - WALK_SPEED / RUN_SPEED).abs() < 1e-5);
        assert!(basis.desired_forward.is_some());
    }

    #[test]
    fn feed_walk_basis_runs_at_full_speed() {
        let mut app = app_with_feed();
        let entity = app
            .world_mut()
            .spawn((
                PlayerMovement {
                    direction: Direction::from_xz(0.0, 1.0).unwrap(),
                    running: true,
                    jump: false,
                },
                TnuaController::<YumeScheme>::default(),
            ))
            .id();
        app.update();

        let controller = app
            .world()
            .get::<TnuaController<YumeScheme>>(entity)
            .unwrap();
        let basis: &TnuaBuiltinWalk = &controller.basis;
        assert!((basis.desired_motion.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn feed_walk_basis_feeds_jump_action() {
        // `TnuaController::action()` panics unless `initiate_action_feeding()`
        // ran first in the same frame — this test passing proves the system
        // initiates feeding and feeds the Jump action without panicking.
        let mut app = app_with_feed();
        let entity = app
            .world_mut()
            .spawn((
                PlayerMovement {
                    jump: true,
                    ..PlayerMovement::default()
                },
                TnuaController::<YumeScheme>::default(),
            ))
            .id();
        app.update();
        app.update();

        let controller = app
            .world()
            .get::<TnuaController<YumeScheme>>(entity)
            .unwrap();
        let basis: &TnuaBuiltinWalk = &controller.basis;
        assert_eq!(basis.desired_motion, Vec3::ZERO);
    }

    #[test]
    fn feed_walk_basis_stands_still_without_direction() {
        let mut app = app_with_feed();
        let entity = app
            .world_mut()
            .spawn((
                PlayerMovement::default(),
                TnuaController::<YumeScheme>::default(),
            ))
            .id();
        app.update();

        let controller = app
            .world()
            .get::<TnuaController<YumeScheme>>(entity)
            .unwrap();
        let basis: &TnuaBuiltinWalk = &controller.basis;
        assert_eq!(basis.desired_motion, Vec3::ZERO);
        assert!(basis.desired_forward.is_none());
    }
}
