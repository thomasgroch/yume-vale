use avian3d::prelude::Position;
use bevy::prelude::*;
use game_protocol::PlayerPosition;

/// Copies avian3d's `Position` → `PlayerPosition` for all player entities.
///
/// The server disables avian's `PhysicsTransformPlugin` (replication runs off
/// avian's own `Position`/`Rotation`, not `Transform`), so `Transform` is
/// never updated by physics in production — this must read `Position`
/// directly. Must run after the physics step so the replicated position
/// reflects the latest server-authoritative movement.
pub fn sync_physics_position_to_player_position(
    mut query: Query<(&Position, &mut PlayerPosition)>,
) {
    for (position, mut pos) in query.iter_mut() {
        pos.0 = position.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_test_app;
    use game_core::id::PlayerId;
    use player::spawn_player;

    #[test]
    fn sync_physics_position_to_player_position_copies_avian_position() {
        let mut app = build_test_app();
        let entity = spawn_player(
            &mut app.world_mut().commands(),
            PlayerId::new(1),
            "Test".into(),
            Vec3::new(5.0, 0.0, 10.0),
        );
        app.world_mut().flush();

        app.world_mut()
            .entity_mut(entity)
            .insert(Position(Vec3::new(15.0, 0.0, 25.0)));
        app.world_mut().flush();

        app.add_systems(FixedUpdate, sync_physics_position_to_player_position);
        app.world_mut().run_schedule(FixedUpdate);

        let pos = app.world().get::<PlayerPosition>(entity).unwrap();
        assert!((pos.x - 15.0).abs() < 1e-5);
        assert!((pos.z - 25.0).abs() < 1e-5);
    }
}
