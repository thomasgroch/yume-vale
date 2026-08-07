use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::math::curve::{Curve, Ease, FunctionCurve, Interval};
use bevy::prelude::{Component, Reflect, Srgba, Vec3};
use game_core::decorations::DecorationKind;
use game_core::player_state::PlayerInput;
use game_core::resources::ResourceKind;
use game_core::world_config::CreatureKind;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub struct MovementInput {
    pub move_x: i8,
    pub move_z: i8,
    pub run: bool,
    pub jump: bool,
}

impl MapEntities for MovementInput {
    fn map_entities<M: EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

// ---------------------------------------------------------------------------
// Player components (existing)
// ---------------------------------------------------------------------------

/// The authoritative position of a player, replicated from server to clients.
/// Lightyear's interpolation system will smooth this component on clients.
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerPosition(pub Vec3);

impl Deref for PlayerPosition {
    type Target = Vec3;
    fn deref(&self) -> &Vec3 {
        &self.0
    }
}

impl DerefMut for PlayerPosition {
    fn deref_mut(&mut self) -> &mut Vec3 {
        &mut self.0
    }
}

/// Manual `Ease` impl because newtype wrappers do NOT get the blanket
/// `VectorSpace`-based implementation.
impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        FunctionCurve::new(Interval::EVERYWHERE, move |t| {
            PlayerPosition(Vec3::lerp(start.0, end.0, t))
        })
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ReplicatedPlayerInput(pub PlayerInput);

/// Server-assigned player color as a `PLAYER_PALETTE` index, replicated so
/// every client renders the same player in the same color.
#[derive(Component, Reflect, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct PlayerColor(pub u8);

/// Shared pastel palette indexed by `PlayerColor`; identical on server and clients.
pub const PLAYER_PALETTE: [Srgba; 8] = [
    Srgba::new(0.95, 0.55, 0.65, 1.0),
    Srgba::new(0.95, 0.65, 0.35, 1.0),
    Srgba::new(0.95, 0.85, 0.40, 1.0),
    Srgba::new(0.55, 0.85, 0.50, 1.0),
    Srgba::new(0.40, 0.80, 0.75, 1.0),
    Srgba::new(0.40, 0.65, 0.95, 1.0),
    Srgba::new(0.65, 0.55, 0.90, 1.0),
    Srgba::new(0.85, 0.50, 0.85, 1.0),
];

pub fn palette_color(index: u8) -> Srgba {
    PLAYER_PALETTE[index as usize % PLAYER_PALETTE.len()]
}

// ---------------------------------------------------------------------------
// Resource node state (replicated, with interpolation)
// ---------------------------------------------------------------------------

/// Replicated state of a resource node (tree, crystal, berry bush, etc.).
/// Position uses named f32 primitives for reliable binary serialization.
/// Intentionally no `Reflect` derive — game_core enum fields are Bevy-free.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResourceNodeState {
    pub resource_id: u64,
    pub kind: ResourceKind,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub depleted: bool,
    pub respawn_progress: f32,
}

impl Ease for ResourceNodeState {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        FunctionCurve::new(Interval::EVERYWHERE, move |t| ResourceNodeState {
            resource_id: end.resource_id,
            kind: end.kind,
            position_x: start.position_x + (end.position_x - start.position_x) * t,
            position_y: start.position_y + (end.position_y - start.position_y) * t,
            position_z: start.position_z + (end.position_z - start.position_z) * t,
            depleted: end.depleted,
            respawn_progress: start.respawn_progress
                + (end.respawn_progress - start.respawn_progress) * t,
        })
    }
}

// ---------------------------------------------------------------------------
// Creature state (replicated, with interpolation)
// ---------------------------------------------------------------------------

/// Replicated state of a wandering creature.
/// Uses named f32 primitives for position fields.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreatureState {
    pub creature_id: u64,
    pub kind: CreatureKind,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub target_x: f32,
    pub target_z: f32,
}

impl Ease for CreatureState {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        FunctionCurve::new(Interval::EVERYWHERE, move |t| CreatureState {
            creature_id: end.creature_id,
            kind: end.kind,
            position_x: start.position_x + (end.position_x - start.position_x) * t,
            position_y: start.position_y + (end.position_y - start.position_y) * t,
            position_z: start.position_z + (end.position_z - start.position_z) * t,
            target_x: end.target_x,
            target_z: end.target_z,
        })
    }
}

// ---------------------------------------------------------------------------
// Decoration state (replicated, static — no interpolation)
// ---------------------------------------------------------------------------

