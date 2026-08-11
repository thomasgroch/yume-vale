pub mod arena;
pub mod camera;
pub mod config;
pub mod connection;
#[cfg(feature = "inspector")]
pub mod debug;
pub mod decorations;
pub mod flow;
pub(crate) mod fonts;
pub mod graphics;
pub mod hud;
pub mod input;
pub mod loading;
pub mod menu;
pub mod plugin;
pub mod touch;
pub mod ui;
pub mod visuals;

pub use camera::{CameraOrbit, follow_local_player, rotate_camera_input, spawn_camera};
pub use config::{ClientConfig, build_client_config};
pub use connection::{IdentityToken, LocalPlayerId};
pub use decorations::spawn_decorations;
pub use game_protocol::{PRIVATE_KEY, PROTOCOL_ID};
pub use input::{InputState, gather_input, read_keyboard_input};
pub use plugin::ClientPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn read_keyboard_input_returns_default_with_no_keys() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        let (dir, run, action) = read_keyboard_input(keys, 0.0);
        assert!(dir.is_zero());
        assert!(!run);
        assert!(action.is_none());
    }

    #[test]
    fn config_defaults_are_reasonable() {
        let cfg = ClientConfig::default();
        assert!(!cfg.server_addr.is_empty());
        assert!(!cfg.player_name.is_empty());
    }

    #[test]
    fn local_player_id_default_is_none() {
        let lid = LocalPlayerId::default();
        assert!(lid.id.is_none());
    }
}
