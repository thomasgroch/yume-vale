use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use game_client::ClientPlugin;

fn main() {
    let asset_path = if cfg!(target_arch = "wasm32") {
        "assets/"
    } else {
        "../../assets"
    };
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_path.to_string(),
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
        .add_plugins(ClientPlugin::default())
        .run();
}
