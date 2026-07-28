//! Pastel design system tokens for Yume Vale.
//!
//! Every visual constant lives here — colors, spacing, radii, font sizes,
//! and interaction state maps. Consumers import tokens by semantic name
//! rather than inlining raw `Color::srgb(…)` or `Val::Px(…)` literals.

use bevy::color::Color;

// ─── Palette ──────────────────────────────────────────────────────────────

/// Pastel pink page background (menu screen).
pub const SURFACE_MENU: Color = Color::srgb(1.0, 0.90, 0.94);

/// Soft lavender loading screen background.
pub const SURFACE_LOADING: Color = Color::srgb(0.92, 0.92, 1.0);

/// Rosy accent — used for the game title.
pub const TEXT_TITLE: Color = Color::srgb(0.88, 0.42, 0.58);

/// Slate blue-grey — used for subtitle, hints, and secondary labels.
pub const TEXT_SUBTLE: Color = Color::srgb(0.43, 0.50, 0.58);

/// Pink primary button background.
pub const BUTTON_PRIMARY: Color = Color::srgb(1.0, 0.56, 0.67);

/// Darker pink button hover/pressed state.
pub const BUTTON_PRIMARY_HOVER: Color = Color::srgb(1.0, 0.48, 0.60);

/// HUD status indicator: connected / healthy.
pub const STATUS_OK: Color = Color::srgb(0.4, 0.9, 0.4);

/// HUD status indicator: connecting / transitional.
pub const STATUS_BUSY: Color = Color::srgb(0.9, 0.8, 0.3);

/// HUD status indicator: disconnected / error.
pub const STATUS_ERR: Color = Color::srgb(0.9, 0.4, 0.4);

/// Version / debug text — white at low opacity.
pub const TEXT_DIM: Color = Color::srgba(1.0, 1.0, 1.0, 0.55);

/// Reconnect button background — dark neutral.
pub const SURFACE_RECONNECT: Color = Color::srgb(0.25, 0.25, 0.3);

// ─── Decorative bubbles (menu) ───────────────────────────────────────────

/// Pastel pink decorative bubble.
pub const BUBBLE_PINK: Color = Color::srgba(1.0, 0.78, 0.85, 0.5);

/// Pastel blue decorative bubble.
pub const BUBBLE_BLUE: Color = Color::srgba(0.78, 0.90, 1.0, 0.5);

/// Pastel green decorative bubble.
pub const BUBBLE_GREEN: Color = Color::srgba(0.80, 0.96, 0.85, 0.5);

// ─── Touch overlay ───────────────────────────────────────────────────────

/// Jump button circle — white at very low opacity.
pub const OVERLAY_JUMP: Color = Color::srgba(1.0, 1.0, 1.0, 0.18);

/// Jump label — white at medium opacity.
pub const OVERLAY_JUMP_TEXT: Color = Color::srgba(1.0, 1.0, 1.0, 0.7);

/// Joystick ring — white at very low opacity.
pub const OVERLAY_RING: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);

/// Joystick knob — white at low opacity.
pub const OVERLAY_KNOB: Color = Color::srgba(1.0, 1.0, 1.0, 0.25);

// ─── Spacing ─────────────────────────────────────────────────────────────

/// 4 px — tightest gap.
pub const SPACE_4: f32 = 4.0;
/// 6 px — tight gap.
pub const SPACE_6: f32 = 6.0;
/// 8 px — small gap.
pub const SPACE_8: f32 = 8.0;
/// 10 px — HUD inset.
pub const SPACE_10: f32 = 10.0;
/// 11 px — version text margin.
pub const SPACE_11: f32 = 11.0;
/// 14 px — hint text.
pub const SPACE_14: f32 = 14.0;
/// 16 px — standard padding.
pub const SPACE_16: f32 = 16.0;
/// 20 px — subtitle.
pub const SPACE_20: f32 = 20.0;
/// 24 px — bottom margin.
pub const SPACE_24: f32 = 24.0;
/// 28 px — button text.
pub const SPACE_28: f32 = 28.0;
/// 32 px — touch inset, joypad margin.
pub const SPACE_32: f32 = 32.0;
/// 48 px — large margin.
pub const SPACE_48: f32 = 48.0;
/// 64 px — button horizontal padding.
pub const SPACE_64: f32 = 64.0;
/// 72 px — jump button diameter.
pub const SPACE_72: f32 = 72.0;
/// 80 px — title font size.
pub const SPACE_80: f32 = 80.0;

