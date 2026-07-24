use glam::Vec3;

/// Radius of the arena ring (wall positions).
pub const ARENA_RADIUS: f32 = 22.0;

/// Number of wall/portal slots in the ring.
pub const WALL_COUNT: usize = 20;

/// Which ring slot holds the portal (instead of a wall).
pub const PORTAL_SLOT: usize = 0;

/// Identifies the visual model for an arena prop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArenaModel {
    Portal,
    Wall,
    Pillar,
    CrystalBig,
    CrystalSmall,
    Rock,
}

impl ArenaModel {
    /// Relative asset path for the model's GLB file.
    pub const fn asset_path(self) -> &'static str {
        match self {
            ArenaModel::Portal => "models/arena/portal.glb",
            ArenaModel::Wall => "models/arena/wall.glb",
            ArenaModel::Pillar => "models/arena/pillar.glb",
            ArenaModel::CrystalBig => "models/arena/crystal_big.glb",
            ArenaModel::CrystalSmall => "models/arena/crystal_small.glb",
            ArenaModel::Rock => "models/arena/rock.glb",
        }
    }
}

/// Shape of a single physics collider attached to a prop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArenaColliderShape {
    /// Axis-aligned box relative to the collider's local frame.
    Cuboid { half_extents: Vec3 },
    /// Cylinder oriented along the local Y axis.
    Cylinder { radius: f32, half_height: f32 },
}

/// A single collider within an `ArenaProp`.
///
/// The `offset` is a translation in the prop's local frame (rotated by the
/// prop's yaw when placed in world space). Offsets place the collider's base
/// at ground level (y=0), matching the client's visual `y_offset` lift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArenaCollider {
    pub shape: ArenaColliderShape,
    pub offset: Vec3,
}

/// A decorative prop placed in the arena, carrying its collider definitions.
///
/// Each prop spawns one entity per entry in `colliders`; the collider's
/// world-space transform is `(prop.translation + yaw_rotated(offset),
/// Quat::from_rotation_y(yaw))`.
#[derive(Clone, Copy, Debug)]
pub struct ArenaProp {
    pub model: ArenaModel,
    pub translation: Vec3,
    pub yaw: f32,
    /// Visual-only scale normalizing the Meshy GLB (~2m, centered origin) to
    /// its intended world size. Ignored by the server (colliders are absolute).
    pub scale: f32,
    pub colliders: &'static [ArenaCollider],
}

