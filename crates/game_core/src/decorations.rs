use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Kind of a scattered decoration prop (outside the arena ring).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DecorationKind {
    /// Tree: cylinder trunk + sphere canopy. Collider = trunk.
    Tree,
    /// Boulder: squashed sphere, half-buried. Field = uniform scale.
    Rock(f32),
    /// Small glowing flower. Visual only, no collider.
    Flower,
}

/// A single scattered decoration, shared so the server can spawn matching
/// physics colliders for the client's visuals.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecorationProp {
    pub kind: DecorationKind,
    pub position: Vec3,
}

/// Deterministic pseudo-random scatter (same algorithm both sides run — no
/// rand dependency). 40 candidates in an 80×80 square, skipping the spawn
/// area (±3) and the arena keep-out ring (radius 26).
pub fn decoration_layout() -> Vec<DecorationProp> {
    let mut seed: u32 = 0x9E37_79B9;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / 16_777_216.0
    };

    let mut props = Vec::new();
    for _ in 0..40 {
        let x = (next() - 0.5) * 80.0;
        let z = (next() - 0.5) * 80.0;
        if x.abs() < 3.0 && z.abs() < 3.0 {
            continue;
        }
        if (x * x + z * z).sqrt() < 26.0 {
            continue;
        }

        let position = Vec3::new(x, 0.0, z);
        match (next() * 10.0) as u32 {
            0..=4 => props.push(DecorationProp {
                kind: DecorationKind::Tree,
                position,
            }),
            5..=7 => props.push(DecorationProp {
                kind: DecorationKind::Rock(0.6 + next() * 0.8),
                position,
            }),
            _ => props.push(DecorationProp {
                kind: DecorationKind::Flower,
                position,
            }),
        }
    }
    props
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_deterministic() {
        assert_eq!(decoration_layout(), decoration_layout());
    }

    #[test]
    fn layout_respects_keep_out_zones() {
        for prop in decoration_layout() {
            let p = prop.position;
            assert!(
                p.x.abs() >= 3.0 || p.z.abs() >= 3.0,
                "prop {:?} inside spawn area",
                prop
            );
            assert!(
                (p.x * p.x + p.z * p.z).sqrt() >= 26.0,
                "prop {:?} inside arena ring",
                prop
            );
        }
    }

    #[test]
    fn layout_has_all_kinds() {
        let props = decoration_layout();
        assert!(props.iter().any(|p| p.kind == DecorationKind::Tree));
        assert!(
            props
                .iter()
                .any(|p| matches!(p.kind, DecorationKind::Rock(_)))
        );
        assert!(props.iter().any(|p| p.kind == DecorationKind::Flower));
    }
}
