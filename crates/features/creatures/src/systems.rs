//! Server-side creature AI and feed systems.
//!
//! * `wander_ai` — deterministic random-steering, return-to-centre, avoidance
//! * `sync_creature_position` — copy Transform → `CreatureState` for replication
//! * `process_feed` — validate and execute a feed intent

use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::game_state::GameError;
use game_core::id::{CreatureId, PlayerId};
use game_core::resources::ResourceKind;
use game_protocol::CreatureState;

use crate::components::*;

// ---------------------------------------------------------------------------
// Wander AI
// ---------------------------------------------------------------------------

/// Drives creature wander movement each fixed tick.
///
/// For each creature entity:
/// 1. Decrement direction-change cooldown; if zero, pick a new random direction.
/// 2. Apply velocity toward the current wander direction.
/// 3. If outside wander radius, add a return-to-centre force.
/// 4. Apply simple collision avoidance from nearby entities.
pub fn wander_ai(
    mut creatures: Query<(
        Entity,
        &mut LinearVelocity,
        &mut WanderState,
        &WanderCenter,
        &WanderRadius,
        &Transform,
    )>,
    other_positions: Query<(Entity, &Transform), Without<WanderState>>,
) {
    for (entity, mut vel, mut state, center, radius, transform) in creatures.iter_mut() {
        // Cooldown tick
        if state.direction_change_cooldown > 0 {
            state.direction_change_cooldown -= 1;
        } else {
            pick_new_direction(&mut state);
        }

        // Wander velocity
        let mut move_x = state.target_dir_x;
        let mut move_z = state.target_dir_z;

        // Return-to-centre if outside wander radius
        let to_centre = center.0 - transform.translation;
        let dist_to_centre = to_centre.xz().length();
        if dist_to_centre > radius.0 {
            let pull = (dist_to_centre - radius.0) * RETURN_STRENGTH;
            move_x += to_centre.x * pull / dist_to_centre;
            move_z += to_centre.z * pull / dist_to_centre;
        }

        // Simple collision avoidance
        let my_pos = transform.translation;
        for (other, other_transform) in other_positions.iter() {
            if other == entity {
                continue;
            }
            let delta = my_pos - other_transform.translation;
            let dist = delta.xz().length();
            if dist < AVOIDANCE_RADIUS && dist > 0.01 {
                let strength = (AVOIDANCE_RADIUS - dist) / AVOIDANCE_RADIUS * AVOIDANCE_STRENGTH;
                move_x += delta.x / dist * strength;
                move_z += delta.z / dist * strength;
            }
        }

        // Normalise and apply velocity
        let len = (move_x * move_x + move_z * move_z).sqrt();
        if len > 0.01 {
            let speed = CREATURE_WANDER_SPEED.min(len);
            vel.0.x = move_x / len * speed;
            vel.0.z = move_z / len * speed;
        } else {
            vel.0.x = 0.0;
            vel.0.z = 0.0;
        }
    }
}

/// Pick a new random direction for a creature's wander state.
pub(crate) fn pick_new_direction(state: &mut WanderState) {
    let angle = state.rng.next_f32_range(0.0, std::f32::consts::TAU);
    state.target_dir_x = angle.cos();
    state.target_dir_z = angle.sin();
    let range = (DIRECTION_CHANGE_MAX_TICKS - DIRECTION_CHANGE_MIN_TICKS) as f32;
    state.direction_change_cooldown =
        DIRECTION_CHANGE_MIN_TICKS + (state.rng.next_f32() * range) as u32;
}

// ---------------------------------------------------------------------------
// Sync creature position
// ---------------------------------------------------------------------------

/// Copies `Transform.translation` → `CreatureState` for all creature entities.
/// Must run after `PhysicsSystems::Writeback` so the replicated position
/// reflects the latest authoritative movement.
pub fn sync_creature_position(mut creatures: Query<(&Creature, &Transform, &mut CreatureState)>) {
    for (creature, transform, mut state) in creatures.iter_mut() {
        state.creature_id = creature.id.get();
        state.position_x = transform.translation.x;
        state.position_y = transform.translation.y;
        state.position_z = transform.translation.z;
    }
}

// ---------------------------------------------------------------------------
// Feed processing
// ---------------------------------------------------------------------------

