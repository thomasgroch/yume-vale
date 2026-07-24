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
    use bevy::prelude::*;
    use game_protocol::ProtocolPlugin;
    use player::PlayerPlugin;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    // Mirror production plugin order (ServerPlugins first): ProtocolPlugin's
    // replicate() calls need resources from lightyear's replicon backend.
    app.add_plugins(lightyear::prelude::server::ServerPlugins {
        tick_duration: core::time::Duration::from_secs_f64(1.0 / 30.0),
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin));
    app.init_resource::<systems::NextPlayerColor>();
    app.init_resource::<systems::WalkConfig>();
    app.add_systems(
        FixedUpdate,
        systems::apply_client_input.in_set(systems::ServerSystems),
    );
    app
}
