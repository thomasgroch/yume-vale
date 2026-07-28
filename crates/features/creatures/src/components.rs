use bevy::prelude::*;
use game_core::id::CreatureId;
use game_core::resources::ResourceKind;
use game_core::world_config::CreatureKind;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Deterministic PRNG for creature wander
// ---------------------------------------------------------------------------

/// A simple LCG (Linear Congruential Generator) seeded by creature ID.
///
/// Produces a deterministic sequence of f32 values in [0, 1) for any given
/// starting seed. The underlying algorithm is the same LCG used by PCG and
/// generational-arena for its table-size computation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CreatureRng(pub u64);

impl CreatureRng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Returns the next f32 in [0.0, 1.0).
    pub fn next_f32(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as u32 as f32) / (u32::MAX as f32)
    }

    /// Returns the next f32 in [min, max).
    pub fn next_f32_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

// ---------------------------------------------------------------------------
// Wander state
// ---------------------------------------------------------------------------

/// Tracks the current wander state for a creature AI.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WanderState {
    /// Seed for deterministic PRNG (derived from creature ID).
    pub rng: CreatureRng,
    /// Ticks remaining before picking a new direction.
    pub direction_change_cooldown: u32,
    /// Current wander target direction (normalised xz).
    pub target_dir_x: f32,
    pub target_dir_z: f32,
}

// ---------------------------------------------------------------------------
// Creature entity marker
// ---------------------------------------------------------------------------

/// Marker component for a spawned creature entity.
/// Stores the logical creature ID and its food kind.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: CreatureId,
    pub kind: CreatureKind,
    pub food_kind: ResourceKind,
}

// ---------------------------------------------------------------------------
// Creature center anchor
// ---------------------------------------------------------------------------

/// The center of a creature's wander area (read from world config).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct WanderCenter(pub bevy::prelude::Vec3);

/// Wander radius (from world config).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct WanderRadius(pub f32);

// ---------------------------------------------------------------------------
// Feed cooldown tracking
// ---------------------------------------------------------------------------

/// Per-creature feed cooldown in ticks.
/// Prevents a single creature from being fed too rapidly.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct FeedCooldown {
    pub remaining_ticks: u32,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Creature wander speed (units per second).
pub const CREATURE_WANDER_SPEED: f32 = 2.0;

/// Minimum ticks between direction changes (at 30 Hz = ~2 s).
pub const DIRECTION_CHANGE_MIN_TICKS: u32 = 60;

/// Maximum ticks between direction changes (at 30 Hz = ~6 s).
pub const DIRECTION_CHANGE_MAX_TICKS: u32 = 180;

/// Return-to-centre strength when outside wander radius (proportional gain).
pub const RETURN_STRENGTH: f32 = 0.5;

/// Collision avoidance radius (other creatures / players).
pub const AVOIDANCE_RADIUS: f32 = 2.0;

/// Collision avoidance force strength.
pub const AVOIDANCE_STRENGTH: f32 = 3.0;

/// Feed cooldown in ticks (at 30 Hz = 1 s).
pub const FEED_COOLDOWN_TICKS: u32 = 30;

/// Max distance for feeding a creature.
pub const FEED_RANGE: f32 = 3.0;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creature_rng_deterministic_same_seed() {
        let mut a = CreatureRng::new(42);
        let mut b = CreatureRng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_f32(), b.next_f32());
        }
    }

    #[test]
    fn creature_rng_different_seed_different_sequence() {
        let mut a = CreatureRng::new(42);
        let mut b = CreatureRng::new(99);
        // It is astronomically unlikely that 10 values all match for different seeds.
        let all_same = (0..10).all(|_| a.next_f32() == b.next_f32());
        assert!(
            !all_same,
            "different seeds should produce different sequences"
        );
    }

    #[test]
    fn creature_rng_next_f32_in_range() {
        let mut rng = CreatureRng::new(1);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v), "value {v} out of range [0, 1)");
        }
    }

    #[test]
    fn creature_rng_next_f32_range_bounds() {
        let mut rng = CreatureRng::new(7);
        for _ in 0..1000 {
            let v = rng.next_f32_range(-5.0, 10.0);
            assert!((-5.0..10.0).contains(&v), "value {v} out of range [-5, 10)");
        }
    }

    #[test]
    fn creature_rng_serde_roundtrip() {
        let orig = CreatureRng::new(12345);
        let json = serde_json::to_string(&orig).unwrap();
        let back: CreatureRng = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
        // Verify sequence continues after deserialization
        let mut a = orig;
        let mut b = back;
        for _ in 0..10 {
            assert_eq!(a.next_f32(), b.next_f32());
        }
    }

    #[test]
    fn wander_state_contains_rng_and_target() {
        let rng = CreatureRng::new(10);
        let state = WanderState {
            rng,
            direction_change_cooldown: 5,
            target_dir_x: 1.0,
            target_dir_z: 0.0,
        };
        assert_eq!(state.rng, CreatureRng::new(10));
        assert_eq!(state.direction_change_cooldown, 5);
    }
}
