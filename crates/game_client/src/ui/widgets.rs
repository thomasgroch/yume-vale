//! Reusable Bevy UI widget builders.
//!
//! Every public fn here returns a bundle of Bevy components so callers can
//! `.spawn(…)` or `.with_children(|p| { p.spawn((…)); })` directly.  All
//! colours and dimensions reference [`crate::ui::theme`] tokens.

use bevy::prelude::*;

use crate::ui::theme;

// ─── Structural builders ─────────────────────────────────────────────────

/// A fully rounded circle / pill node.
///
/// Use for touch overlays (jump button, joystick feedback) and the menu
/// play‑button shape.
pub fn pill(diameter: f32, color: impl Into<Color>) -> (Node, BackgroundColor) {
    (
        Node {
            width: Val::Px(diameter),
            height: Val::Px(diameter),
            border_radius: BorderRadius::all(Val::Px(theme::RADIUS_PILL)),
            ..default()
        },
        BackgroundColor(color.into()),
    )
}

/// A decorative floating bubble — absolutely positioned circle with
/// `BorderRadius::MAX`.
///
/// Used on the menu screen behind the title card.
pub fn bubble(size: f32, top: f32, left: f32, color: impl Into<Color>) -> (Node, BackgroundColor) {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(size),
            height: Val::Px(size),
            top: Val::Px(top),
            left: Val::Px(left),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(color.into()),
    )
}

// ─── Button builders ─────────────────────────────────────────────────────

/// Style a [`Button`] entity as a pill‑shaped interactive element.
///
/// ```ignore
/// commands.spawn((
///     Button,
///     button_frame(SPACE_64, SPACE_16),
///     BackgroundColor(BUTTON_PRIMARY),
/// ));
/// ```
pub fn button_frame(h_padding: f32, v_padding: f32) -> Node {
    Node {
        padding: UiRect::axes(Val::Px(h_padding), Val::Px(v_padding)),
        border_radius: BorderRadius::all(Val::Px(theme::RADIUS_PILL)),
        ..default()
    }
}

// ─── Text builders ───────────────────────────────────────────────────────

/// A [`TextFont`] sized from a theme token.
///
/// ```ignore
/// commands.spawn((
///     Text::new("Hello"),
///     text_font(FONT_MD),
///     TextColor(Color::WHITE),
/// ));
/// ```
pub fn text_font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

/// Shorthand for the common `(TextFont, TextColor, TextShadow)` trio.
///
/// ```ignore
/// commands.spawn((
///     Text::new("Hello"),
///     text_style(FONT_MD, Color::WHITE),
/// ));
/// ```
pub fn text_style(size: f32, color: impl Into<Color>) -> (TextFont, TextColor, TextShadow) {
    (
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color.into()),
        TextShadow::default(),
    )
}

// ─── Layout helpers ──────────────────────────────────────────────────────

/// An absolutely positioned [`Node`] with the given edges.
///
/// Pass `None` for edges that should remain `Val::Auto`.
pub fn absolute(
    top: Option<f32>,
    left: Option<f32>,
    bottom: Option<f32>,
    right: Option<f32>,
) -> Node {
    Node {
        position_type: PositionType::Absolute,
        top: top.map(Val::Px).unwrap_or(Val::Auto),
        left: left.map(Val::Px).unwrap_or(Val::Auto),
        bottom: bottom.map(Val::Px).unwrap_or(Val::Auto),
        right: right.map(Val::Px).unwrap_or(Val::Auto),
        ..default()
    }
}

/// A fixed‑size [`Node`] (width + height).
pub fn fixed_size(w: f32, h: f32) -> Node {
    Node {
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::color::Color;

    #[test]
    fn pill_uses_theme_radius() {
        let (node, _bg) = pill(64.0, Color::WHITE);
        assert_eq!(
            node.border_radius,
            BorderRadius::all(Val::Px(theme::RADIUS_PILL)),
            "pill widget must use theme::RADIUS_PILL"
        );
    }

    #[test]
    fn bubble_uses_max_radius() {
        let (node, _bg) = bubble(100.0, 0.0, 0.0, Color::WHITE);
        assert_eq!(
            node.border_radius,
            BorderRadius::MAX,
            "bubble must be circular"
        );
        assert_eq!(node.position_type, PositionType::Absolute);
    }

    #[test]
    fn button_frame_uses_pill_radius() {
        let node = button_frame(32.0, 12.0);
        assert_eq!(
            node.border_radius,
            BorderRadius::all(Val::Px(theme::RADIUS_PILL)),
        );
        assert_eq!(node.padding.left, Val::Px(32.0));
        assert_eq!(node.padding.top, Val::Px(12.0));
    }

    #[test]
    fn text_font_sets_size() {
        let tf = text_font(theme::FONT_TITLE);
        assert_eq!(tf.font_size, FontSize::Px(theme::FONT_TITLE));
    }

    #[test]
    fn text_style_includes_shadow() {
        let (_font, _color, shadow) = text_style(14.0, Color::srgb(1.0, 0.0, 0.0));
        let _ = shadow; // presence is the assertion
    }

    #[test]
    fn absolute_sets_edges() {
        let node = absolute(Some(10.0), Some(20.0), None, Some(30.0));
        assert_eq!(node.top, Val::Px(10.0));
        assert_eq!(node.left, Val::Px(20.0));
        assert_eq!(node.bottom, Val::Auto);
        assert_eq!(node.right, Val::Px(30.0));
    }

    #[test]
    fn panel_and_button_use_theme_tokens() {
        // Using the pill and button_frame builders, verify they produce
        // nodes whose geometry references theme constants.
        let (pill_node, _bg) = pill(theme::SPACE_72, Color::WHITE);
        assert_eq!(pill_node.width, Val::Px(theme::SPACE_72));

        let btn = button_frame(theme::SPACE_64, theme::SPACE_16);
        assert_eq!(btn.padding.left, Val::Px(theme::SPACE_64));
        assert_eq!(btn.padding.top, Val::Px(theme::SPACE_16));
    }
}
