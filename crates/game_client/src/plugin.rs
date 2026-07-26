use bevy::prelude::*;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_inspector_egui::bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use core::time::Duration;
use game_core::constants::TICK_RATE_HZ;
use game_protocol::ProtocolPlugin;
use lightyear::prelude::client::ClientPlugins;
use player::PlayerPlugin;

use crate::arena::{load_arena_assets, spawn_arena};
use crate::camera::{
    CameraOrbit, follow_local_player, rotate_camera_input, spawn_camera, spawn_ground,
    touch_camera_input, zoom_camera_input,
};
use crate::config::ClientConfig;
use crate::connection::{LocalPlayerId, handle_welcome, retry_connect_when_disconnected};
use crate::debug::{DebugMode, inspector_ui, toggle_debug_mode};
use crate::decorations::spawn_decorations;
use crate::hud::{reconnect_button, spawn_hud, update_hud_status, update_version_text};
use crate::input::{InputState, gather_input};
use crate::menu::{AppFlow, play_button, play_button_hover, spawn_menu};
use crate::touch::{
    TouchDetected, TouchJump, detect_touch, jump_button_input, spawn_touch_ui, touch_ui_visibility,
    update_joystick_ui,
};
use crate::visuals::{
    animate_foxes, attach_player_visuals, load_fox_assets, mark_local_player_visuals,
    setup_fox_animators, sync_position_to_transform,
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
        app.add_plugins((EguiPlugin::default(), DefaultInspectorConfigPlugin));

        app.register_type::<game_protocol::PlayerPosition>();
        app.register_type::<game_protocol::PlayerColor>();
        app.register_type::<player::PlayerName>();

        app.insert_resource(self.config.clone());
        app.init_resource::<InputState>();
        app.init_resource::<LocalPlayerId>();
        app.init_resource::<CameraOrbit>();
        app.init_resource::<AppFlow>();
        app.init_resource::<DebugMode>();
        app.init_resource::<TouchJump>();
        app.init_resource::<TouchDetected>();

        app.add_systems(
            Startup,
            (
                spawn_camera,
                spawn_ground,
                spawn_decorations,
                spawn_hud,
                spawn_menu,
                load_fox_assets,
                load_arena_assets,
                spawn_arena,
                spawn_touch_ui,
            )
                .chain(),
        );

        app.add_systems(
            Update,
            (
                handle_welcome,
                attach_player_visuals,
                setup_fox_animators,
                mark_local_player_visuals,
                gather_input,
                rotate_camera_input,
                zoom_camera_input,
                touch_camera_input,
            ),
        );
        app.add_systems(
            Update,
            (
                retry_connect_when_disconnected,
                update_hud_status,
                update_version_text,
                reconnect_button,
                play_button,
                play_button_hover,
                toggle_debug_mode,
            ),
        );
        app.add_systems(
            Update,
            (
                detect_touch,
                touch_ui_visibility,
                jump_button_input,
                update_joystick_ui,
            ),
        );
        app.add_systems(EguiPrimaryContextPass, inspector_ui);
        app.add_systems(
            PostUpdate,
            (
                sync_position_to_transform,
                animate_foxes,
                follow_local_player,
            )
                .chain(),
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
