use crate::arena::ARENA_RADIUS;
use crate::id::{CreatureId, ResourceId};
use crate::resources::ResourceKind;
use glam::Vec3;
use glam::Vec3Swizzles;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Kind of creature in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CreatureKind {
    Fluffball,
    Glimmerwing,
}

/// Configuration for a single resource type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceConfig {
    pub id: ResourceId,
    pub kind: ResourceKind,
    /// Number of spawn positions for this resource.
    pub count: u32,
    /// Amount yielded per harvest action.
    pub yield_amount: u32,
    /// Seconds until a harvested node respawns.
    pub respawn_seconds: f32,
    /// Spawn positions (must match count).
    pub positions: Vec<Vec3>,
    /// Relative asset path for the model.
    pub model_path: String,
}

/// Configuration for a single creature type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatureConfig {
    pub id: CreatureId,
    pub kind: CreatureKind,
    /// Center of the creature's wander area.
    pub center: Vec3,
    /// Radius the creature can wander from center.
    pub wander_radius: f32,
    /// Resource kind this creature feeds on.
    pub food_kind: ResourceKind,
    /// Relative asset path for the model.
    pub model_path: String,
}

/// Top-level world configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorldConfig {
    pub resources: Vec<ResourceConfig>,
    pub creatures: Vec<CreatureConfig>,
}