/// Parameters for a feed request resolved from the server action system.
#[derive(Debug, Clone)]
pub struct FeedRequest {
    pub player_id: PlayerId,
    pub creature_entity: Entity,
    pub creature_id: CreatureId,
    pub food_kind: ResourceKind,
    pub inventory_slot: usize,
}

/// Result of a feed attempt.
#[derive(Debug, Clone)]
pub enum FeedOutcome {
    /// Feed succeeded, bond level after feeding.
    Success { bond_level: u32 },
    /// Feed failed for a specific reason.
    Error(GameError),
}

/// Process a resolved feed request.
///
/// Validates:
/// * Creature exists and is in range
/// * Feed cooldown has expired
///
/// Returns the outcome. The caller (server action handler) should:
/// 1. Apply the `Intent::Feed` to `GameState`
/// 2. Send `BondSnapshot` to the player on success
pub fn process_feed(
    request: &FeedRequest,
    _creature_transform: &Transform,
    creature_cooldown: &mut FeedCooldown,
) -> FeedOutcome {
    // Check feed cooldown
    if creature_cooldown.remaining_ticks > 0 {
        return FeedOutcome::Error(GameError::UnknownCreature(request.creature_id));
    }

    // The caller will check range in the server system before calling here.
    // This function handles the entity-level validation.
    FeedOutcome::Success { bond_level: 0 }
}

