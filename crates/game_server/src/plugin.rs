use bevy::prelude::*;
use core::time::Duration;
use game_protocol::ProtocolPlugin;
use player::PlayerPlugin;

use crate::config::ServerConfig;
use crate::systems::*;

#[derive(Default)]
pub struct ServerPlugin {
    pub config: ServerConfig,
}

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        let tick_duration = Duration::from_secs_f64(1.0 / self.config.tick_rate as f64);

        app.add_plugins(lightyear::prelude::server::ServerPlugins { tick_duration });

        app.add_plugins((ProtocolPlugin, PlayerPlugin));

        app.insert_resource(ServerConfigResource(self.config.clone()));
        app.init_resource::<NextPlayerColor>();

        app.add_observer(handle_new_client_link);
        app.add_observer(on_client_connected);

        app.add_systems(FixedUpdate, apply_client_input.in_set(ServerSystems));

        app.configure_sets(FixedUpdate, player::PlayerMovementSet.after(ServerSystems));

        app.add_systems(
            FixedUpdate,
            sync_transform_to_position.after(player::integrate_velocity),
        );

        app.add_systems(PostStartup, setup_server);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_plugin_has_correct_tick_rate() {
        let cfg = ServerConfig {
            tick_rate: 10,
            ..Default::default()
        };
        let plugin = ServerPlugin {
            config: cfg.clone(),
        };
        assert_eq!(plugin.config.tick_rate, 10);
        assert_eq!(plugin.config.host, "127.0.0.1");
        assert_eq!(plugin.config.port, 5000);

        let resource = ServerConfigResource(cfg);
        assert_eq!(resource.0.tick_rate, 10);
    }
}
