use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use game_client::ClientPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: "../../assets".to_string(),
            ..default()
        }))
        .add_plugins(ClientPlugin::default())
        .run();
}
