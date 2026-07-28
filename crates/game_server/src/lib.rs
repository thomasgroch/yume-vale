#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::map_identity
)]
pub mod config;
pub mod plugin;
pub mod systems;

pub use config::*;
pub use plugin::*;
pub use systems::*;

/// Build a minimal `App` configured with protocol and player plugins, time,
/// and core server systems, suitable for testing server logic without starting
/// actual network IO.
#[cfg(test)]
pub fn build_test_app() -> bevy::prelude::App {
    use crate::systems::persistence::PersistenceCoordinator;
    use crate::systems::setup::WorldConfigResource;
    use bevy::prelude::*;
    use creatures::CreaturePlugin;
    use game_core::id::CreatureId;
    use game_core::resources::ResourceKind;
    use game_core::world_config::{CreatureConfig, CreatureKind, WorldConfig};
    use game_protocol::ProtocolPlugin;
    use player::PlayerPlugin;
    use quests::QuestPlugin;

    use social::SocialPlugin;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    // Mirror production plugin order (ServerPlugins first): ProtocolPlugin's
    // replicate() calls need resources from lightyear's replicon backend.
    app.add_plugins(lightyear::prelude::server::ServerPlugins {
        tick_duration: core::time::Duration::from_secs_f64(1.0 / 30.0),
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin, CreaturePlugin, SocialPlugin));
    app.init_resource::<systems::NextPlayerColor>();
    app.init_resource::<systems::WalkConfig>();
    app.init_resource::<PersistenceCoordinator>();
    // World config resource with test creatures
    app.insert_resource(WorldConfigResource(WorldConfig {
        resources: vec![],
        creatures: vec![
            CreatureConfig {
                id: CreatureId::new(1),
                kind: CreatureKind::Fluffball,
                center: Vec3::new(10.0, 0.0, 5.0),
                wander_radius: 8.0,
                food_kind: ResourceKind::Berry,
                model_path: "fluff.glb".into(),
            },
            CreatureConfig {
                id: CreatureId::new(2),
                kind: CreatureKind::Glimmerwing,
                center: Vec3::new(-5.0, 0.0, 15.0),
                wander_radius: 6.0,
                food_kind: ResourceKind::Crystal,
                model_path: "glim.glb".into(),
            },
        ],
        quests: vec![],
    }));
    // Add quest plugin with empty definitions (tests can override via app.world_mut())
    app.add_plugins(QuestPlugin { quests: vec![] });
    // Spatial interest management
    app.init_resource::<systems::InterestSettings>();
    app.init_resource::<systems::VisibilityCache>();
    app.add_systems(FixedUpdate, systems::update_spatial_visibility);

    // Auth: pending session on connect, process IdentityHello in FixedUpdate
    app.add_observer(systems::auth::on_client_connected);
    app.add_systems(
        FixedUpdate,
        (
            systems::auth::handle_identity_hello,
            systems::apply_client_input,
            systems::handle_action_intent,
            systems::initialize_player_components,
            systems::tick_player_cooldowns,
            systems::process_pending_transactions,
        )
            .in_set(systems::ServerSystems),
    );
    // Quest systems (event handler is an observer registered in the plugin)
    app.add_systems(
        FixedUpdate,
        (
            quests::initialize_player_quests,
            quests::persist_quest_progress,
        )
            .chain()
            .in_set(systems::ServerSystems),
    );
    // Resource node spawning (after world config is set)
    app.add_systems(
        PostStartup,
        |commands: Commands, config: Res<WorldConfigResource>| {
            resources::systems::spawn_resource_nodes(commands, &config.0);
        },
    );
    // Resource respawn tick
    app.add_systems(
        FixedUpdate,
        resources::systems::tick_resource_respawn.in_set(systems::ServerSystems),
    );
    app
}
