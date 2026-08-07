use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use bevy::window::WindowResolution;
use game_client::ClientPlugin;

fn primary_window(viewport: Option<(u32, u32)>) -> Window {
    Window {
        fit_canvas_to_parent: true,
        resolution: viewport.map_or_else(WindowResolution::default, |(width, height)| {
            WindowResolution::new(width, height)
        }),
        ..default()
    }
}

#[cfg(target_arch = "wasm32")]
fn web_viewport() -> Option<(u32, u32)> {
    let window = web_sys::window()?;
    Some((
        window.inner_width().ok()?.as_f64()?.round() as u32,
        window.inner_height().ok()?.as_f64()?.round() as u32,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn web_viewport() -> Option<(u32, u32)> {
    None
}

#[cfg(target_arch = "wasm32")]
fn sync_web_resolution(mut windows: Query<&mut Window>) {
    let Some(browser) = web_sys::window() else {
        return;
    };
    let Some((width, height)) = web_viewport() else {
        return;
    };
    let scale = browser.device_pixel_ratio() as f32;
    let physical_width = (width as f32 * scale).round() as u32;
    let physical_height = (height as f32 * scale).round() as u32;
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if window.resolution.physical_width() != physical_width
        || window.resolution.physical_height() != physical_height
    {
        window
            .resolution
            .set_physical_resolution(physical_width, physical_height);
    }
}

fn main() {
    let asset_path = if cfg!(target_arch = "wasm32") {
        "assets/"
    } else {
        "../../assets"
    };
    #[cfg(not(target_arch = "wasm32"))]
    let client_plugin = {
        let mut plugin = ClientPlugin::default();
        if let Some(addr) =
            game_client::connection::server_addr_from_env(std::env::var("YUME_SERVER_ADDR").ok())
        {
            plugin.config.server_addr = addr;
        }
        plugin
    };
    #[cfg(target_arch = "wasm32")]
    let client_plugin = ClientPlugin::default();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_path.to_string(),
                // No .meta files are shipped; always use default loader settings.
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(primary_window(web_viewport())),
                ..default()
            }),
    )
    .add_plugins(client_plugin);
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, sync_web_resolution);
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_window_starts_at_css_viewport_without_scale_override() {
        let window = primary_window(Some((390, 844)));
        assert_eq!(window.resolution.physical_width(), 390);
        assert_eq!(window.resolution.physical_height(), 844);
        assert_eq!(window.resolution.scale_factor_override(), None);
    }

    #[test]
    fn native_window_uses_platform_scale() {
        assert_eq!(
            primary_window(None).resolution.scale_factor_override(),
            None
        );
    }
}
