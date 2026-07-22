use bevy::prelude::*;
use game_protocol::PlayerPosition;

/// Copies `Transform.translation` → `PlayerPosition` for all player entities.
/// Must run AFTER `integrate_velocity` so that the replicated position reflects
/// the latest server-authoritative movement.
pub fn sync_transform_to_position(mut query: Query<(&Transform, &mut PlayerPosition)>) {
    for (transform, mut pos) in query.iter_mut() {
        pos.0 = transform.translation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_test_app;
    use game_core::id::PlayerId;
    use player::spawn_player;

    #[test]
    fn sync_transform_to_position_copies_translation() {
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
            .insert(Transform::from_translation(Vec3::new(15.0, 0.0, 25.0)));
        app.world_mut().flush();

        app.add_systems(FixedUpdate, sync_transform_to_position);
        app.world_mut().run_schedule(FixedUpdate);

        let pos = app.world().get::<PlayerPosition>(entity).unwrap();
        assert!((pos.x - 15.0).abs() < 1e-5);
        assert!((pos.z - 25.0).abs() < 1e-5);
    }
}
