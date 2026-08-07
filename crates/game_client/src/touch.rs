use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::flow::AppFlow;
use crate::ui::{theme, widgets};

/// Diameter of the on-screen jump button; the hitbox derives from the visual.
const JUMP_BUTTON_SIZE: f32 = theme::SPACE_72;
/// Distance of the jump button from the right / bottom screen edges.
const JUMP_BUTTON_MARGIN: f32 = theme::SPACE_32;
/// Extra catch area allowed around the visual jump button (capped at 8px).
const JUMP_HITBOX_PAD: f32 = 8.0;
/// Touch starts left of this fraction of the width drive movement; the rest
/// (right half) drives the camera.
const LEFT_RIGHT_SPLIT: f32 = 0.5;
const _: () = assert!(
    JUMP_HITBOX_PAD <= 8.0,
    "jump hitbox padding must not exceed 8px"
);

const JOYSTICK_RING_SIZE: f32 = 96.0;
const JOYSTICK_KNOB_SIZE: f32 = 44.0;

/// Set while a finger holds the on-screen jump button.
#[derive(Resource, Default)]
pub struct TouchJump(pub bool);

/// True once any touch has been seen; reveals the touch-only UI (the jump
/// button and joystick feedback are useless clutter on desktop).
#[derive(Resource, Default)]
pub struct TouchDetected(pub bool);

#[derive(Component)]
pub struct TouchUi;
#[derive(Component)]
pub struct JumpButton;
#[derive(Component)]
pub struct JoystickRing;
#[derive(Component)]
pub struct JoystickKnob;

pub fn spawn_touch_ui(mut commands: Commands) {
    let (node, bg) = widgets::pill(theme::SPACE_72, theme::OVERLAY_JUMP);
    commands
        .spawn((
            TouchUi,
            JumpButton,
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(theme::SPACE_32),
                bottom: Val::Px(theme::SPACE_32),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..node
            },
            bg,
            Visibility::Hidden,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Pular"),
                widgets::text_font(theme::FONT_MD),
                TextColor(theme::OVERLAY_JUMP_TEXT),
            ));
        });

    let (ring_node, ring_bg) = widgets::pill(JOYSTICK_RING_SIZE, theme::OVERLAY_RING);
    commands.spawn((
        TouchUi,
        JoystickRing,
        Node {
            position_type: PositionType::Absolute,
            ..ring_node
        },
        ring_bg,
        Visibility::Hidden,
    ));
    let (knob_node, knob_bg) = widgets::pill(JOYSTICK_KNOB_SIZE, theme::OVERLAY_KNOB);
    commands.spawn((
        TouchUi,
        JoystickKnob,
        Node {
            position_type: PositionType::Absolute,
            ..knob_node
        },
        knob_bg,
        Visibility::Hidden,
    ));
}

pub fn detect_touch(touches: Res<Touches>, mut detected: ResMut<TouchDetected>) {
    // iter_just_pressed also catches quick taps (pressed+released within one
    // frame), which iter() would miss entirely.
    if !detected.0 && touches.iter_just_pressed().next().is_some() {
        detected.0 = true;
    }
}

/// The jump button appears only on touch devices, during gameplay.
pub fn touch_ui_visibility(
    detected: Res<TouchDetected>,
    flow: Res<State<AppFlow>>,
    mut jump_ui: Query<&mut Visibility, With<JumpButton>>,
) {
    let visible = if detected.0 && flow.get() == &AppFlow::InGame {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut vis in &mut jump_ui {
        *vis = visible;
    }
}

pub fn jump_button_input(
    interactions: Query<&Interaction, (Changed<Interaction>, With<JumpButton>)>,
    mut jump: ResMut<TouchJump>,
) {
    for interaction in &interactions {
        jump.0 = matches!(interaction, Interaction::Pressed);
    }
}

/// Touch positions are window coordinates (origin bottom-left); bevy UI uses
/// origin top-left, so Y flips.
fn to_ui_pos(window_pos: Vec2, window_height: f32) -> Vec2 {
    Vec2::new(window_pos.x, window_height - window_pos.y)
}

/// The control a touch is assigned to, decided once at press time purely from
/// its start position (window coords, origin bottom-left).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchRole {
    /// Started in the left half of the screen: drives the movement joystick.
    Movement,
    /// Started in the right half: one-finger camera orbit + vertical-drag zoom.
    Camera,
    /// Started on the jump button: jump only, never movement or camera.
    Jump,
}