/// Replicated state of a static decoration prop.
/// These do not move, so no Ease impl is provided.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DecorationState {
    pub kind: DecorationKind,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub rotation: f32,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::curve::Ease;

    // --- PlayerPosition (existing) ---

    #[test]
    fn player_position_serde_roundtrip() {
        let orig = PlayerPosition(Vec3::new(1.0, 2.0, 3.0));
        let json = serde_json::to_string(&orig).unwrap();
        let back: PlayerPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn player_position_ease_lerp() {
        let start = PlayerPosition(Vec3::ZERO);
        let end = PlayerPosition(Vec3::new(10.0, 0.0, 0.0));
        let curve = PlayerPosition::interpolating_curve_unbounded(start, end);
        let mid = curve.sample(0.5);
        assert!((mid.unwrap().0.x - 5.0).abs() < 1e-5);
    }

    // --- PlayerColor ---

    #[test]
    fn player_color_serde_roundtrip() {
        let orig = PlayerColor(7);
        let json = serde_json::to_string(&orig).unwrap();
        let back: PlayerColor = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn palette_color_wraps_around() {
        assert_eq!(palette_color(0), palette_color(8));
        assert_ne!(palette_color(0), palette_color(1));
    }

    // --- ResourceNodeState ---

    #[test]
    fn resource_node_state_serde_roundtrip() {
        let orig = ResourceNodeState {
            resource_id: 1,
            kind: ResourceKind::Wood,
            position_x: 8.0,
            position_y: 0.0,
            position_z: 8.0,
            depleted: false,
            respawn_progress: 0.5,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ResourceNodeState = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn resource_node_ease_lerp_position() {
        let start = ResourceNodeState {
            resource_id: 1,
            kind: ResourceKind::Wood,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            depleted: false,
            respawn_progress: 0.0,
        };
        let end = ResourceNodeState {
            resource_id: 1,
            kind: ResourceKind::Wood,
            position_x: 10.0,
            position_y: 0.0,
            position_z: 0.0,
            depleted: false,
            respawn_progress: 1.0,
        };
        let curve = ResourceNodeState::interpolating_curve_unbounded(start, end);
        let mid = curve.sample(0.5).unwrap();
        assert!((mid.position_x - 5.0).abs() < 1e-5);
        assert!((mid.respawn_progress - 0.5).abs() < 1e-5);
    }

    #[test]
    fn resource_node_ease_snaps_depleted() {
        let start = ResourceNodeState {
            resource_id: 1,
            kind: ResourceKind::Wood,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            depleted: true,
            respawn_progress: 0.0,
        };
        let end = ResourceNodeState {
            resource_id: 1,
            kind: ResourceKind::Wood,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            depleted: false,
            respawn_progress: 1.0,
        };
        let curve = ResourceNodeState::interpolating_curve_unbounded(start, end);
        assert!(!curve.sample(0.0).unwrap().depleted);
        assert!(!curve.sample(1.0).unwrap().depleted);
    }

    // --- CreatureState ---

    #[test]
    fn creature_state_serde_roundtrip() {
        let orig = CreatureState {
            creature_id: 1,
            kind: CreatureKind::Fluffball,
            position_x: 10.0,
            position_y: 0.0,
            position_z: 5.0,
            target_x: 12.0,
            target_z: 3.0,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: CreatureState = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn creature_state_ease_lerp() {
        let start = CreatureState {
            creature_id: 1,
            kind: CreatureKind::Fluffball,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            target_x: 5.0,
            target_z: 5.0,
        };
        let end = CreatureState {
            creature_id: 1,
            kind: CreatureKind::Fluffball,
            position_x: 10.0,
            position_y: 0.0,
            position_z: 10.0,
            target_x: 15.0,
            target_z: 15.0,
        };
        let curve = CreatureState::interpolating_curve_unbounded(start, end);
        let mid = curve.sample(0.5).unwrap();
        assert!((mid.position_x - 5.0).abs() < 1e-5);
        assert!((mid.position_z - 5.0).abs() < 1e-5);
    }

    // --- DecorationState ---

    #[test]
    fn decoration_state_serde_roundtrip() {
        let orig = DecorationState {
            kind: DecorationKind::Tree,
            position_x: -15.0,
            position_y: 0.0,
            position_z: 20.0,
            rotation: 0.0,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: DecorationState = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn decoration_state_rock_kind_serde() {
        let orig = DecorationState {
            kind: DecorationKind::Rock(0.8),
            position_x: 10.0,
            position_y: 0.0,
            position_z: -10.0,
            rotation: 1.5,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: DecorationState = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }
}
