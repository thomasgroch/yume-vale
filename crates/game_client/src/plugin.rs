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
use crate::connection::{
    IdentityToken, LocalPlayerId, TransportState, handle_transport_fallback, handle_welcome,
    load_identity_token, retry_connect_when_disconnected, send_identity_hello,
};
use crate::debug::{DebugMode, inspector_ui, toggle_debug_mode};
use crate::decorations::spawn_decorations;
use crate::flow::{self, AppFlow};
use crate::hud::{
    ClientActionFeedback, ClientCooldown, ClientInventory, ClientQuests, clear_action_feedback,
    receive_action_rejected, receive_inventory_snapshot, receive_quest_snapshot, reconnect_button,
    spawn_hud, spawn_inventory_panel, spawn_quest_panel, tick_cooldown, toggle_gameplay_panels,
    update_hud_status, update_inventory_panel, update_quest_panel, update_version_text,
};
use crate::input::{InputState, gather_input};
use crate::menu::{play_button, play_button_hover, spawn_menu};
use crate::prediction::{
    InputHistory, LastProcessedTick, mark_local_predicted, predict_movement, reconcile_on_ack,
};
use crate::touch::{
    TouchDetected, TouchJump, detect_touch, jump_button_input, spawn_touch_ui, touch_ui_visibility,
    update_joystick_ui,
};
use crate::ui::{
    chat::{
        ChatInputState, process_chat_input, spawn_chat_panel, toggle_chat_focus, update_chat_panel,
    },
    focus::{FocusState, clear_stale_focus, manage_focus},
    roster::{
        handle_accept_button, handle_decline_button, handle_leave_button, spawn_roster_panel,
        update_roster_panel,
    },
    social::{
        ClientChat, ClientEmote, ClientGroup, receive_chat_received, receive_emote_broadcast,
        receive_group_update,
    },
};
use crate::visuals::{
    BuildMode, ClientBonds, animate_foxes, attach_creature_visuals, attach_decoration_visuals,
    attach_player_visuals, build_controls_ui, handle_action_rejected, handle_bond_snapshot,
    load_creature_assets, load_fox_assets, mark_local_player_visuals, send_wave_emote,
    setup_fox_animators, show_bond_display, show_feed_prompt, spawn_plot_boundaries,
    sync_position_to_transform, toggle_build_mode, trigger_wave_from_emote,
    update_plot_owner_indicators,
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
        app.init_resource::<InputHistory>();
        app.init_resource::<LastProcessedTick>();
        app.init_resource::<LocalPlayerId>();
        app.init_resource::<CameraOrbit>();
        app.init_resource::<BuildMode>();
        app.init_resource::<ClientBonds>();
        app.init_resource::<ClientInventory>();
        app.init_resource::<ClientQuests>();
        app.init_resource::<ClientCooldown>();
        app.init_resource::<ClientActionFeedback>();
        app.init_resource::<ClientChat>();
        app.init_resource::<ClientGroup>();
        app.init_resource::<ClientEmote>();
        app.init_resource::<ChatInputState>();

        // Identity persistence
        {
            let stored = load_identity_token().unwrap_or_default();
            app.insert_resource(IdentityToken(stored));
        }
        app.insert_resource(TransportState::detect());
        app.init_resource::<DebugMode>();
        app.init_resource::<FocusState>();
        app.init_resource::<TouchJump>();
        app.init_resource::<TouchDetected>();

        // ── State machine ─────────────────────────────────────────────────
        app.init_state::<AppFlow>();

        // Loading: parse config, load assets
        app.add_systems(
            OnEnter(AppFlow::Loading),
            (
                flow::load_world_config,
                flow::load_game_assets,
                load_fox_assets,
                load_creature_assets,
                load_arena_assets,
            ),
        );
        app.add_systems(
            Update,
            (flow::check_assets_loaded, flow::update_loading_progress)
                .run_if(in_state(AppFlow::Loading)),
        );

        // Menu: show title screen
        app.add_systems(OnEnter(AppFlow::Menu), spawn_menu);
        app.add_systems(OnExit(AppFlow::Menu), flow::despawn_menu);

        // InGame: systems that run during gameplay
        app.add_systems(OnExit(AppFlow::InGame), flow::despawn_ingame);

        // Spawn persistent entities at startup (loading screen, camera, world)
        app.add_systems(
            Startup,
            (
                flow::spawn_loading_ui,
                spawn_camera,
                spawn_ground,
                spawn_decorations,
                spawn_hud,
                spawn_arena,
                spawn_touch_ui,
                spawn_plot_boundaries,
                spawn_inventory_panel,
                spawn_quest_panel,
                spawn_chat_panel,
                spawn_roster_panel,
            )
                .chain(),
        );

        // Update systems (always running, some self-gate on state)
        app.add_systems(
            Update,
            (
                handle_welcome,
                send_identity_hello,
                attach_player_visuals,
                setup_fox_animators,
                mark_local_player_visuals,
                mark_local_predicted,
                attach_creature_visuals,
                attach_decoration_visuals,
                toggle_build_mode,
                toggle_chat_focus,
                process_chat_input,
                send_wave_emote,
                gather_input,
                predict_movement,
                rotate_camera_input,
                zoom_camera_input,
                touch_camera_input,
            ),
        );
        // Snapshot receivers (Lightyear MessageReceiver systems)
        app.add_systems(
            Update,
            (
                handle_bond_snapshot,
                handle_action_rejected,
                receive_inventory_snapshot,
                receive_quest_snapshot,
                receive_action_rejected,
                receive_chat_received,
                receive_group_update,
                receive_emote_broadcast,
                reconcile_on_ack,
                tick_cooldown,
                clear_action_feedback,
                trigger_wave_from_emote,
                toggle_gameplay_panels,
            ),
        );
        app.add_systems(
            Update,
            (
                retry_connect_when_disconnected,
                handle_transport_fallback,
                update_hud_status,
                update_version_text,
                reconnect_button,
                play_button,
                play_button_hover,
                toggle_debug_mode,
                build_controls_ui,
                show_bond_display,
                show_feed_prompt,
                update_plot_owner_indicators,
            )
                .run_if(in_state(AppFlow::Menu).or_else(in_state(AppFlow::InGame))),
        );
        app.add_systems(
            Update,
            (
                update_inventory_panel,
                update_quest_panel,
                update_chat_panel,
                update_roster_panel,
                handle_accept_button,
                handle_decline_button,
                handle_leave_button,
            )
                .run_if(in_state(AppFlow::InGame)),
        );
        app.add_systems(
            Update,
            (
                detect_touch,
                touch_ui_visibility,
                jump_button_input,
                update_joystick_ui,
                manage_focus,
                clear_stale_focus,
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
