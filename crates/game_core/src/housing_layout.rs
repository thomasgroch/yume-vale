use glam::Vec3;

/// Number of housing plots.
pub const HOUSING_PLOT_COUNT: usize = 16;

/// Half-size (radius) of each square plot area.
pub const PLOT_HALF_SIZE: f32 = 1.0;

/// Spacing between adjacent plot centers.
pub const PLOT_SPACING: f32 = 3.0;

/// Center X coordinate of the plot row.
pub const PLOT_ROW_X: f32 = -44.0;

/// A single housing plot slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotSlot {
    /// Index (0 … 15).
    pub index: usize,
    /// Center position (y = 0).
    pub center: Vec3,
    /// Half-size of the square plot area.
    pub half_size: f32,
}

/// Deterministic 16-slot plot layout outside the arena ring.
///
/// All 16 slots are arranged in a single row at x = -44 (well outside the
/// arena ring at radius 22 and the decoration scatter at |x| ≤ 40), spaced
/// 3 units apart along Z.
pub fn plot_layout() -> Vec<PlotSlot> {
    let start_z = -(HOUSING_PLOT_COUNT as f32 - 1.0) * PLOT_SPACING / 2.0;
    let mut slots = Vec::with_capacity(HOUSING_PLOT_COUNT);
    for i in 0..HOUSING_PLOT_COUNT {
        let z = start_z + i as f32 * PLOT_SPACING;
        slots.push(PlotSlot {
            index: i,
            center: Vec3::new(PLOT_ROW_X, 0.0, z),
            half_size: PLOT_HALF_SIZE,
        });
    }
    slots
}

/// Returns a stable slot index (0 … 15) for a given `PlayerId`.
///
/// Guaranteed: same `PlayerId` → same slot on every call.
pub fn slot_for_player(player_id: impl Into<u64>) -> usize {
    (player_id.into() as usize) % HOUSING_PLOT_COUNT
}

/// Returns the center position for a given slot index.
pub fn slot_center(slot_index: usize) -> Vec3 {
    let layout = plot_layout();
    layout[slot_index.min(HOUSING_PLOT_COUNT - 1)].center
}

/// Check whether a world-space point falls within a plot slot's square bounds.
pub fn point_in_plot(slot_index: usize, x: f32, z: f32) -> bool {
    if slot_index >= HOUSING_PLOT_COUNT {
        return false;
    }
    let layout = plot_layout();
    let slot = &layout[slot_index];
    let dx = (x - slot.center.x).abs();
    let dz = (z - slot.center.z).abs();
    dx <= slot.half_size && dz <= slot.half_size
}

/// Minimum squared distance from any plot slot to a given point.
/// Used for keep-out checks with decoration and arena props.
pub fn min_distance_sq_to_any_plot(x: f32, z: f32) -> f32 {
    let layout = plot_layout();
    layout
        .iter()
        .map(|s| {
            let dx = x - s.center.x;
            let dz = z - s.center.z;
            dx * dx + dz * dz
        })
        .fold(f32::MAX, f32::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_has_16_slots() {
        let slots = plot_layout();
        assert_eq!(slots.len(), HOUSING_PLOT_COUNT);
    }

    #[test]
    fn layout_is_deterministic() {
        assert_eq!(plot_layout(), plot_layout());
    }

    #[test]
    fn slots_are_non_overlapping() {
        let slots = plot_layout();
        for i in 0..slots.len() {
            for j in (i + 1)..slots.len() {
                let a = &slots[i];
                let b = &slots[j];
                let dx = (a.center.x - b.center.x).abs();
                let dz = (a.center.z - b.center.z).abs();
                // Slots may share an edge (dx ≈ 0, dz ≈ 3) but not overlap.
                assert!(
                    dx >= a.half_size + b.half_size - 0.01
                        || dz >= a.half_size + b.half_size - 0.01,
                    "slots {i} and {j} overlap: centers ({},{}) vs ({},{})",
                    a.center.x,
                    a.center.z,
                    b.center.x,
                    b.center.z
                );
            }
        }
    }

    #[test]
    fn slots_outside_arena_ring() {
        let arena_radius = 26.0; // decoration keep-out radius
        for slot in plot_layout() {
            let dist = (slot.center.x * slot.center.x + slot.center.z * slot.center.z).sqrt();
            assert!(
                dist >= arena_radius,
                "slot {} at dist {dist} is inside arena keep-out ring ({arena_radius})",
                slot.index
            );
        }
    }

    #[test]
    fn slots_outside_decoration_scatter() {
        // Decorations are bounded by |x| ≤ 40, |z| ≤ 40
        let max_scatter = 40.0;
        for slot in plot_layout() {
            // Plot extends half_size beyond center, so check the nearest edge
            let near_x = slot.center.x.abs() - slot.half_size;
            // All slots are at negative X, so |x| = |PLOT_ROW_X|
            assert!(
                near_x > max_scatter,
                "slot {} at x={} (edge at {near_x}) is inside decoration scatter zone",
                slot.index,
                slot.center.x
            );
        }
    }

    #[test]
    fn slots_outside_spawn_area() {
        for slot in plot_layout() {
            let near_x = slot.center.x.abs() - slot.half_size;
            let near_z = slot.center.z.abs() - slot.half_size;
            assert!(
                near_x >= 3.0 || near_z >= 3.0,
                "slot {} at ({},{}) overlaps spawn area (±3)",
                slot.index,
                slot.center.x,
                slot.center.z
            );
        }
    }

    #[test]
    fn slot_for_player_is_stable() {
        let id: u64 = 42;
        assert_eq!(slot_for_player(id), slot_for_player(id));
    }

    #[test]
    fn slot_for_player_produces_valid_range() {
        for id in 0..1000_u64 {
            let slot = slot_for_player(id);
            assert!(
                slot < HOUSING_PLOT_COUNT,
                "id {id} → slot {slot} out of range"
            );
        }
    }

    #[test]
    fn point_in_plot_accepts_center() {
        let layout = plot_layout();
        for slot in &layout {
            assert!(point_in_plot(slot.index, slot.center.x, slot.center.z));
        }
    }

    #[test]
    fn point_in_plot_rejects_outside() {
        assert!(!point_in_plot(0, -100.0, 0.0));
    }

    #[test]
    fn point_in_plot_rejects_invalid_index() {
        assert!(!point_in_plot(99, 0.0, 0.0));
    }

    #[test]
    fn min_distance_sq_to_any_plot_zero_at_center() {
        let layout = plot_layout();
        for slot in &layout {
            assert!(
                min_distance_sq_to_any_plot(slot.center.x, slot.center.z) < 0.001,
                "distance at slot {} center should be ~0",
                slot.index
            );
        }
    }
}
