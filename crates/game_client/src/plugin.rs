use bevy::prelude::*;
use core::time::Duration;
use game_core::constants::TICK_RATE_HZ;
use game_protocol::ProtocolPlugin;
use lightyear::prelude::client::ClientPlugins;
use player::PlayerPlugin;

use crate::camera::{
    CameraOrbit, follow_local_player, rotate_camera_input, spawn_camera, spawn_ground,
};
use crate::config::ClientConfig;
use crate::connection::{
    LocalPlayerId, handle_welcome, retry_connect_when_disconnected, setup_client,
};
use crate::decorations::spawn_decorations;
use crate::input::{InputState, gather_input};
use crate::visuals::{
    attach_player_visuals, mark_local_player_visuals, sync_position_to_transform,
};

#[derive(Default)]
pub struct ClientPlugin {
    pub config: ClientConfig,
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        let tick_duration = Duration::from_secs_f64(1.0 / TICK_RATE_HZ as f64);
        app.add_plugins(ClientPlugins { tick_duration });
        app.add_plugins((ProtocolPlugin, PlayerPlugin));

        app.insert_resource(self.config.clone());
        app.init_resource::<InputState>();
        app.init_resource::<LocalPlayerId>();
        app.init_resource::<CameraOrbit>();

        app.add_systems(
            Startup,
            (spawn_camera, spawn_ground, spawn_decorations, setup_client),
        );

        app.add_systems(
            Update,
            (
                handle_welcome,
                attach_player_visuals,
                mark_local_player_visuals,
                gather_input,
                rotate_camera_input,
                retry_connect_when_disconnected,
            ),
        );
        app.add_systems(
            PostUpdate,
            (sync_position_to_transform, follow_local_player).chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_plugin_has_default_config() {
        let plugin = ClientPlugin::default();
        assert_eq!(plugin.config.server_addr, "127.0.0.1:5000");
    }

    #[test]
    fn custom_plugin_config() {
        let plugin = ClientPlugin {
            config: ClientConfig {
                server_addr: "192.168.1.100:8080".into(),
                player_name: "Yume".into(),
                ..Default::default()
            },
        };
        assert_eq!(plugin.config.server_addr, "192.168.1.100:8080");
    }

    #[test]
    fn client_config_defaults() {
        let cfg = ClientConfig::default();
        assert_eq!(cfg.server_addr, "127.0.0.1:5000");
        assert_eq!(cfg.player_name, "Player");
    }
}