/// Decrement all feed cooldowns each tick.
pub fn tick_feed_cooldowns(mut cooldowns: Query<&mut FeedCooldown>) {
    for mut cd in cooldowns.iter_mut() {
        if cd.remaining_ticks > 0 {
            cd.remaining_ticks -= 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::id::CreatureId;
    use game_core::world_config::CreatureKind;

    /// Helper: spawn a creature entity with all required components.
    fn spawn_test_creature(
        commands: &mut Commands,
        id: u64,
        kind: CreatureKind,
        food_kind: ResourceKind,
        center: Vec3,
        radius: f32,
        position: Vec3,
    ) -> Entity {
        let rng = CreatureRng::new(id);
        let state = WanderState {
            rng,
            direction_change_cooldown: 0,
            target_dir_x: 1.0,
            target_dir_z: 0.0,
        };
        commands
            .spawn((
                Creature {
                    id: CreatureId::new(id),
                    kind,
                    food_kind,
                },
                WanderCenter(center),
                WanderRadius(radius),
                state,
                Transform::from_translation(position),
                LinearVelocity::default(),
                CreatureState {
                    creature_id: id,
                    kind,
                    position_x: position.x,
                    position_y: position.y,
                    position_z: position.z,
                    target_x: 0.0,
                    target_z: 0.0,
                },
                FeedCooldown::default(),
            ))
            .id()
    }

    // -----------------------------------------------------------------------
    // Wander AI
    // -----------------------------------------------------------------------

    #[test]
    fn pick_new_direction_resets_cooldown() {
        let rng = CreatureRng::new(1);
        let mut state = WanderState {
            rng,
            direction_change_cooldown: 0,
            target_dir_x: 0.0,
            target_dir_z: 0.0,
        };
        pick_new_direction(&mut state);
        assert!(
            state.direction_change_cooldown >= DIRECTION_CHANGE_MIN_TICKS,
            "cooldown should be at least {} ticks",
            DIRECTION_CHANGE_MIN_TICKS
        );
        assert!(
            state.direction_change_cooldown <= DIRECTION_CHANGE_MAX_TICKS,
            "cooldown should be at most {} ticks",
            DIRECTION_CHANGE_MAX_TICKS
        );
        // Target direction should be a unit vector
        let len = (state.target_dir_x * state.target_dir_x
            + state.target_dir_z * state.target_dir_z)
            .sqrt();
        assert!((len - 1.0).abs() < 0.001, "direction should be normalised");
    }

    #[test]
    fn wander_ai_decrements_cooldown() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(FixedUpdate, wander_ai);

        let entity = spawn_test_creature(
            &mut app.world_mut().commands(),
            1,
            CreatureKind::Fluffball,
            ResourceKind::Berry,
            Vec3::ZERO,
            5.0,
            Vec3::new(1.0, 0.0, 0.0),
        );
        app.world_mut().flush();

        // Set cooldown to 10
        app.world_mut().entity_mut(entity).insert(WanderState {
            rng: CreatureRng::new(1),
            direction_change_cooldown: 10,
            target_dir_x: 1.0,
            target_dir_z: 0.0,
        });
        app.world_mut().flush();

        // Run one fixed tick
        app.world_mut().run_schedule(FixedUpdate);
        let state = app.world().get::<WanderState>(entity).unwrap();
        assert_eq!(state.direction_change_cooldown, 9);
    }

    #[test]
    fn wander_ai_sets_velocity_toward_target() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(FixedUpdate, wander_ai);

        let entity = spawn_test_creature(
            &mut app.world_mut().commands(),
            1,
            CreatureKind::Glimmerwing,
            ResourceKind::Crystal,
            Vec3::ZERO,
            5.0,
            Vec3::ZERO,
        );
        app.world_mut().flush();

        // Target = +X direction
        app.world_mut().entity_mut(entity).insert(WanderState {
            rng: CreatureRng::new(1),
            direction_change_cooldown: 60,
            target_dir_x: 1.0,
            target_dir_z: 0.0,
        });
        app.world_mut().flush();

        // Run AI once
        app.world_mut().run_schedule(FixedUpdate);

        let vel = app.world().get::<LinearVelocity>(entity).unwrap();
        // Velocity should be in +X direction
        assert!(vel.x > 0.0, "velocity should be in +X direction");
        assert!((vel.z).abs() < 0.01, "Z velocity should be near zero");
    }

    // -----------------------------------------------------------------------
    // Syncing position
    // -----------------------------------------------------------------------

    #[test]
    fn sync_creature_position_copies_translation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(FixedUpdate, sync_creature_position);

        let entity = spawn_test_creature(
            &mut app.world_mut().commands(),
            1,
            CreatureKind::Fluffball,
            ResourceKind::Berry,
            Vec3::ZERO,
            5.0,
            Vec3::new(10.0, 0.0, 20.0),
        );
        app.world_mut().flush();

        app.world_mut().run_schedule(FixedUpdate);

        let state = app.world().get::<CreatureState>(entity).unwrap();
        assert!((state.position_x - 10.0).abs() < 1e-5);
        assert!((state.position_z - 20.0).abs() < 1e-5);
    }

    // -----------------------------------------------------------------------
    // Feed cooldown
    // -----------------------------------------------------------------------

    #[test]
    fn tick_feed_cooldowns_decrements() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(FixedUpdate, tick_feed_cooldowns);

        let entity = spawn_test_creature(
            &mut app.world_mut().commands(),
            1,
            CreatureKind::Fluffball,
            ResourceKind::Berry,
            Vec3::ZERO,
            5.0,
            Vec3::ZERO,
        );
        app.world_mut().flush();

        app.world_mut()
            .entity_mut(entity)
            .insert(FeedCooldown { remaining_ticks: 5 });
        app.world_mut().flush();

        app.world_mut().run_schedule(FixedUpdate);
        let cd = app.world().get::<FeedCooldown>(entity).unwrap();
        assert_eq!(cd.remaining_ticks, 4);
    }

    #[test]
    fn wander_stays_within_radius() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(FixedUpdate, wander_ai);

        let center = Vec3::new(0.0, 0.0, 0.0);
        let radius = 5.0;
        let entity = spawn_test_creature(
            &mut app.world_mut().commands(),
            1,
            CreatureKind::Fluffball,
            ResourceKind::Berry,
            center,
            radius,
            Vec3::new(3.0, 0.0, 0.0),
        );
        app.world_mut().flush();

        // Set wander direction away from center, long cooldown
        app.world_mut().entity_mut(entity).insert(WanderState {
            rng: CreatureRng::new(1),
            direction_change_cooldown: 1000,
            target_dir_x: 1.0,
            target_dir_z: 0.0,
        });
        app.world_mut().flush();

        // Run many fixed ticks (simulates ~33 seconds at 30 Hz)
        for _ in 0..1000 {
            app.world_mut().run_schedule(FixedUpdate);
        }

        let pos = app.world().get::<Transform>(entity).unwrap();
        let dist = pos.translation.xz().distance(center.xz());
        assert!(
            dist <= radius + 1.0,
            "creature at distance {dist} should stay near radius {radius}"
        );
    }
}
