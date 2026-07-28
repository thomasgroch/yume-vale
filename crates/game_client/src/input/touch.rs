use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_core::math::Direction;

const SWIPE_DEAD_ZONE: f32 = 24.0;
const SWIPE_MAX_RADIUS: f32 = 120.0;
const SWIPE_RUN_THRESHOLD: f32 = 0.85;

/// Virtual joystick: drag from `start` to `current` gives a camera-relative
/// movement direction; dragging near the max radius sets run.
pub fn swipe_direction(start: Vec2, current: Vec2, yaw: f32) -> Option<(Direction, bool)> {
    let delta = current - start;
    let dist = delta.length();
    if dist < SWIPE_DEAD_ZONE {
        return None;
    }
    let magnitude = ((dist - SWIPE_DEAD_ZONE) / (SWIPE_MAX_RADIUS - SWIPE_DEAD_ZONE)).min(1.0);
    let dir = to_world_direction(delta.x, delta.y, yaw);
    if dir.is_zero() {
        return None;
    }
    Some((dir, magnitude >= SWIPE_RUN_THRESHOLD))
}

fn to_world_direction(right: f32, back: f32, yaw: f32) -> Direction {
    let (sin, cos) = yaw.sin_cos();
    let forward = -back;
    let world_x = cos * right - sin * forward;
    let world_z = -sin * right - cos * forward;
    Direction::from_xz(world_x, world_z).unwrap_or(Direction::zero())
}

/// Touch-side input sources bundled to keep `gather_input`'s arity low.
#[derive(bevy::ecs::system::SystemParam)]
pub struct TouchParams<'w, 's> {
    pub touches: Res<'w, Touches>,
    pub touch_jump: Res<'w, super::super::touch::TouchJump>,
    pub window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swipe_below_dead_zone_is_none() {
        assert!(swipe_direction(Vec2::ZERO, Vec2::new(10.0, 5.0), 0.0).is_none());
    }

    #[test]
    fn swipe_up_moves_forward_walk() {
        let (dir, run) = swipe_direction(Vec2::ZERO, Vec2::new(0.0, -60.0), 0.0).unwrap();
        assert!(dir.0.z < -0.99, "expected forward, got {:?}", dir.0);
        assert!(dir.0.x.abs() < 1e-4);
        assert!(!run);
    }

    #[test]
    fn swipe_far_diagonal_moves_and_runs() {
        let (dir, run) = swipe_direction(Vec2::ZERO, Vec2::new(100.0, -100.0), 0.0).unwrap();
        assert!(
            dir.0.x > 0.5 && dir.0.z < -0.5,
            "expected diagonal, got {:?}",
            dir.0
        );
        assert!(run, "full-radius drag should run");
    }

    #[test]
    fn swipe_respects_camera_yaw() {
        let (dir, _) = swipe_direction(
            Vec2::ZERO,
            Vec2::new(0.0, -60.0),
            std::f32::consts::FRAC_PI_2,
        )
        .unwrap();
        assert!(dir.0.x < -0.99, "expected -X at yaw=90°, got {:?}", dir.0);
    }

    #[test]
    fn characterization_constants() {
        assert_eq!(super::SWIPE_DEAD_ZONE, 24.0);
        assert_eq!(super::SWIPE_MAX_RADIUS, 120.0);
        assert_eq!(super::SWIPE_RUN_THRESHOLD, 0.85);
    }

    #[test]
    fn characterization_touch_params_system_param_exists() {
        fn _assert(_: TouchParams) {}
        let _ = _assert;
    }
}
