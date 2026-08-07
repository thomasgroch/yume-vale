use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use player::LocalPlayer;

use crate::graphics::LightToggle;
use crate::touch::camera_touch_id;

const CAMERA_RADIUS: f32 = 9.9;
const CAMERA_HEIGHT: f32 = 8.0;
const CAMERA_SMOOTHING: f32 = 8.0;
const ROTATION_SMOOTHING: f32 = 10.0;
const MIN_ZOOM_RADIUS: f32 = 4.0;
const MAX_ZOOM_RADIUS: f32 = 20.0;
const ZOOM_SPEED: f32 = 1.5;
const TOUCH_ROTATE_SPEED: f32 = 0.005;
const TOUCH_ZOOM_SPEED: f32 = 0.02;

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct CameraOrbit {
    pub yaw: f32,
    pub target_yaw: f32,
    pub radius: f32,
    pub target_radius: f32,
}

impl Default for CameraOrbit {
    fn default() -> Self {
        Self {
            yaw: std::f32::consts::FRAC_PI_4,
            target_yaw: std::f32::consts::FRAC_PI_4,
            radius: CAMERA_RADIUS,
            target_radius: CAMERA_RADIUS,
        }
    }
}

pub fn spawn_camera(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::srgb(0.6, 0.8, 0.95)));

    commands.spawn((
        Camera3d::default(),
        Camera::default(),
        Transform::from_translation(Vec3::new(7.0, 8.0, 7.0)).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::FULL_DAYLIGHT,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
    ));

    commands.spawn((
        PointLight {
            intensity: 1_500_000.0,
            range: 120.0,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        LightToggle,
        Visibility::Visible,
        Transform::from_translation(Vec3::new(2.0, 6.0, 2.0)),
    ));

    commands.spawn((
        PointLight {
            intensity: 800_000.0,
            range: 80.0,
            color: Color::srgb(0.8, 0.9, 1.0),
            ..default()
        },
        LightToggle,
        Visibility::Visible,
        Transform::from_translation(Vec3::new(-3.0, 4.0, -3.0)),
    ));
}

pub fn rotate_camera_input(keys: Res<ButtonInput<KeyCode>>, mut orbit: ResMut<CameraOrbit>) {
    if keys.just_pressed(KeyCode::KeyQ) {
        orbit.target_yaw += std::f32::consts::FRAC_PI_4;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        orbit.target_yaw -= std::f32::consts::FRAC_PI_4;
    }
}

pub fn zoom_camera_input(mut wheel: MessageReader<MouseWheel>, mut orbit: ResMut<CameraOrbit>) {
    for event in wheel.read() {
        let lines = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y * 0.05,
        };
        orbit.target_radius =
            (orbit.target_radius - lines * ZOOM_SPEED).clamp(MIN_ZOOM_RADIUS, MAX_ZOOM_RADIUS);
    }
}

/// Touch that started in the right half of the screen: drag horizontally to
/// orbit the camera, vertically to zoom (up = in). Independent of the
/// movement touch; works with a single finger.
pub fn touch_camera_input(
    touches: Res<Touches>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut orbit: ResMut<CameraOrbit>,
    mut prev: Local<Option<(u64, Vec2)>>,
) {
    let Ok(window) = window.single() else {
        *prev = None;
        return;
    };
    let Some(cam_id) = camera_touch_id(&touches, window) else {
        *prev = None;
        return;
    };
    let Some(cam_touch) = touches.get_pressed(cam_id) else {
        *prev = None;
        return;
    };
    let pos = cam_touch.position();
    if let Some((id, prev_pos)) = *prev {
        if id == cam_id {
            let delta = pos - prev_pos;
            orbit.target_yaw += delta.x * TOUCH_ROTATE_SPEED;
            orbit.target_radius = (orbit.target_radius - delta.y * TOUCH_ZOOM_SPEED)
                .clamp(MIN_ZOOM_RADIUS, MAX_ZOOM_RADIUS);
        }
    }
    *prev = Some((cam_id, pos));
}