// ─── Accessibility constants ─────────────────────────────────────────────

/// Minimum touch/click target size (WCAG 2.1 AA).
pub const MIN_TOUCH_TARGET: f32 = 44.0;

// ─── Border radius ───────────────────────────────────────────────────────

/// Pill shape (fully rounded `999` px) — buttons, touch circles.
pub const RADIUS_PILL: f32 = 999.0;

// ─── Font sizes (kept as f32 for TextFont consumers) ─────────────────────

/// 11 px — version / debug overlay.
pub const FONT_XS: f32 = 11.0;
/// 14 px — controls hint.
pub const FONT_SM: f32 = 14.0;
/// 16 px — touch button label, status text.
pub const FONT_MD: f32 = 16.0;
/// 20 px — subtitle.
pub const FONT_LG: f32 = 20.0;
/// 28 px — button label.
pub const FONT_XL: f32 = 28.0;
/// 80 px — game title.
pub const FONT_TITLE: f32 = 80.0;

// ─── Interaction helpers ─────────────────────────────────────────────────

/// Map an [`Interaction`] to the colour a primary button should show.
///
/// Returns the resting colour for `Interaction::None` and the hover/pressed
/// colour for both `Hovered` and `Pressed`.
pub fn button_interaction_color(interaction: &bevy::ui::Interaction) -> Color {
    match interaction {
        bevy::ui::Interaction::Pressed | bevy::ui::Interaction::Hovered => BUTTON_PRIMARY_HOVER,
        bevy::ui::Interaction::None => BUTTON_PRIMARY,
    }
}

// ─── Accessibility helpers ───────────────────────────────────────────────

/// WCAG 2.1 relative luminance from an sRGB triple in `[0, 1]`.
fn channel_luminance(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Approximate WCAG 2.1 relative luminance from a Bevy `Color`.
///
/// Works with any colour that was created via `Color::srgb` or `Color::srgba`
/// (the standard sRGB constructors).  Returns a value in `[0.0, 1.0]`.
pub fn relative_luminance(c: Color) -> f32 {
    let (r, g, b) = srgb_channels(c);
    0.2126 * channel_luminance(r) + 0.7152 * channel_luminance(g) + 0.0722 * channel_luminance(b)
}

/// Extract normalised `(r, g, b)` sRGB channels from a Bevy `Color`.
///
/// Bevy stores colour internally as linear RGBA, but our constants are
/// built with `Color::srgb` which tags them as sRGB.  We apply the
/// linear → sRGB gamma curve to recover the original sRGB values for
/// the WCAG luminance calculation.
fn srgb_channels(c: Color) -> (f32, f32, f32) {
    let linear = c.to_linear();
    let gamma = |v: f32| {
        if v <= 0.0031308 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    (gamma(linear.red), gamma(linear.green), gamma(linear.blue))
}

/// Compute the contrast ratio between two colours (WCAG 2.1).
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::color::Color;

    /// Verify the contrast-ratio utility itself is correct.
    ///
    /// This does NOT enforce WCAG AA (pastels inherently have lower contrast)
    /// — that would be a design change, which is out of scope.  The actual
    /// ratios are documented in [`DESIGN.md`] for reference.
    #[test]
    fn contrast_ratio_utility_is_correct() {
        // Same colour against itself → ratio should be 1.0
        assert!((contrast_ratio(Color::WHITE, Color::WHITE) - 1.0).abs() < 0.001);

        // White vs black → approximately 21:1
        let wb = contrast_ratio(Color::WHITE, Color::BLACK);
        assert!((wb - 21.0).abs() < 1.0, "white/black ratio: {wb}");

        // Actual palette ratios (documented, not enforced)
        let btn_text_on_bg = contrast_ratio(Color::WHITE, BUTTON_PRIMARY);
        let _bg_on_surface = contrast_ratio(BUTTON_PRIMARY, SURFACE_MENU);
        let _title_on_surface = contrast_ratio(TEXT_TITLE, SURFACE_MENU);
        let _subtle_on_surface = contrast_ratio(TEXT_SUBTLE, SURFACE_MENU);
        // Sanity: white on pink is at least barely visible
        assert!(
            btn_text_on_bg > 1.5,
            "white on BUTTON_PRIMARY: {btn_text_on_bg}"
        );
    }
}