impl WorldConfig {
    /// Parse a `WorldConfig` from a RON string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &str) -> Result<Self, WorldConfigError> {
        let config: Self = ron::from_str(input)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration, checking all invariants.
    pub fn validate(&self) -> Result<(), WorldConfigError> {
        // --- resources ---
        let mut seen_resource_ids = HashSet::new();
        for res in &self.resources {
            if !seen_resource_ids.insert(res.id) {
                return Err(WorldConfigError::DuplicateResourceId(res.id));
            }

            if res.count == 0 {
                return Err(WorldConfigError::ZeroCount { kind: res.kind });
            }
            if res.yield_amount == 0 {
                return Err(WorldConfigError::ZeroYield { kind: res.kind });
            }
            if res.respawn_seconds <= 0.0 {
                return Err(WorldConfigError::ZeroRespawn {
                    kind: res.kind,
                    respawn_seconds: res.respawn_seconds,
                });
            }
            if res.count as usize != res.positions.len() {
                return Err(WorldConfigError::CountPositionMismatch {
                    kind: res.kind,
                    count: res.count,
                    pos_count: res.positions.len(),
                });
            }
            for (i, pos) in res.positions.iter().enumerate() {
                if pos.xz().length() >= ARENA_RADIUS {
                    return Err(WorldConfigError::ResourcePositionOutOfBounds {
                        kind: res.kind,
                        index: i,
                        pos: *pos,
                    });
                }
            }
        }

        // --- creatures ---
        let mut seen_creature_ids = HashSet::new();
        for creature in &self.creatures {
            if !seen_creature_ids.insert(creature.id) {
                return Err(WorldConfigError::DuplicateCreatureId(creature.id));
            }

            if creature.wander_radius <= 0.0 {
                return Err(WorldConfigError::ZeroWanderRadius {
                    kind: creature.kind,
                    wander_radius: creature.wander_radius,
                });
            }
            if creature.center.xz().length() >= ARENA_RADIUS {
                return Err(WorldConfigError::CreaturePositionOutOfBounds {
                    kind: creature.kind,
                    pos: creature.center,
                });
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum WorldConfigError {
    #[error("RON parse error: {0}")]
    RonError(#[from] ron::error::SpannedError),

    #[error("duplicate resource ID: {0}")]
    DuplicateResourceId(ResourceId),

    #[error("duplicate creature ID: {0}")]
    DuplicateCreatureId(CreatureId),

    #[error("resource {kind:?} has zero count")]
    ZeroCount { kind: ResourceKind },

    #[error("resource {kind:?} has zero yield amount")]
    ZeroYield { kind: ResourceKind },

    #[error("resource {kind:?} has non-positive respawn time: {respawn_seconds}")]
    ZeroRespawn {
        kind: ResourceKind,
        respawn_seconds: f32,
    },

    #[error("resource {kind:?} count ({count}) does not match positions ({pos_count})")]
    CountPositionMismatch {
        kind: ResourceKind,
        count: u32,
        pos_count: usize,
    },

    #[error("resource {kind:?} position {index} ({pos:?}) is outside arena bounds")]
    ResourcePositionOutOfBounds {
        kind: ResourceKind,
        index: usize,
        pos: Vec3,
    },

    #[error("creature {kind:?} has non-positive wander radius: {wander_radius}")]
    ZeroWanderRadius {
        kind: CreatureKind,
        wander_radius: f32,
    },

    #[error("creature {kind:?} center ({pos:?}) is outside arena bounds")]
    CreaturePositionOutOfBounds { kind: CreatureKind, pos: Vec3 },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical world.ron payload — mirrors the shipped asset.
    const VALID_RON: &str = r#"(
        resources: [
            (
                id: (1),
                kind: Wood,
                count: 3,
                yield_amount: 2,
                respawn_seconds: 30.0,
                positions: [(8.0, 0.0, 8.0), (10.0, 0.0, 5.0), (6.0, 0.0, 10.0)],
                model_path: "assets/models/resources/wood.glb",
            ),
            (
                id: (2),
                kind: Crystal,
                count: 2,
                yield_amount: 1,
                respawn_seconds: 60.0,
                positions: [(-8.0, 0.0, -5.0), (-10.0, 0.0, -8.0)],
                model_path: "assets/models/resources/crystal.glb",
            ),
            (
                id: (3),
                kind: Berry,
                count: 4,
                yield_amount: 3,
                respawn_seconds: 20.0,
                positions: [(-3.0, 0.0, 8.0), (5.0, 0.0, -4.0), (-6.0, 0.0, -3.0), (0.0, 0.0, 12.0)],
                model_path: "assets/models/resources/berry.glb",
            ),
        ],
        creatures: [
            (
                id: (1),
                kind: Fluffball,
                center: (10.0, 0.0, 5.0),
                wander_radius: 8.0,
                food_kind: Berry,
                model_path: "assets/models/creatures/fluffball.glb",
            ),
            (
                id: (2),
                kind: Glimmerwing,
                center: (-5.0, 0.0, 15.0),
                wander_radius: 6.0,
                food_kind: Crystal,
                model_path: "assets/models/creatures/glimmerwing.glb",
            ),
        ],
    )"#;

    // -----------------------------------------------------------------------
    // RED tests — expect these to fail before the shipped asset is updated
    // -----------------------------------------------------------------------

    #[test]
    fn world_config_parses_shipped_asset() {
        let config = WorldConfig::from_str(VALID_RON).unwrap();
        assert_eq!(config.resources.len(), 3, "expected 3 resources");
        assert_eq!(config.creatures.len(), 2, "expected 2 creatures");
    }

    #[test]
    fn valid_config_resource_counts() {
        let config = WorldConfig::from_str(VALID_RON).unwrap();

        let wood = config
            .resources
            .iter()
            .find(|r| r.kind == ResourceKind::Wood)
            .unwrap();
        assert_eq!(wood.count, 3);
        assert_eq!(wood.positions.len(), 3);
        assert_eq!(wood.yield_amount, 2);
        assert_eq!(wood.model_path, "assets/models/resources/wood.glb");

        let crystal = config
            .resources
            .iter()
            .find(|r| r.kind == ResourceKind::Crystal)
            .unwrap();
        assert_eq!(crystal.count, 2);
        assert_eq!(crystal.model_path, "assets/models/resources/crystal.glb");

        let berry = config
            .resources
            .iter()
            .find(|r| r.kind == ResourceKind::Berry)
            .unwrap();
        assert_eq!(berry.count, 4);
        assert_eq!(berry.model_path, "assets/models/resources/berry.glb");
    }

    #[test]
    fn valid_config_creatures() {
        let config = WorldConfig::from_str(VALID_RON).unwrap();

        let fluff = config
            .creatures
            .iter()
            .find(|c| c.kind == CreatureKind::Fluffball)
            .unwrap();
        assert_eq!(fluff.food_kind, ResourceKind::Berry);
        assert_eq!(fluff.model_path, "assets/models/creatures/fluffball.glb");

        let glimmer = config
            .creatures
            .iter()
            .find(|c| c.kind == CreatureKind::Glimmerwing)
            .unwrap();
        assert_eq!(glimmer.food_kind, ResourceKind::Crystal);
        assert_eq!(
            glimmer.model_path,
            "assets/models/creatures/glimmerwing.glb"
        );
    }

    // -----------------------------------------------------------------------
    // Malformed fixture tests (expect typed errors)
    // -----------------------------------------------------------------------

    #[test]
    fn malformed_ron_returns_parse_error() {
        let err = WorldConfig::from_str("not valid ron {{{").unwrap_err();
        assert!(matches!(err, WorldConfigError::RonError(_)));
    }

    #[test]
    fn duplicate_resource_id_returns_specific_error() {
        let ron = r#"(
            resources: [
                (id: (1), kind: Wood, count: 1, yield_amount: 1, respawn_seconds: 10.0, positions: [(0.0, 0.0, 0.0)], model_path: "wood.glb"),
                (id: (1), kind: Crystal, count: 1, yield_amount: 1, respawn_seconds: 10.0, positions: [(1.0, 0.0, 0.0)], model_path: "crystal.glb"),
            ],
            creatures: [],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(
            matches!(err, WorldConfigError::DuplicateResourceId(_)),
            "expected DuplicateResourceId, got {err}"
        );
    }

    #[test]
    fn duplicate_creature_id_returns_specific_error() {
        let ron = r#"(
            resources: [],
            creatures: [
                (id: (1), kind: Fluffball, center: (0.0, 0.0, 0.0), wander_radius: 5.0, food_kind: Berry, model_path: "fluff.glb"),
                (id: (1), kind: Glimmerwing, center: (1.0, 0.0, 0.0), wander_radius: 5.0, food_kind: Crystal, model_path: "glim.glb"),
            ],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(
            matches!(err, WorldConfigError::DuplicateCreatureId(_)),
            "expected DuplicateCreatureId, got {err}"
        );
    }

    #[test]
    fn zero_resource_count_returns_error() {
        let ron = r#"(
            resources: [
                (id: (1), kind: Wood, count: 0, yield_amount: 1, respawn_seconds: 10.0, positions: [], model_path: "wood.glb"),
            ],
            creatures: [],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(matches!(err, WorldConfigError::ZeroCount { .. }));
    }

    #[test]
    fn zero_yield_returns_error() {
        let ron = r#"(
            resources: [
                (id: (1), kind: Wood, count: 1, yield_amount: 0, respawn_seconds: 10.0, positions: [(0.0, 0.0, 0.0)], model_path: "wood.glb"),
            ],
            creatures: [],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(matches!(err, WorldConfigError::ZeroYield { .. }));
    }

    #[test]
    fn zero_respawn_time_returns_error() {
        let ron = r#"(
            resources: [
                (id: (1), kind: Wood, count: 1, yield_amount: 1, respawn_seconds: 0.0, positions: [(0.0, 0.0, 0.0)], model_path: "wood.glb"),
            ],
            creatures: [],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(matches!(err, WorldConfigError::ZeroRespawn { .. }));
    }

    #[test]
    fn negative_respawn_time_returns_error() {
        let ron = r#"(
            resources: [
                (id: (1), kind: Wood, count: 1, yield_amount: 1, respawn_seconds: -5.0, positions: [(0.0, 0.0, 0.0)], model_path: "wood.glb"),
            ],
            creatures: [],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(matches!(err, WorldConfigError::ZeroRespawn { .. }));
    }

    #[test]
    fn count_position_mismatch_returns_error() {
        let ron = r#"(
            resources: [
                (id: (1), kind: Wood, count: 3, yield_amount: 1, respawn_seconds: 10.0, positions: [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)], model_path: "wood.glb"),
            ],
            creatures: [],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(
            matches!(err, WorldConfigError::CountPositionMismatch { .. }),
            "expected CountPositionMismatch, got {err}"
        );
    }

    #[test]
    fn resource_out_of_bounds_returns_error() {
        let ron = r#"(
            resources: [
                (id: (1), kind: Wood, count: 1, yield_amount: 1, respawn_seconds: 10.0, positions: [(30.0, 0.0, 0.0)], model_path: "wood.glb"),
            ],
            creatures: [],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(
            matches!(err, WorldConfigError::ResourcePositionOutOfBounds { .. }),
            "expected ResourcePositionOutOfBounds, got {err}"
        );
    }

    #[test]
    fn zero_wander_radius_returns_error() {
        let ron = r#"(
            resources: [],
            creatures: [
                (id: (1), kind: Fluffball, center: (0.0, 0.0, 0.0), wander_radius: 0.0, food_kind: Berry, model_path: "fluff.glb"),
            ],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(
            matches!(err, WorldConfigError::ZeroWanderRadius { .. }),
            "expected ZeroWanderRadius, got {err}"
        );
    }

    #[test]
    fn creature_out_of_bounds_returns_error() {
        let ron = r#"(
            resources: [],
            creatures: [
                (id: (1), kind: Fluffball, center: (50.0, 0.0, 0.0), wander_radius: 5.0, food_kind: Berry, model_path: "fluff.glb"),
            ],
        )"#;
        let err = WorldConfig::from_str(ron).unwrap_err();
        assert!(
            matches!(err, WorldConfigError::CreaturePositionOutOfBounds { .. }),
            "expected CreaturePositionOutOfBounds, got {err}"
        );
    }

    // -----------------------------------------------------------------------
    // Serde round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn world_config_serde_roundtrip() {
        let config = WorldConfig::from_str(VALID_RON).unwrap();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: WorldConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