pub fn follow_local_player(
    time: Res<Time>,
    local: Query<&Transform, With<LocalPlayer>>,
    mut orbit: ResMut<CameraOrbit>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<LocalPlayer>)>,
) {
    let Ok(target) = local.single() else {
        return;
    };
    let dt = time.delta_secs();
    let rot_t = 1.0 - (-ROTATION_SMOOTHING * dt).exp();
    orbit.yaw += (orbit.target_yaw - orbit.yaw) * rot_t;
    orbit.radius += (orbit.target_radius - orbit.radius) * rot_t;

    let offset = Vec3::new(
        orbit.radius * orbit.yaw.sin(),
        CAMERA_HEIGHT,
        orbit.radius * orbit.yaw.cos(),
    );
    let desired = target.translation + offset;
    let t = 1.0 - (-CAMERA_SMOOTHING * dt).exp();
    for mut camera_transform in camera.iter_mut() {
        camera_transform.translation = camera_transform.translation.lerp(desired, t);
        camera_transform.look_at(target.translation, Vec3::Y);
    }
}

pub fn spawn_ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.55, 0.30),
            perceptual_roughness: 0.9,
            metallic: 0.0,
            ..default()
        })),
        Transform::IDENTITY,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::touch::{TouchInput, TouchPhase};
    use bevy::window::WindowResolution;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_systems(Startup, spawn_camera);
        app
    }

    /// App with a real `Touches` resource, a primary window sized like a
    /// portrait phone, and the touch camera system.
    fn camera_touch_app() -> App {
        let mut app = App::new();
        app.init_resource::<bevy::input::touch::Touches>();
        app.add_message::<TouchInput>();
        app.init_resource::<CameraOrbit>();
        let window = Window {
            resolution: WindowResolution::new(390, 844),
            ..default()
        };
        app.world_mut().spawn((window, PrimaryWindow));
        app.add_systems(
            Update,
            (
                bevy::input::touch::touch_screen_input_system,
                touch_camera_input,
            )
                .chain(),
        );
        app
    }

    fn touch(app: &mut App, id: u64, phase: TouchPhase, pos: Vec2) {
        app.world_mut().write_message(TouchInput {
            id,
            position: pos,
            phase,
            force: None,
            window: Entity::PLACEHOLDER,
        });
    }

    #[test]
    fn camera_has_correct_position() {
        let mut app = test_app();
        app.update();

        let mut query = app.world_mut().query::<(&Camera3d, &Transform)>();
        let cameras: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(cameras.len(), 1);
        let t = cameras[0].1;
        assert!((t.translation.x - 7.0).abs() < 1e-5);
        assert!((t.translation.y - 8.0).abs() < 1e-5);
        assert!((t.translation.z - 7.0).abs() < 1e-5);
    }

    #[test]
    fn camera_looks_at_origin() {
        let mut app = test_app();
        app.update();

        let mut query = app.world_mut().query::<(&Camera3d, &Camera)>();
        assert_eq!(query.iter(app.world()).len(), 1);
    }

    #[test]
    fn light_is_present() {
        let mut app = test_app();
        app.update();

        let mut query = app.world_mut().query::<&DirectionalLight>();
        assert_eq!(query.iter(app.world()).len(), 1);
    }

    #[test]
    fn point_lights_are_marked_for_toggle() {
        use crate::graphics::LightToggle;

        let mut app = test_app();
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&Visibility, (With<PointLight>, With<LightToggle>)>();
        let visibilities: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(
            visibilities.len(),
            2,
            "both point lights must carry LightToggle and Visibility"
        );
        assert!(visibilities.iter().all(|v| *v == Visibility::Visible));
    }

    #[test]
    fn orbit_defaults_to_isometric_angle() {
        let orbit = CameraOrbit::default();
        assert_eq!(orbit.yaw, std::f32::consts::FRAC_PI_4);
        assert_eq!(orbit.target_yaw, std::f32::consts::FRAC_PI_4);
    }

    #[test]
    fn q_rotates_camera_counterclockwise() {
        let mut app = App::new();
        app.init_resource::<CameraOrbit>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, rotate_camera_input);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();

        let orbit = app.world().resource::<CameraOrbit>();
        let expected = std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_4;
        assert!((orbit.target_yaw - expected).abs() < 1e-5);
    }

    #[test]
    fn e_rotates_camera_clockwise() {
        let mut app = App::new();
        app.init_resource::<CameraOrbit>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, rotate_camera_input);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        app.update();

        let orbit = app.world().resource::<CameraOrbit>();
        let expected = std::f32::consts::FRAC_PI_4 - std::f32::consts::FRAC_PI_4;
        assert!((orbit.target_yaw - expected).abs() < 1e-5);
    }

    #[test]
    fn rotation_accumulates_over_presses() {
        let mut app = App::new();
        app.init_resource::<CameraOrbit>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, rotate_camera_input);

        for _ in 0..4 {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyQ);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .clear();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyQ);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .clear();
        }

        let orbit = app.world().resource::<CameraOrbit>();
        let expected = std::f32::consts::FRAC_PI_4 + std::f32::consts::PI;
        assert!(
            (orbit.target_yaw - expected).abs() < 1e-4,
            "target_yaw={} expected={}",
            orbit.target_yaw,
            expected
        );
    }

    #[test]
    fn ground_is_present() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_systems(Startup, spawn_ground);
        app.update();

        let mut query = app.world_mut().query::<(&Mesh3d, &Transform)>();
        assert_eq!(query.iter(app.world()).count(), 1);
    }

    // ─── Touch camera (right-half zone) ────────────────────────────────

    #[test]
    fn camera_touch_drag_orbits_yaw_without_movement_touch() {
        let mut app = camera_touch_app();
        let yaw_before = app.world().resource::<CameraOrbit>().target_yaw;

        // Frame 1: press + first move in the right half (records start).
        touch(&mut app, 7, TouchPhase::Started, Vec2::new(300.0, 400.0));
        touch(&mut app, 7, TouchPhase::Moved, Vec2::new(320.0, 400.0));
        app.update();
        let yaw_start = app.world().resource::<CameraOrbit>().target_yaw;
        assert_eq!(
            yaw_start, yaw_before,
            "first frame must only record the touch"
        );

        // Frame 2: horizontal drag +20px rotates the camera.
        touch(&mut app, 7, TouchPhase::Moved, Vec2::new(340.0, 400.0));
        app.update();
        let expected = yaw_before + 20.0 * TOUCH_ROTATE_SPEED;
        let yaw_after = app.world().resource::<CameraOrbit>().target_yaw;
        assert!(
            (yaw_after - expected).abs() < 1e-4,
            "expected yaw {expected}, got {yaw_after}"
        );
    }

    #[test]
    fn camera_touch_vertical_drag_zooms_radius() {
        let mut app = camera_touch_app();
        let radius_before = app.world().resource::<CameraOrbit>().target_radius;

        touch(&mut app, 8, TouchPhase::Started, Vec2::new(300.0, 500.0));
        touch(&mut app, 8, TouchPhase::Moved, Vec2::new(300.0, 480.0));
        app.update();

        touch(&mut app, 8, TouchPhase::Moved, Vec2::new(300.0, 460.0));
        app.update();
        let expected =
            (radius_before + 20.0 * TOUCH_ZOOM_SPEED).clamp(MIN_ZOOM_RADIUS, MAX_ZOOM_RADIUS);
        let radius_after = app.world().resource::<CameraOrbit>().target_radius;
        assert!(
            (radius_after - expected).abs() < 1e-4,
            "expected radius {expected}, got {radius_after}"
        );
    }

    #[test]
    fn movement_touch_alone_does_not_move_the_camera() {
        let mut app = camera_touch_app();
        let orbit_before = app.world().resource::<CameraOrbit>().clone();

        touch(&mut app, 9, TouchPhase::Started, Vec2::new(100.0, 400.0));
        touch(&mut app, 9, TouchPhase::Moved, Vec2::new(150.0, 400.0));
        app.update();
        touch(&mut app, 9, TouchPhase::Moved, Vec2::new(200.0, 400.0));
        app.update();

        let orbit = app.world().resource::<CameraOrbit>();
        assert_eq!(orbit.target_yaw, orbit_before.target_yaw);
        assert_eq!(orbit.target_radius, orbit_before.target_radius);
    }

    #[test]
    fn jump_touch_does_not_move_the_camera() {
        let mut app = camera_touch_app();
        let orbit_before = app.world().resource::<CameraOrbit>().clone();

        // Jump button center in the 390x844 portrait window.
        touch(&mut app, 10, TouchPhase::Started, Vec2::new(322.0, 68.0));
        touch(&mut app, 10, TouchPhase::Moved, Vec2::new(360.0, 90.0));
        app.update();
        touch(&mut app, 10, TouchPhase::Moved, Vec2::new(380.0, 110.0));
        app.update();

        let orbit = app.world().resource::<CameraOrbit>();
        assert_eq!(orbit.target_yaw, orbit_before.target_yaw);
        assert_eq!(orbit.target_radius, orbit_before.target_radius);
    }
}
