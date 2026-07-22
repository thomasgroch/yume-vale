use bevy::math::curve::{Curve, Ease, FunctionCurve, Interval};
use bevy::prelude::{Component, Srgba, Vec3};
use game_core::player_state::PlayerInput;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

/// The authoritative position of a player, replicated from server to clients.
/// Lightyear's interpolation system will smooth this component on clients.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
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
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