/// The jump button rectangle (window coords, bottom-left origin): a
/// [`JUMP_BUTTON_SIZE`]-px square `JUMP_BUTTON_MARGIN` px from the right and
/// bottom edges, grown by [`JUMP_HITBOX_PAD`] on every side.
fn jump_hitbox(width: f32) -> Rect {
    let right = width - JUMP_BUTTON_MARGIN;
    Rect::from_corners(
        Vec2::new(
            right - JUMP_BUTTON_SIZE - JUMP_HITBOX_PAD,
            JUMP_BUTTON_MARGIN - JUMP_HITBOX_PAD,
        ),
        Vec2::new(
            right + JUMP_HITBOX_PAD,
            JUMP_BUTTON_MARGIN + JUMP_BUTTON_SIZE + JUMP_HITBOX_PAD,
        ),
    )
}

/// Classify a window position into a [`TouchRole`]. Jump wins over the half
/// split; every point is covered (no dead zones).
pub fn touch_role(pos: Vec2, width: f32) -> TouchRole {
    if jump_hitbox(width).contains(pos) {
        TouchRole::Jump
    } else if pos.x < width * LEFT_RIGHT_SPLIT {
        TouchRole::Movement
    } else {
        TouchRole::Camera
    }
}

/// The first touch whose start position classifies as `role`.
fn first_touch_with_role(touches: &Touches, window: &Window, role: TouchRole) -> Option<u64> {
    touches
        .iter()
        .find(|t| touch_role(t.start_position(), window.width()) == role)
        .map(|t| t.id())
}

/// The first touch that started in the left half (drives the joystick).
pub fn movement_touch_id(touches: &Touches, window: &Window) -> Option<u64> {
    first_touch_with_role(touches, window, TouchRole::Movement)
}

/// The first touch that started in the right half (drives the camera).
pub fn camera_touch_id(touches: &Touches, window: &Window) -> Option<u64> {
    first_touch_with_role(touches, window, TouchRole::Camera)
}

