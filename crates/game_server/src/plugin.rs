use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::prelude::*;
use core::time::Duration;
use creatures::CreaturePlugin;
use game_protocol::ProtocolPlugin;
use housing::ServerHousingPlugin;
use housing::components::HousingPlayer;
use player::{PlayerPlugin, YumeScheme};
use social::SocialPlugin;

use crate::config::ServerConfig;
use crate::systems::WorldConfigResource;
use crate::systems::tls::{TlsConfig, load_tls_identity_system};
use crate::systems::*;
use resources::systems::{spawn_resource_nodes, tick_resource_respawn};

#[derive(Default)]
pub struct ServerPlugin {
    pub config: ServerConfig,
}

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        let tick_duration = Duration::from_secs_f64(1.0 / self.config.tick_rate as f64);

        app.add_plugins(lightyear::prelude::server::ServerPlugins { tick_duration });

        app.add_plugins((
            PhysicsPlugins::default(),
            TnuaControllerPlugin::<YumeScheme>::new(FixedUpdate),
            TnuaAvian3dPlugin::new(FixedUpdate),
        ));

        app.add_plugins((
            ProtocolPlugin,
            PlayerPlugin,
            CreaturePlugin,
            SocialPlugin,
            ServerHousingPlugin,
        ));

        app.insert_resource(ServerConfigResource(self.config.clone()));
        app.init_resource::<NextPlayerColor>();
        app.init_resource::<WalkConfig>();
        app.init_resource::<PersistenceCoordinator>();

        // Wire TLS identity loading from config/env vars.
        // The config fields take precedence; if both are None the TlsConfig
        // falls back to YUME_TLS_CERT / YUME_TLS_KEY env vars, then self-signed.
        {
            let tls_cert = self
                .config
                .tls_cert_path
                .clone()
                .or_else(|| std::env::var("YUME_TLS_CERT").ok());
            let tls_key = self
                .config
                .tls_key_path
                .clone()
                .or_else(|| std::env::var("YUME_TLS_KEY").ok());
            app.insert_resource(TlsConfig {
                cert_path: tls_cert,
                key_path: tls_key,
                check_interval_ticks: 600,
            });
        }
        load_tls_identity_system(app.world_mut());

        // Load world config from embedded RON
        let world_config_ron = include_str!("../../../assets/world.ron");
        match game_core::world_config::WorldConfig::from_str(world_config_ron) {
            Ok(wc) => {
                app.insert_resource(WorldConfigResource(wc));
            }
            Err(e) => {
                tracing::error!("Failed to parse world.ron: {e}");
            }
        }

        // Spatial interest management at 5 Hz
        app.init_resource::<InterestSettings>();
        app.init_resource::<VisibilityCache>();
        app.add_systems(FixedUpdate, update_spatial_visibility);

        app.add_observer(handle_new_client_link);
        app.add_observer(auth::on_client_connected);
        app.add_observer(attach_housing_player);
        app.add_systems(
            FixedUpdate,
            auth::handle_identity_hello.in_set(ServerSystems),
        );

        app.add_systems(FixedUpdate, apply_client_input.in_set(ServerSystems));

        app.configure_sets(FixedUpdate, player::PlayerMovementSet.after(ServerSystems));

        app.add_systems(
            FixedUpdate,
            (
                sync_transform_to_position,
                creatures::sync_creature_position,
            )
                .after(PhysicsSystems::Writeback),
        );

        // Resource node collection, cooldown, persistence commit, and player component initialization
        app.add_systems(
            FixedUpdate,
            (
                handle_action_intent,
                initialize_player_components,
                tick_player_cooldowns,
                tick_resource_respawn,
                process_pending_transactions,
            )
                .in_set(ServerSystems),
        );

        // Spawn resource nodes after world is set up
        app.add_systems(
            PostStartup,
            (
                setup_server,
                setup_world,
                |commands: Commands, config: Res<WorldConfigResource>| {
                    spawn_resource_nodes(commands, &config.0);
                },
            ),
        );
    }
}

/// Observer: attach `HousingPlayer` to client link entities when they
/// receive a `ClientPlayer` component (i.e., after successful auth).
fn attach_housing_player(
    trigger: On<Add, ClientPlayer>,
    mut commands: Commands,
    query: Query<&ClientPlayer>,
) {
    let Ok(cp) = query.get(trigger.entity) else {
        return;
    };
    commands.entity(trigger.entity).insert(HousingPlayer {
        player_entity: cp.player_entity,
        player_id: cp.player_id,
    });
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
        assert_eq!(plugin.config.host, "0.0.0.0");
        assert_eq!(plugin.config.port, 5000);

        let resource = ServerConfigResource(cfg);
        assert_eq!(resource.0.tick_rate, 10);
    }
}
