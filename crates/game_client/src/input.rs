use bevy::prelude::*;
use game_core::actions::ActionKind;
use game_core::math::Direction;
use game_protocol::ClientInput;
use game_protocol::channels::InputChannel;
use lightyear::prelude::MessageSender;

use crate::camera::CameraOrbit;

#[derive(Resource, Default)]
pub struct InputState {
    pub tick: u32,
}

/// Movement rotated by camera `yaw`: W = away from camera, D = screen-right.
/// `yaw = 0` maps W to world -Z.
pub fn read_keyboard_input(
    keys: &ButtonInput<KeyCode>,
    yaw: f32,
) -> (Direction, bool, Option<ActionKind>) {
    let (mut right, mut back) = (0.0f32, 0.0f32);
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        back -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        back += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        right -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        right += 1.0;
    }

    let movement = to_world_direction(right, back, yaw);
    let run = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    let action = if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::KeyF) {
        Some(ActionKind::Collect)
    } else {
        None
    };

    (movement, run, action)
}

fn to_world_direction(right: f32, back: f32, yaw: f32) -> Direction {
    let (sin, cos) = yaw.sin_cos();
    let forward = -back;
    let world_x = cos * right - sin * forward;
    let world_z = -sin * right - cos * forward;
    Direction::from_xz(world_x, world_z).unwrap_or(Direction::zero())
}

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

pub fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    orbit: Res<CameraOrbit>,
    flow: Res<crate::menu::AppFlow>,
    mut state: ResMut<InputState>,
    mut senders: Query<&mut MessageSender<ClientInput>>,
) {
    if *flow == crate::menu::AppFlow::Menu {
        return;
    }
    let (movement, run, _action) = read_keyboard_input(&keys, orbit.yaw);
    let (movement, run) = if movement.is_zero() {
        touches
            .iter()
            .next()
            .and_then(|t| swipe_direction(t.start_position(), t.position(), orbit.yaw))
            .unwrap_or((movement, run))
    } else {
        (movement, run)
    };
    state.tick = state.tick.wrapping_add(1);

    let input = ClientInput {
        tick: state.tick,
        move_x: (movement.0.x * 127.0).round() as i8,
        move_z: (movement.0.z * 127.0).round() as i8,
        run,
        jump: keys.pressed(KeyCode::Space),
    };

    if let Ok(mut sender) = senders.single_mut() {
        sender.send::<InputChannel>(input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press_keys(app: &mut App, keys: &[KeyCode]) {
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        for k in keys {
            input.press(*k);
        }
    }

    #[test]
    fn wasd_maps_to_direction() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();

        press_keys(&mut app, &[KeyCode::KeyW, KeyCode::KeyD]);
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (dir, run, action) = read_keyboard_input(keys, 0.0);

        assert!(dir.0.x > 0.0, "expected right movement");
        assert!(dir.0.z < 0.0, "expected forward movement");
        assert!(!run);
        assert!(action.is_none());
    }

    #[test]
    fn no_keys_pressed_means_no_movement() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();

        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (dir, run, action) = read_keyboard_input(keys, 0.0);

        assert!(dir.is_zero());
        assert!(!run);
        assert!(action.is_none());
    }

    #[test]
    fn shift_sets_run_flag() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();

        press_keys(&mut app, &[KeyCode::ShiftLeft, KeyCode::KeyW]);
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (_, run, _) = read_keyboard_input(keys, 0.0);

        assert!(run);
    }

    #[test]
    fn space_sets_collect_action() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();

        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.press(KeyCode::Space);
        }
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (_, _, action) = read_keyboard_input(keys, 0.0);

        assert_eq!(action, Some(ActionKind::Collect));
    }

    #[test]
    fn arrow_keys_map_to_direction() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();

        press_keys(&mut app, &[KeyCode::ArrowUp, KeyCode::ArrowLeft]);
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (dir, _, _) = read_keyboard_input(keys, 0.0);

        assert!(dir.0.x < 0.0, "expected left movement");
        assert!(dir.0.z < 0.0, "expected forward movement");
    }

    #[test]
    fn w_moves_away_from_camera_at_quarter_turn() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();

        press_keys(&mut app, &[KeyCode::KeyW]);
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (dir, _, _) = read_keyboard_input(keys, std::f32::consts::FRAC_PI_2);

        assert!(dir.0.x < -0.99, "expected -X movement, got {:?}", dir.0);
        assert!(dir.0.z.abs() < 1e-4);
    }

    #[test]
    fn d_moves_screen_right_at_quarter_turn() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();

        press_keys(&mut app, &[KeyCode::KeyD]);
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (dir, _, _) = read_keyboard_input(keys, std::f32::consts::FRAC_PI_2);

        assert!(dir.0.z < -0.99, "expected -Z movement, got {:?}", dir.0);
        assert!(dir.0.x.abs() < 1e-4);
    }

    #[test]
    fn w_matches_default_isometric_yaw() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();

        press_keys(&mut app, &[KeyCode::KeyW]);
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (dir, _, _) = read_keyboard_input(keys, std::f32::consts::FRAC_PI_4);

        let expected = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (dir.0.x + expected).abs() < 1e-2 && (dir.0.z + expected).abs() < 1e-2,
            "expected diagonal ({}, {}), got {:?}",
            -expected,
            -expected,
            dir.0
        );
    }

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
    fn input_state_tick_increments() {
        let mut state = InputState::default();
        assert_eq!(state.tick, 0);
        state.tick = state.tick.wrapping_add(1);
        assert_eq!(state.tick, 1);
        state.tick = state.tick.wrapping_add(1);
        assert_eq!(state.tick, 2);
    }

    #[test]
    fn input_state_tick_wraps() {
        let mut state = InputState { tick: u32::MAX };
        state.tick = state.tick.wrapping_add(1);
        assert_eq!(state.tick, 0);
    }
}
