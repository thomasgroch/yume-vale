use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::flow::AppFlow;
use crate::ui::{theme, widgets};

/// Fraction of the window (from the right / from the bottom) where the jump
/// button lives; touches starting there must not drive movement.
const JUMP_ZONE_RIGHT: f32 = 0.30;
const JUMP_ZONE_BOTTOM: f32 = 0.35;
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

/// Whether a window position lies inside the jump button zone.
pub fn in_jump_zone(pos: Vec2, window: &Window) -> bool {
    pos.x > window.width() * (1.0 - JUMP_ZONE_RIGHT) && pos.y < window.height() * JUMP_ZONE_BOTTOM
}

/// The first touch that did not start inside the jump button zone.
pub fn movement_touch_id(touches: &Touches, window: &Window) -> Option<u64> {
    touches
        .iter()
        .find(|t| !in_jump_zone(t.start_position(), window))
        .map(|t| t.id())
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
}
