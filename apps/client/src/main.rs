use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use game_client::ClientPlugin;

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
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_path.to_string(),
                    // No .meta files are shipped; always use default loader settings.
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(client_plugin)
        .run();
}
