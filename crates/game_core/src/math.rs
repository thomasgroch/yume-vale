use crate::constants::GROUND_Y;
use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position(pub Vec3);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Velocity(pub Vec3);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Direction(pub Vec3);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rotation(pub Quat);

impl Position {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }

    pub fn distance_to(self, other: Self) -> f32 {
        self.0.distance(other.0)
    }

    pub fn distance_squared_to(self, other: Self) -> f32 {
        self.0.distance_squared(other.0)
    }
}

impl From<Vec3> for Position {
    fn from(v: Vec3) -> Self {
        Self(v)
    }
}

impl From<Position> for Vec3 {
    fn from(p: Position) -> Self {
        p.0
    }
}

impl Velocity {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

impl From<Vec3> for Velocity {
    fn from(v: Vec3) -> Self {
        Self(v)
    }
}

impl From<Velocity> for Vec3 {
    fn from(v: Velocity) -> Self {
        v.0
    }
}

impl Direction {
    pub fn from_vec3(v: Vec3) -> Option<Self> {
        if v == Vec3::ZERO {
            None
        } else {
            Some(Self(v.normalize()))
        }
    }

    pub fn from_xz(x: f32, z: f32) -> Option<Self> {
        let v = Vec3::new(x, 0.0, z);
        if v == Vec3::ZERO {
            None
        } else {
            Some(Self(v.normalize()))
        }
    }

    pub const fn zero() -> Self {
        Self(Vec3::ZERO)
    }

    pub fn is_zero(self) -> bool {
        self.0 == Vec3::ZERO
    }
}

impl From<Vec3> for Direction {
    fn from(v: Vec3) -> Self {
        Self::from_vec3(v).unwrap_or(Self(Vec3::ZERO))
    }
}

impl Rotation {
    pub const fn identity() -> Self {
        Self(Quat::IDENTITY)
    }
}

impl From<Quat> for Rotation {
    fn from(q: Quat) -> Self {
        Self(q)
    }
}

impl From<Rotation> for Quat {
    fn from(r: Rotation) -> Self {
        r.0
    }
}

pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    value.clamp(min, max)
}

pub fn distance2(a: Vec2, b: Vec2) -> f32 {
    a.distance(b)
}

pub fn distance3(a: Vec3, b: Vec3) -> f32 {
    a.distance(b)
}

pub fn move_towards(from: Vec3, to: Vec3, max_distance: f32) -> Vec3 {
    let delta = to - from;
    let dist = delta.length();
    if dist <= max_distance || dist < 1e-6 {
        to
    } else {
        from + (delta / dist) * max_distance
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub fn snap_to_ground(pos: Vec3, ground_y: Option<f32>) -> Vec3 {
    let y = ground_y.unwrap_or(GROUND_Y);
    Vec3::new(pos.x, y, pos.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn position_new_and_access() {
        let p = Position::new(1.0, 2.0, 3.0);
        assert_eq!(p.0.x, 1.0);
        assert_eq!(p.0.y, 2.0);
        assert_eq!(p.0.z, 3.0);
    }

    #[test]
    fn position_distance() {
        let a = Position::new(0.0, 0.0, 0.0);
        let b = Position::new(3.0, 0.0, 4.0);
        assert!((a.distance_to(b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn distance2_computes_xz_distance() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(3.0, 4.0);
        assert!((distance2(a, b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn distance3_computes_3d_distance() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 2.0, 2.0);
        assert!((distance3(a, b) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn move_towards_snaps_when_close() {
        let from = Vec3::new(0.0, 0.0, 0.0);
        let to = Vec3::new(1.0, 0.0, 0.0);
        let result = move_towards(from, to, 2.0);
        assert_eq!(result, to);
    }

    #[test]
    fn move_towards_moves_partial() {
        let from = Vec3::new(0.0, 0.0, 0.0);
        let to = Vec3::new(10.0, 0.0, 0.0);
        let result = move_towards(from, to, 3.0);
        assert!((result.distance(Vec3::new(3.0, 0.0, 0.0))).abs() < 1e-6);
    }

    #[test]
    fn move_towards_zero_distance() {
        let from = Vec3::new(5.0, 5.0, 5.0);
        let result = move_towards(from, from, 10.0);
        assert_eq!(result, from);
    }

    #[test]
    fn clamp_works() {
        assert_eq!(clamp(5.0, 0.0, 10.0), 5.0);
        assert_eq!(clamp(-1.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp(15.0, 0.0, 10.0), 10.0);
    }

    #[test]
    fn lerp_endpoints() {
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < 1e-6);
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_clamps_t() {
        assert!((lerp(0.0, 10.0, -0.5) - 0.0).abs() < 1e-6);
        assert!((lerp(0.0, 10.0, 1.5) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn snap_to_ground_default() {
        let pos = Vec3::new(1.0, 5.0, 3.0);
        let snapped = snap_to_ground(pos, None);
        assert_eq!(snapped, Vec3::new(1.0, 0.0, 3.0));
    }

    #[test]
    fn snap_to_ground_custom() {
        let pos = Vec3::new(1.0, 5.0, 3.0);
        let snapped = snap_to_ground(pos, Some(0.5));
        assert_eq!(snapped, Vec3::new(1.0, 0.5, 3.0));
    }

    #[test]
    fn direction_from_vec3_normalizes() {
        let dir = Direction::from_vec3(Vec3::new(0.0, 0.0, 5.0)).unwrap();
        assert!((dir.0.z - 1.0).abs() < 1e-6);
        assert!(dir.0.length() - 1.0 < 1e-6);
    }

    #[test]
    fn direction_from_zero_returns_none() {
        assert!(Direction::from_vec3(Vec3::ZERO).is_none());
    }

    #[test]
    fn direction_from_xz() {
        let dir = Direction::from_xz(3.0, 4.0).unwrap();
        assert!((dir.0.length() - 1.0).abs() < 1e-6);
        assert_eq!(dir.0.y, 0.0);
    }

    #[test]
    fn velocity_new() {
        let v = Velocity::new(1.0, 2.0, 3.0);
        assert_eq!(v.0.x, 1.0);
    }

    #[test]
    fn rotation_identity() {
        let r = Rotation::identity();
        assert_eq!(r.0, Quat::IDENTITY);
    }

    #[test]
    fn position_from_vec3() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let p: Position = v.into();
        assert_eq!(p.0, v);
    }

    #[test]
    fn position_into_vec3() {
        let p = Position::new(1.0, 2.0, 3.0);
        let v: Vec3 = p.into();
        assert_eq!(v, p.0);
    }
}
