use avian3d::prelude::*;
use bevy::app::PluginGroup;
use bevy::prelude::*;
use core::time::Duration;
use creatures::CreaturePlugin;
use game_protocol::ProtocolPlugin;
use housing::ServerHousingPlugin;
use housing::components::HousingPlayer;
use lightyear::avian3d::plugin::{AvianReplicationMode, LightyearAvianPlugin};
use player::PlayerPlugin;
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

        app.add_plugins(
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsTransformPlugin>()
                .disable::<PhysicsInterpolationPlugin>(),
        );
        app.add_plugins(LightyearAvianPlugin {
            replication_mode: AvianReplicationMode::Position,
            rollback_resources: true,
            ..default()
        });

        app.add_plugins((
            ProtocolPlugin,
            PlayerPlugin,
            CreaturePlugin,
            SocialPlugin,
            ServerHousingPlugin,
            AdminApiPlugin {
                port: self.config.admin_port,
            },
        ));

        app.insert_resource(ServerConfigResource(self.config.clone()));
        app.init_resource::<NextPlayerColor>();
        app.init_resource::<PersistenceCoordinator>();

        // Wire persistence from config/env var.
        // The config field takes precedence; if unset, falls back to
        // YUME_DATABASE_URL. If neither is set, the server runs with
        // ephemeral (non-persistent) player data — see the "no persistence
        // configured" log lines in systems/auth.rs and systems/collect.rs.
        {
            let env_db_url = std::env::var("YUME_DATABASE_URL").ok();
            let db_url =
                persistence::resolve_db_url(self.config.db_url.as_deref(), env_db_url.as_deref());
            match db_url {
                Some(db_url) => match persistence::spawn_persistence(&db_url) {
                    Ok(resource) => {
                        app.insert_resource(resource);
                        tracing::info!("persistence enabled");
                    }
                    Err(e) => {
                        tracing::error!("failed to start persistence: {e}");
                    }
                },
                None => {
                    tracing::info!(
                        "YUME_DATABASE_URL not set — running with ephemeral (non-persistent) player data"
                    );
                }
            }
        }

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
            (
                auth::handle_identity_hello,
                apply_client_input,
                stop_stale_player_input,
            )
                .in_set(ServerSystems),
        );

        app.configure_sets(FixedUpdate, player::PlayerMovementSet.after(ServerSystems));

        app.add_systems(
            FixedUpdate,
            (creatures::sync_creature_position,).after(PhysicsSystems::Writeback),
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
