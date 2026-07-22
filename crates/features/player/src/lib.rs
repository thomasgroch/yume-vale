pub mod components;
pub mod events;
pub mod plugin;
pub mod systems;

pub use components::*;
pub use events::*;
pub use plugin::{PlayerMovementSet, PlayerPlugin};
pub use systems::*;

use bevy::prelude::*;
use game_core::id::PlayerId;
use game_protocol::*;

pub fn spawn_player(commands: &mut Commands, id: PlayerId, name: String, position: Vec3) -> Entity {
    commands
        .spawn((
            Player { id },
            PlayerName(name),
            PlayerMovement::default(),
            Velocity(Vec3::ZERO),
            PlayerPosition(position),
            ReplicatedPlayerInput(game_core::player_state::PlayerInput::default()),
            Transform::from_translation(position),
        ))
        .id()
}

pub fn despawn_player(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).despawn();
}

pub mod prelude {
    pub use crate::components::*;
    pub use crate::events::*;
    pub use crate::plugin::PlayerPlugin;
    pub use crate::{despawn_player, spawn_player};
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::observer::On;
    use game_core::actions::ActionKind;
    use game_core::constants::{GROUND_Y, RUN_SPEED, WALK_SPEED};
    use game_core::math::Direction;
    use game_core::player_state::PlayerInput;

    #[test]
    fn spawn_player_has_all_components() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = spawn_player(
            &mut app.world_mut().commands(),
            PlayerId::new(1),
            "Yume".into(),
            Vec3::new(10.0, 0.0, 20.0),
        );
        app.world_mut().flush();

        let world = app.world();
        assert!(world.get::<Player>(entity).is_some());
        assert!(world.get::<PlayerName>(entity).is_some());
        assert!(world.get::<PlayerMovement>(entity).is_some());
        assert!(world.get::<Velocity>(entity).is_some());
        assert!(world.get::<PlayerPosition>(entity).is_some());
        assert!(world.get::<Transform>(entity).is_some());
    }

    #[test]
    fn apply_movement_sets_walk_velocity() {
        let movement = PlayerMovement {
            direction: Direction::from_xz(1.0, 0.0).unwrap(),
            running: false,
        };

        let result = apply_movement_input_logic(&movement);

        assert!((result.0.x - WALK_SPEED).abs() < 1e-5);
        assert!((result.0.y).abs() < 1e-5);
        assert!((result.0.z).abs() < 1e-5);
    }

    #[test]
    fn apply_movement_sets_run_velocity() {
        let movement = PlayerMovement {
            direction: Direction::from_xz(0.0, 1.0).unwrap(),
            running: true,
        };

        let result = apply_movement_input_logic(&movement);

        assert!((result.0.x).abs() < 1e-5);
        assert!((result.0.y).abs() < 1e-5);
        assert!((result.0.z - RUN_SPEED).abs() < 1e-5);
    }

    #[test]
    fn integrate_velocity_moves_transform() {
        let velocity = Velocity(Vec3::new(3.0, 0.0, 0.0));
        let mut transform = Transform::from_translation(Vec3::new(0.0, 0.0, 0.0));

        integrate_velocity_logic(0.1, &velocity, &mut transform);

        assert!(transform.translation.x > 0.0);
        assert_eq!(transform.translation.y, GROUND_Y);
    }

    #[test]
    fn process_actions_emits_event() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<TestActions>();
        app.add_observer(
            |event: On<ActionStarted>, mut actions: ResMut<TestActions>| {
                actions.0.push(event.event().clone());
            },
        );

        let player_id = PlayerId::new(1);
        let input = PlayerInput {
            movement: Direction::zero(),
            run: false,
            interact: None,
            action: Some(ActionKind::Collect),
        };
        let entity = app
            .world_mut()
            .spawn((Player { id: player_id }, ReplicatedPlayerInput(input)))
            .id();

        app.world_mut().flush();

        let player = app.world().get::<Player>(entity).unwrap().id;
        let action = app
            .world()
            .get::<ReplicatedPlayerInput>(entity)
            .unwrap()
            .0
            .action
            .unwrap();
        app.world_mut().commands().trigger(ActionStarted {
            player_id: player,
            action,
        });
        app.world_mut().flush();

        let actions = app.world().resource::<TestActions>();
        assert_eq!(actions.0.len(), 1);
        assert_eq!(actions.0[0].player_id, player_id);
        assert_eq!(actions.0[0].action, ActionKind::Collect);
    }

    #[test]
    fn despawn_player_removes_entity() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app
            .world_mut()
            .spawn(Player {
                id: PlayerId::new(1),
            })
            .id();

        app.world_mut().flush();
        assert!(app.world().get::<Player>(entity).is_some());

        app.world_mut().commands().entity(entity).despawn();
        app.world_mut().flush();
        assert!(app.world().get::<Player>(entity).is_none());
    }

    #[derive(Resource, Default)]
    struct TestActions(Vec<ActionStarted>);

    fn apply_movement_input_logic(movement: &PlayerMovement) -> Velocity {
        let speed = if movement.running {
            RUN_SPEED
        } else {
            WALK_SPEED
        };
        let dir = movement.direction.0;
        Velocity(Vec3::new(dir.x * speed, 0.0, dir.z * speed))
    }

    fn integrate_velocity_logic(dt: f32, velocity: &Velocity, transform: &mut Transform) {
        transform.translation += velocity.0 * dt;
        transform.translation.y = GROUND_Y;
    }
}
