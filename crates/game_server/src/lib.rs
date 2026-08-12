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

/// Build a minimal `App` running the exact same [`GameLogicPlugin`] as
/// production (see plugin.rs), suitable for testing server logic without
/// starting actual network IO. Using the real plugin instead of a
/// hand-maintained system list is deliberate: a second, independently
/// drifting list is how three gameplay systems previously went missing from
/// production without a single test catching it.
#[cfg(test)]
pub fn build_test_app() -> bevy::prelude::App {
    use crate::systems::setup::WorldConfigResource;
    use bevy::prelude::*;
    use game_core::id::CreatureId;
    use game_core::resources::ResourceKind;
    use game_core::world_config::{CreatureConfig, CreatureKind, WorldConfig};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    // GameLogicPlugin disables avian's PhysicsTransformPlugin (see
    // plugin.rs), so nothing else provides Transform propagation — needed by
    // any test that runs the full schedule via `app.update()`.
    app.add_plugins(bevy::transform::TransformPlugin);
    // Mirror production plugin order (ServerPlugins first): ProtocolPlugin's
    // replicate() calls need resources from lightyear's replicon backend.
    app.add_plugins(lightyear::prelude::server::ServerPlugins {
        tick_duration: core::time::Duration::from_secs_f64(1.0 / 30.0),
    });
    app.add_plugins(plugin::GameLogicPlugin);
    // Override the embedded world config with a small test fixture.
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
    }));
    app
}
