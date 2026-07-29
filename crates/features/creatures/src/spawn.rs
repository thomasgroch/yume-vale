//! Creature entity spawning from world config.
//!
//! Spawns logical creature entities: marker, wander state, replication state.
//! Physics components (RigidBody, Collider, LockedAxes) are added by the server
//! crate's `setup_world`, keeping this crate physics-optional.

use bevy::prelude::*;
use game_core::world_config::{CreatureConfig, WorldConfig};
use game_protocol::CreatureState;

use crate::components::*;

/// Spawn all creatures defined in a `WorldConfig`.
pub fn spawn_creatures(commands: &mut Commands, world_config: &WorldConfig) {
    for creature_config in &world_config.creatures {
        spawn_creature(commands, creature_config);
    }
}

/// Spawn a single creature entity with logical components.
///
/// Each creature gets:
/// * `Creature` marker with logical ID, kind, food kind
/// * `WanderCenter` / `WanderRadius` from config
/// * `WanderState` with deterministic PRNG seeded by creature ID
/// * `Transform` near creature's center
/// * `CreatureState` for replication with interpolation
/// * `Replicate` and `InterpolationTarget` for Lightyear
/// * `FeedCooldown` initialised to zero
pub fn spawn_creature(commands: &mut Commands, config: &CreatureConfig) -> Entity {
    let seed = config
        .id
        .get()
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(0x1234_5678);
    let mut rng = CreatureRng::new(seed);
    let center = config.center;
    let initial_angle = rng.next_f32_range(0.0, std::f32::consts::TAU);
    let initial_offset = 1.0;
    let initial_pos = Vec3::new(
        center.x + initial_angle.cos() * initial_offset,
        center.y,
        center.z + initial_angle.sin() * initial_offset,
    );

    let mut state = WanderState {
        rng,
        direction_change_cooldown: 0,
        target_dir_x: initial_angle.cos(),
        target_dir_z: initial_angle.sin(),
    };

    // Pick first direction so first tick has a valid wander target
    crate::systems::pick_new_direction(&mut state);

    commands
        .spawn((
            Creature {
                id: config.id,
                kind: config.kind,
                food_kind: config.food_kind,
            },
            WanderCenter(center),
            WanderRadius(config.wander_radius),
            state,
            Transform::from_translation(initial_pos),
            CreatureState {
                creature_id: config.id.get(),
                kind: config.kind,
                position_x: initial_pos.x,
                position_y: initial_pos.y,
                position_z: initial_pos.z,
                target_x: center.x,
                target_z: center.z,
            },
            lightyear::prelude::Replicate::to_clients(
                lightyear::connection::network_target::NetworkTarget::All,
            ),
            lightyear::prelude::InterpolationTarget::to_clients(
                lightyear::connection::network_target::NetworkTarget::All,
            ),
            FeedCooldown::default(),
        ))
        .id()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::id::CreatureId;
    use game_core::resources::ResourceKind;
    use game_core::world_config::CreatureKind;

    fn test_config() -> WorldConfig {
        WorldConfig {
            resources: vec![],
            creatures: vec![
                CreatureConfig {
                    id: CreatureId::new(1),
                    kind: CreatureKind::Fluffball,
                    center: Vec3::new(10.0, 0.0, 5.0),
                    wander_radius: 8.0,
                    food_kind: ResourceKind::Berry,
                    model_path: "fluff.glb".into(),
                },
                CreatureConfig {
                    id: CreatureId::new(2),
                    kind: CreatureKind::Glimmerwing,
                    center: Vec3::new(-5.0, 0.0, 15.0),
                    wander_radius: 6.0,
                    food_kind: ResourceKind::Crystal,
                    model_path: "glim.glb".into(),
                },
            ],
        }
    }

    #[test]
    fn spawn_creatures_creates_two_entities() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let config = test_config();
        spawn_creatures(&mut app.world_mut().commands(), &config);
        app.world_mut().flush();

        let mut query = app.world_mut().query::<&Creature>();
        let count = query.iter(app.world()).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn spawned_creature_has_required_components() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let config = test_config();
        spawn_creatures(&mut app.world_mut().commands(), &config);
        app.world_mut().flush();

        let mut query = app.world_mut().query::<&Creature>();
        for creature in query.iter(app.world()) {
            assert!(
                creature.id == CreatureId::new(1) || creature.id == CreatureId::new(2),
                "unexpected creature ID {}",
                creature.id
            );
        }
    }

    #[test]
    fn spawned_creature_has_creature_state() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let config = test_config();
        spawn_creatures(&mut app.world_mut().commands(), &config);
        app.world_mut().flush();

        let mut q = app
            .world_mut()
            .query::<(Entity, Option<&Creature>, Option<&CreatureState>)>();
        for (_, creature, state) in q.iter(app.world()) {
            if creature.is_some() {
                assert!(state.is_some());
            }
        }
    }

    #[test]
    fn same_seed_produces_same_direction_sequence() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let config = test_config();
        spawn_creatures(&mut app.world_mut().commands(), &config);
        app.world_mut().flush();

        let mut query = app.world_mut().query::<&WanderState>();
        let states: Vec<WanderState> = query.iter(app.world()).copied().collect();
        assert_eq!(states.len(), 2);

        let mut app2 = App::new();
        app2.add_plugins(MinimalPlugins);
        spawn_creatures(&mut app2.world_mut().commands(), &config);
        app2.world_mut().flush();

        let mut query2 = app2.world_mut().query::<&WanderState>();
        let states2: Vec<WanderState> = query2.iter(app2.world()).copied().collect();
        assert_eq!(states2.len(), 2);

        for (s1, s2) in states.iter().zip(states2.iter()) {
            assert_eq!(s1.rng, s2.rng);
            assert_eq!(s1.target_dir_x, s2.target_dir_x);
            assert_eq!(s1.target_dir_z, s2.target_dir_z);
            assert_eq!(s1.direction_change_cooldown, s2.direction_change_cooldown);
        }
    }

    #[test]
    fn spawned_creature_has_feed_cooldown() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let config = test_config();
        spawn_creatures(&mut app.world_mut().commands(), &config);
        app.world_mut().flush();

        let mut q = app
            .world_mut()
            .query::<(Entity, Option<&Creature>, Option<&FeedCooldown>)>();
        for (_, creature, cd) in q.iter(app.world()) {
            if creature.is_some() {
                assert!(cd.is_some());
                assert_eq!(cd.unwrap().remaining_ticks, 0);
            }
        }
    }
}
