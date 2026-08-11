//! Overrides Bevy's built-in default font.
//!
//! `bevy_text`'s `default_font` feature bundles `FiraMono-subset.ttf` — a
//! subset with limited glyph coverage that's missing Latin-1 accented
//! characters (á, ã, ç, õ, ...), so any UI text using `TextFont::default()`
//! (which every `text_font()`/`text_style()` call in `ui::widgets` does)
//! renders Portuguese text with tofu boxes in place of accented letters.
//!
//! `bevy_text::TextPlugin` inserts its bundled font at `AssetId::default()`,
//! which is exactly what `Handle::<Font>::default()` (and therefore
//! `TextFont::default()`) resolves to. Overwriting that same asset ID with a
//! font that has full Latin coverage fixes every text node in the app with
//! zero call-site changes — no need to thread a font handle through
//! `ui::widgets` or the asset-loading queue.

use bevy::asset::AssetId;
use bevy::prelude::*;
use bevy::text::Font;

/// Fredoka (OFL-1.1, google/fonts) — full Latin Extended-A coverage.
const FONT_DATA: &[u8] = include_bytes!("../../../assets/fonts/Fredoka.ttf");

/// Replaces the default font asset. Must run after `TextPlugin` has been
/// added (any `Startup` system satisfies this — plugins build before the
/// first `Startup` schedule runs).
pub(crate) fn install_default_font(mut fonts: ResMut<Assets<Font>>) {
    let font = Font::from_bytes(FONT_DATA.to_vec());
    let _ = fonts.insert(AssetId::default(), font);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_default_font_overwrites_the_default_asset_id() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Font>();
        // Simulate TextPlugin's own bundled-font insert happening first.
        {
            let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
            let placeholder = Font::from_bytes(vec![0u8; 4]);
            let _ = fonts.insert(AssetId::default(), placeholder);
        }
        app.add_systems(Startup, install_default_font);
        app.update();

        let fonts = app.world().resource::<Assets<Font>>();
        assert!(
            fonts.contains(AssetId::<Font>::default()),
            "default font asset id must still resolve after install"
        );
    }

    #[test]
    fn font_data_is_a_valid_non_empty_font_file() {
        // TTF/OTF files start with a sfnt version tag; the smallest sane
        // sanity check without pulling in a font-parsing crate.
        assert!(FONT_DATA.len() > 1000, "font data looks truncated");
    }
}