/// Builds the deterministic arena layout — 36 props arranged in a ring with
/// interior decorations.
///
/// Must be called from runtime (trig-based placement can't use `const fn`).
/// Both server and client call this to stay in sync.
pub fn arena_layout() -> Vec<ArenaProp> {
    use std::f32::consts::TAU;

    let mut props = Vec::with_capacity(36);

    // --- Shared collider slices (static references, allocated once) ---

    const WALL_COLLIDERS: &[ArenaCollider] = &[ArenaCollider {
        shape: ArenaColliderShape::Cuboid {
            half_extents: Vec3::new(3.4, 1.47, 1.2),
        },
        offset: Vec3::new(0.0, 1.47, 0.0),
    }];

    // Portal posts measured from portal.glb at scale 2.5: each post spans
    // |x| 0.94..2.5 (opening ~1.9m stays passable), z -0.83..0.48. The three
    // stacked steps under the arch are measured per layer (tops at 0.28 /
    // 0.60 / 0.91) so the fox can jump up onto the platform.
    const PORTAL_COLLIDERS: &[ArenaCollider] = &[
        ArenaCollider {
            shape: ArenaColliderShape::Cuboid {
                half_extents: Vec3::new(0.78, 2.48, 0.65),
            },
            offset: Vec3::new(1.72, 2.48, -0.18),
        },
        ArenaCollider {
            shape: ArenaColliderShape::Cuboid {
                half_extents: Vec3::new(0.78, 2.48, 0.65),
            },
            offset: Vec3::new(-1.72, 2.48, -0.18),
        },
        ArenaCollider {
            shape: ArenaColliderShape::Cuboid {
                half_extents: Vec3::new(0.94, 0.14, 1.69),
            },
            offset: Vec3::new(0.0, 0.14, 0.0),
        },
        ArenaCollider {
            shape: ArenaColliderShape::Cuboid {
                half_extents: Vec3::new(0.94, 0.16, 1.36),
            },
            offset: Vec3::new(0.0, 0.44, 0.05),
        },
        ArenaCollider {
            shape: ArenaColliderShape::Cuboid {
                half_extents: Vec3::new(0.94, 0.155, 0.58),
            },
            offset: Vec3::new(0.0, 0.755, -0.15),
        },
    ];

    const PILLAR_COLLIDERS: &[ArenaCollider] = &[ArenaCollider {
        shape: ArenaColliderShape::Cylinder {
            radius: 1.0,
            half_height: 2.0,
        },
        offset: Vec3::new(0.0, 2.0, 0.0),
    }];

    const CRYSTAL_BIG_COLLIDERS: &[ArenaCollider] = &[ArenaCollider {
        shape: ArenaColliderShape::Cylinder {
            radius: 1.4,
            half_height: 1.6,
        },
        offset: Vec3::new(0.0, 1.6, 0.0),
    }];

    const CRYSTAL_SMALL_COLLIDERS: &[ArenaCollider] = &[ArenaCollider {
        shape: ArenaColliderShape::Cylinder {
            radius: 0.4,
            half_height: 0.44,
        },
        offset: Vec3::new(0.0, 0.44, 0.0),
    }];

    const ROCK_COLLIDERS: &[ArenaCollider] = &[ArenaCollider {
        shape: ArenaColliderShape::Cuboid {
            half_extents: Vec3::new(0.8, 0.73, 0.7),
        },
        offset: Vec3::new(0.0, 0.73, 0.0),
    }];

    // --- Ring: walls + one portal ---

    for i in 0..WALL_COUNT {
        let a = i as f32 * TAU / WALL_COUNT as f32;
        let pos = Vec3::new(ARENA_RADIUS * a.sin(), 0.0, ARENA_RADIUS * a.cos());

        if i == PORTAL_SLOT {
            props.push(ArenaProp {
                model: ArenaModel::Portal,
                translation: pos,
                yaw: 0.0,
                scale: 2.5,
                colliders: PORTAL_COLLIDERS,
            });
        } else {
            props.push(ArenaProp {
                model: ArenaModel::Wall,
                translation: pos,
                yaw: a,
                scale: 3.5,
                colliders: WALL_COLLIDERS,
            });
        }
    }

    // --- Pillars at radius 14, 45° intervals ---

    for angle_deg in [45_f32, 135_f32, 225_f32, 315_f32] {
        let a = angle_deg.to_radians();
        props.push(ArenaProp {
            model: ArenaModel::Pillar,
            translation: Vec3::new(14.0 * a.sin(), 0.0, 14.0 * a.cos()),
            yaw: a,
            scale: 2.0,
            colliders: PILLAR_COLLIDERS,
        });
    }

    // --- CrystalBig at (0, 0, -6) ---
    // Players spawn at (0, 0, 0) — still > 4 units away, so position is fine.
    props.push(ArenaProp {
        model: ArenaModel::CrystalBig,
        translation: Vec3::new(0.0, 0.0, -6.0),
        yaw: 0.0,
        scale: 1.6,
        colliders: CRYSTAL_BIG_COLLIDERS,
    });

    // --- CrystalSmall at radius 8, 60° intervals starting at 30° ---

    for angle_deg in [30_f32, 90_f32, 150_f32, 210_f32, 270_f32, 330_f32] {
        let a = angle_deg.to_radians();
        props.push(ArenaProp {
            model: ArenaModel::CrystalSmall,
            translation: Vec3::new(8.0 * a.sin(), 0.0, 8.0 * a.cos()),
            yaw: a,
            scale: 0.55,
            colliders: CRYSTAL_SMALL_COLLIDERS,
        });
    }

    // --- Rocks at radius 17, uneven spacing ---

    for angle_deg in [15_f32, 80_f32, 160_f32, 250_f32, 300_f32] {
        let a = angle_deg.to_radians();
        props.push(ArenaProp {
            model: ArenaModel::Rock,
            translation: Vec3::new(17.0 * a.sin(), 0.0, 17.0 * a.cos()),
            yaw: a,
            scale: 0.8,
            colliders: ROCK_COLLIDERS,
        });
    }

    props
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collider_bases_never_buried() {
        for prop in arena_layout() {
            for c in prop.colliders {
                let base = match c.shape {
                    ArenaColliderShape::Cuboid { half_extents } => c.offset.y - half_extents.y,
                    ArenaColliderShape::Cylinder { half_height, .. } => c.offset.y - half_height,
                };
                assert!(
                    base >= -1e-5,
                    "{:?} collider base at {}, must not be buried below 0",
                    prop.model,
                    base
                );
            }
        }
    }

    #[test]
    fn layout_has_36_props() {
        assert_eq!(arena_layout().len(), 36);
    }

    #[test]
    fn layout_has_one_portal() {
        let layout = arena_layout();
        let portals: Vec<_> = layout
            .iter()
            .filter(|p| p.model == ArenaModel::Portal)
            .collect();
        assert_eq!(portals.len(), 1);
        assert_eq!(portals[0].colliders.len(), 5);
    }

    #[test]
    fn layout_has_19_walls() {
        let layout = arena_layout();
        let walls: Vec<_> = layout
            .iter()
            .filter(|p| p.model == ArenaModel::Wall)
            .collect();
        assert_eq!(walls.len(), 19);
    }

    #[test]
    fn layout_has_four_pillars() {
        let layout = arena_layout();
        let pillars: Vec<_> = layout
            .iter()
            .filter(|p| p.model == ArenaModel::Pillar)
            .collect();
        assert_eq!(pillars.len(), 4);
    }

    #[test]
    fn layout_has_one_big_crystal() {
        let layout = arena_layout();
        let big: Vec<_> = layout
            .iter()
            .filter(|p| p.model == ArenaModel::CrystalBig)
            .collect();
        assert_eq!(big.len(), 1);
    }

    #[test]
    fn layout_has_six_small_crystals() {
        let layout = arena_layout();
        let small: Vec<_> = layout
            .iter()
            .filter(|p| p.model == ArenaModel::CrystalSmall)
            .collect();
        assert_eq!(small.len(), 6);
    }

    #[test]
    fn layout_has_five_rocks() {
        let layout = arena_layout();
        let rocks: Vec<_> = layout
            .iter()
            .filter(|p| p.model == ArenaModel::Rock)
            .collect();
        assert_eq!(rocks.len(), 5);
    }

    #[test]
    fn portal_is_at_slot_zero() {
        let layout = arena_layout();
        assert_eq!(layout[PORTAL_SLOT].model, ArenaModel::Portal);
        // Position = (R * sin(0), 0, R * cos(0)) = (0, 0, R)
        let expected = Vec3::new(0.0, 0.0, ARENA_RADIUS);
        assert!(
            (layout[PORTAL_SLOT].translation - expected).length() < 0.001,
            "portal at ({}, {}, {}) expected ({expected})",
            layout[PORTAL_SLOT].translation.x,
            layout[PORTAL_SLOT].translation.y,
            layout[PORTAL_SLOT].translation.z,
        );
    }

    #[test]
    fn asset_paths_are_correct() {
        assert_eq!(ArenaModel::Portal.asset_path(), "models/arena/portal.glb");
        assert_eq!(ArenaModel::Wall.asset_path(), "models/arena/wall.glb");
        assert_eq!(ArenaModel::Pillar.asset_path(), "models/arena/pillar.glb");
        assert_eq!(
            ArenaModel::CrystalBig.asset_path(),
            "models/arena/crystal_big.glb"
        );
        assert_eq!(
            ArenaModel::CrystalSmall.asset_path(),
            "models/arena/crystal_small.glb"
        );
        assert_eq!(ArenaModel::Rock.asset_path(), "models/arena/rock.glb");
    }

    #[test]
    fn all_props_have_at_least_one_collider() {
        let layout = arena_layout();
        for (i, prop) in layout.iter().enumerate() {
            assert!(
                !prop.colliders.is_empty(),
                "prop {i} ({:?}) has zero colliders",
                prop.model,
            );
        }
    }

    #[test]
    fn layout_is_deterministic() {
        let a = arena_layout();
        let b = arena_layout();
        for (i, (pa, pb)) in a.iter().zip(b.iter()).enumerate() {
            assert_eq!(pa.model, pb.model, "prop {i} model mismatch");
            assert!(
                (pa.translation - pb.translation).length() < 0.0001,
                "prop {i} translation mismatch"
            );
        }
        assert_eq!(a.len(), b.len());
    }
}