/// Shows the joystick ring/knob under the movement touch while it drags.
#[allow(clippy::type_complexity)]
pub fn update_joystick_ui(
    touches: Res<Touches>,
    detected: Res<TouchDetected>,
    flow: Res<State<AppFlow>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut joystick: ParamSet<(
        Query<(&mut Node, &mut Visibility), With<JoystickRing>>,
        Query<(&mut Node, &mut Visibility), With<JoystickKnob>>,
    )>,
) {
    let Ok(window) = window.single() else {
        return;
    };
    let active = detected.0 && flow.get() == &AppFlow::InGame;
    let touch = active
        .then(|| movement_touch_id(&touches, window))
        .flatten()
        .and_then(|id| touches.get_pressed(id));

    let Some(touch) = touch else {
        for mut vis in joystick.p0().iter_mut().map(|(_, v)| v).collect::<Vec<_>>() {
            *vis = Visibility::Hidden;
        }
        for mut vis in joystick.p1().iter_mut().map(|(_, v)| v).collect::<Vec<_>>() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let start = to_ui_pos(touch.start_position(), window.height());
    let current = to_ui_pos(touch.position(), window.height());
    {
        let mut ring = joystick.p0();
        if let Ok((mut node, mut vis)) = ring.single_mut() {
            node.left = Val::Px(start.x - JOYSTICK_RING_SIZE / 2.0);
            node.top = Val::Px(start.y - JOYSTICK_RING_SIZE / 2.0);
            *vis = Visibility::Inherited;
        }
    }
    {
        let mut knob = joystick.p1();
        if let Ok((mut node, mut vis)) = knob.single_mut() {
            node.left = Val::Px(current.x - JOYSTICK_KNOB_SIZE / 2.0);
            node.top = Val::Px(current.y - JOYSTICK_KNOB_SIZE / 2.0);
            *vis = Visibility::Inherited;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::touch::{TouchInput, TouchPhase};
    use bevy::window::WindowResolution;

    const PORTRAIT: (f32, f32) = (390.0, 844.0);
    const LANDSCAPE: (f32, f32) = (844.0, 390.0);

    fn window(width: f32, height: f32) -> Window {
        Window {
            resolution: WindowResolution::new(width as u32, height as u32),
            ..default()
        }
    }

    fn jump_center(width: f32) -> Vec2 {
        let right = width - JUMP_BUTTON_MARGIN;
        Vec2::new(
            right - JUMP_BUTTON_SIZE / 2.0,
            JUMP_BUTTON_MARGIN + JUMP_BUTTON_SIZE / 2.0,
        )
    }

    #[test]
    fn ui_pos_flips_y() {
        assert_eq!(
            to_ui_pos(Vec2::new(10.0, 30.0), 800.0),
            Vec2::new(10.0, 770.0)
        );
    }

    #[test]
    fn jump_button_sets_touch_jump_while_pressed() {
        let mut app = App::new();
        app.init_resource::<TouchJump>();
        app.add_systems(Update, jump_button_input);
        let button = app.world_mut().spawn((JumpButton, Interaction::None)).id();

        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        app.update();
        assert!(app.world().resource::<TouchJump>().0);

        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::None;
        app.update();
        assert!(!app.world().resource::<TouchJump>().0);
    }

    // ─── Zone classifier ───────────────────────────────────────────────

    #[test]
    fn left_half_is_movement_in_both_orientations() {
        for (width, height) in [PORTRAIT, LANDSCAPE] {
            let pos = Vec2::new(width * 0.25, height * 0.5);
            assert_eq!(touch_role(pos, width), TouchRole::Movement);
        }
    }

    #[test]
    fn right_half_is_camera_in_both_orientations() {
        for (width, height) in [PORTRAIT, LANDSCAPE] {
            let pos = Vec2::new(width * 0.75, height * 0.5);
            assert_eq!(touch_role(pos, width), TouchRole::Camera);
        }
    }

    #[test]
    fn exact_50_percent_boundary_goes_to_camera() {
        for (width, height) in [PORTRAIT, LANDSCAPE] {
            let boundary = width * 0.5;
            assert_eq!(
                touch_role(Vec2::new(boundary, height * 0.5), width),
                TouchRole::Camera,
                "x == 50% must be camera"
            );
            assert_eq!(
                touch_role(Vec2::new(boundary - 0.5, height * 0.5), width),
                TouchRole::Movement,
                "just left of 50% must be movement"
            );
        }
    }

    #[test]
    fn jump_center_is_jump_in_both_orientations() {
        for width in [PORTRAIT.0, LANDSCAPE.0] {
            assert_eq!(touch_role(jump_center(width), width), TouchRole::Jump);
        }
    }

    #[test]
    fn visual_button_corners_are_jump() {
        for width in [PORTRAIT.0, LANDSCAPE.0] {
            let right = width - JUMP_BUTTON_MARGIN;
            assert_eq!(
                touch_role(
                    Vec2::new(right - JUMP_BUTTON_SIZE, JUMP_BUTTON_MARGIN),
                    width
                ),
                TouchRole::Jump,
                "top-left corner of the visual button"
            );
            assert_eq!(
                touch_role(
                    Vec2::new(right, JUMP_BUTTON_MARGIN + JUMP_BUTTON_SIZE),
                    width
                ),
                TouchRole::Jump,
                "bottom-right corner of the visual button"
            );
        }
    }

    #[test]
    fn jump_hitbox_padding_is_at_most_8px() {
        for (width, _) in [PORTRAIT, LANDSCAPE] {
            let box_ = jump_hitbox(width);
            let visual = Rect::from_corners(
                Vec2::new(
                    width - JUMP_BUTTON_MARGIN - JUMP_BUTTON_SIZE,
                    JUMP_BUTTON_MARGIN,
                ),
                Vec2::new(
                    width - JUMP_BUTTON_MARGIN,
                    JUMP_BUTTON_MARGIN + JUMP_BUTTON_SIZE,
                ),
            );
            assert!(
                box_.contains(visual.min) && box_.contains(visual.max),
                "hitbox must cover the whole visual button"
            );
        }
    }

    #[test]
    fn just_outside_jump_hitbox_is_camera_not_jump() {
        for width in [PORTRAIT.0, LANDSCAPE.0] {
            let box_ = jump_hitbox(width);
            let center = box_.center();
            for pos in [
                Vec2::new(box_.max.x + 0.5, center.y),
                Vec2::new(box_.min.x - 0.5, center.y),
                Vec2::new(center.x, box_.max.y + 0.5),
            ] {
                assert_eq!(
                    touch_role(pos, width),
                    TouchRole::Camera,
                    "point {pos:?} outside the hitbox must be camera"
                );
            }
        }
    }

    #[test]
    fn left_half_inside_jump_band_is_movement() {
        // y inside the jump band but x in the left half: no dead zone, not jump.
        for width in [PORTRAIT.0, LANDSCAPE.0] {
            let pos = Vec2::new(width * 0.25, jump_center(width).y);
            assert_eq!(touch_role(pos, width), TouchRole::Movement);
        }
    }

    #[test]
    fn roles_cover_every_point_exactly_once() {
        for (width, height) in [PORTRAIT, LANDSCAPE] {
            let mut x = 0.0;
            while x <= width {
                let mut y = 0.0;
                while y <= height {
                    let role = touch_role(Vec2::new(x, y), width);
                    assert!(
                        matches!(
                            role,
                            TouchRole::Movement | TouchRole::Camera | TouchRole::Jump
                        ),
                        "point ({x},{y}) must get a role"
                    );
                    y += 25.0;
                }
                x += 25.0;
            }
        }
    }

    // ─── Touch selection ───────────────────────────────────────────────

    /// A minimal app with a working `Touches` resource driven by
    /// [`TouchInput`] messages, mirroring bevy's own touch system.
    fn touches_app() -> App {
        let mut app = App::new();
        app.init_resource::<bevy::input::touch::Touches>();
        app.add_message::<TouchInput>();
        app.add_systems(Update, (bevy::input::touch::touch_screen_input_system,));
        app
    }

    fn press(app: &mut App, id: u64, pos: Vec2) {
        app.world_mut().write_message(TouchInput {
            id,
            position: pos,
            phase: TouchPhase::Started,
            force: None,
            window: Entity::PLACEHOLDER,
        });
    }

    #[test]
    fn simultaneous_left_and_right_touches_get_distinct_roles() {
        let mut app = touches_app();
        let w = window(PORTRAIT.0, PORTRAIT.1);
        press(&mut app, 1, Vec2::new(100.0, 400.0));
        press(&mut app, 2, Vec2::new(300.0, 400.0));
        app.update();

        let touches = app.world().resource::<bevy::input::touch::Touches>();
        assert_eq!(movement_touch_id(touches, &w), Some(1));
        assert_eq!(camera_touch_id(touches, &w), Some(2));
    }

    #[test]
    fn jump_touch_is_selected_by_neither_movement_nor_camera() {
        let mut app = touches_app();
        let w = window(PORTRAIT.0, PORTRAIT.1);
        press(&mut app, 3, jump_center(w.width()));
        app.update();

        let touches = app.world().resource::<bevy::input::touch::Touches>();
        assert_eq!(movement_touch_id(touches, &w), None);
        assert_eq!(camera_touch_id(touches, &w), None);
    }

    #[test]
    fn camera_touch_works_without_any_movement_touch() {
        let mut app = touches_app();
        let w = window(LANDSCAPE.0, LANDSCAPE.1);
        press(&mut app, 4, Vec2::new(600.0, 100.0));
        app.update();

        let touches = app.world().resource::<bevy::input::touch::Touches>();
        assert_eq!(movement_touch_id(touches, &w), None);
        assert_eq!(camera_touch_id(touches, &w), Some(4));
    }

    #[test]
    fn jump_touch_does_not_steal_the_movement_role() {
        let mut app = touches_app();
        let w = window(PORTRAIT.0, PORTRAIT.1);
        press(&mut app, 5, jump_center(w.width()));
        press(&mut app, 6, Vec2::new(100.0, 400.0));
        app.update();

        let touches = app.world().resource::<bevy::input::touch::Touches>();
        assert_eq!(movement_touch_id(touches, &w), Some(6));
        assert_eq!(camera_touch_id(touches, &w), None);
    }
}
