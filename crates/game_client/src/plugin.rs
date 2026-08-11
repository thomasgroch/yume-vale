use bevy::prelude::*;
#[cfg(feature = "inspector")]
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
#[cfg(feature = "inspector")]
use bevy_inspector_egui::bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use core::time::Duration;
use game_core::constants::TICK_RATE_HZ;
use game_protocol::ProtocolPlugin;
use lightyear::prelude::client::ClientPlugins;
use player::PlayerPlugin;

use crate::arena::spawn_arena;
use crate::camera::{
    CameraOrbit, follow_local_player, rotate_camera_input, spawn_camera, spawn_ground,
    touch_camera_input, zoom_camera_input,
};
use crate::config::ClientConfig;
use crate::connection::{
    IdentityToken, LocalPlayerId, PageLifecycle, TransportState, handle_connection_rejected,
    handle_page_visibility, handle_transport_fallback, handle_welcome, install_visibility_listener,
    load_identity_token, retry_connect_when_disconnected, send_identity_hello,
};
#[cfg(feature = "inspector")]
use crate::debug::{DebugMode, inspector_ui, toggle_debug_mode};
use crate::decorations::spawn_decorations;
use crate::flow::{self, AppFlow};
use crate::graphics::{
    GraphicsQuality, apply_graphics_quality, graphics_toggle_button, graphics_toggle_hover,
};
use crate::hud::{
    ClientActionFeedback, ClientCooldown, ClientInventory, clear_action_feedback,
    dismiss_rejection_modal, open_commit_link, receive_action_rejected, receive_inventory_snapshot,
    reconnect_button, spawn_hud, spawn_inventory_panel, sync_rejection_modal, tick_cooldown,
    toggle_gameplay_panels, update_hud_status, update_inventory_panel, update_version_text,
};
use crate::input::{InputState, gather_input};
use crate::loading;
use crate::menu::{play_button, play_button_hover, spawn_menu};
use crate::touch::{
    TouchDetected, TouchJump, detect_touch, jump_button_input, spawn_touch_ui, touch_ui_visibility,
    update_joystick_ui,
};
use crate::ui::{
    focus::{FocusState, clear_stale_focus, manage_focus},
    roster::{spawn_roster_panel, update_roster_panel},
    social::{ClientEmote, receive_emote_broadcast},
};
use crate::visuals::{
    BuildMode, ClientBonds, animate_foxes, attach_creature_visuals, attach_decoration_visuals,
    attach_player_visuals, build_controls_ui, handle_action_rejected, handle_bond_snapshot,
    mark_local_player_visuals, send_wave_emote, setup_fox_animators, show_bond_display,
    show_feed_prompt, spawn_plot_boundaries, sync_position_to_transform, toggle_build_mode,
    trigger_wave_from_emote, update_plot_owner_indicators,
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
        #[cfg(feature = "inspector")]
        app.add_plugins((EguiPlugin::default(), DefaultInspectorConfigPlugin));

        app.register_type::<game_protocol::PlayerPosition>();
        app.register_type::<game_protocol::PlayerColor>();
        app.register_type::<player::PlayerName>();

        app.insert_resource(self.config.clone());
        app.init_resource::<InputState>();
        app.init_resource::<GraphicsQuality>();
        app.init_resource::<LocalPlayerId>();
        app.init_resource::<CameraOrbit>();
        app.init_resource::<BuildMode>();
        app.init_resource::<ClientBonds>();
        app.init_resource::<ClientInventory>();
        app.init_resource::<ClientCooldown>();
        app.init_resource::<ClientActionFeedback>();
        app.init_resource::<ClientEmote>();

        // Identity persistence
        {
            let stored = load_identity_token().unwrap_or_default();
            app.insert_resource(IdentityToken(stored));
        }
        app.insert_resource(TransportState::detect());
        #[cfg(feature = "inspector")]
        app.init_resource::<DebugMode>();
        app.init_resource::<FocusState>();
        app.init_resource::<TouchJump>();
        app.init_resource::<TouchDetected>();
        app.init_resource::<PageLifecycle>();

        // ── State machine ─────────────────────────────────────────────────
        app.init_state::<AppFlow>();
        configure_loading_flow(app);

        // Menu: show title screen and spawn arena
        app.add_systems(OnEnter(AppFlow::Menu), (spawn_menu, spawn_arena));
        app.add_systems(OnExit(AppFlow::Menu), flow::despawn_menu);

        // InGame: systems that run during gameplay
        app.add_systems(OnExit(AppFlow::InGame), flow::despawn_ingame);

        // Spawn persistent entities at startup (camera, world)
        app.add_systems(
            Startup,
            (
                crate::fonts::install_default_font,
                spawn_camera,
                spawn_ground,
                spawn_decorations,
                spawn_hud,
                spawn_touch_ui,
                spawn_plot_boundaries,
                spawn_inventory_panel,
                spawn_roster_panel,
            )
                .chain(),
        );
        app.add_systems(Startup, install_visibility_listener);

        // Update systems (always running, some self-gate on state)
        app.add_systems(
            Update,
            (
                handle_welcome,
                handle_connection_rejected,
                send_identity_hello,
                attach_player_visuals,
                setup_fox_animators,
                mark_local_player_visuals,
                attach_creature_visuals,
                attach_decoration_visuals,
                toggle_build_mode,
                send_wave_emote,
                gather_input,
                rotate_camera_input,
                zoom_camera_input,
                touch_camera_input,
            ),
        );
        app.add_systems(
            Update,
            (
                sync_rejection_modal.after(handle_connection_rejected),
                dismiss_rejection_modal,
            ),
        );
        // Snapshot receivers (Lightyear MessageReceiver systems)
        app.add_systems(
            Update,
            (
                handle_bond_snapshot,
                handle_action_rejected,
                receive_inventory_snapshot,
                receive_action_rejected,
                receive_emote_broadcast,
                tick_cooldown,
                clear_action_feedback,
                trigger_wave_from_emote,
                toggle_gameplay_panels,
            ),
        );
        app.add_systems(
            Update,
            (
                retry_connect_when_disconnected.after(handle_page_visibility),
                handle_transport_fallback,
                update_hud_status,
                update_version_text,
                open_commit_link,
                reconnect_button,
                play_button,
                play_button_hover,
                build_controls_ui,
                show_bond_display,
                show_feed_prompt,
                update_plot_owner_indicators,
            )
                .run_if(in_state(AppFlow::Menu).or_else(in_state(AppFlow::InGame))),
        );
        // Graphics quality toggle: flip state, then push it onto the render
        // world — deterministic toggle → apply ordering, only while the menu
        // is active (the preset is chosen before pressing Jogar).
        app.add_systems(
            Update,
            (
                graphics_toggle_button,
                apply_graphics_quality,
                graphics_toggle_hover,
            )
                .chain()
                .run_if(in_state(AppFlow::Menu)),
        );
        #[cfg(feature = "inspector")]
        app.add_systems(
            Update,
            toggle_debug_mode.run_if(in_state(AppFlow::Menu).or_else(in_state(AppFlow::InGame))),
        );
        app.add_systems(
            Update,
            (update_inventory_panel, update_roster_panel).run_if(in_state(AppFlow::InGame)),
        );
        app.add_systems(
            Update,
            (
                handle_page_visibility,
                detect_touch,
                touch_ui_visibility,
                jump_button_input,
                update_joystick_ui,
                manage_focus,
                clear_stale_focus,
            ),
        );
        #[cfg(feature = "inspector")]
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

fn configure_loading_flow(app: &mut App) {
    app.add_systems(
        OnEnter(AppFlow::Loading),
        (
            flow::load_world_config,
            loading::create_loading_queue,
            flow::spawn_loading_ui,
        )
            .chain(),
    );
    app.add_systems(
        Update,
        (loading::poll_and_finalize, flow::update_loading_progress)
            .run_if(in_state(AppFlow::Loading)),
    );
    app.add_systems(OnExit(AppFlow::Loading), flow::despawn_loading_ui);
}
#[cfg(test)]
mod tests;
