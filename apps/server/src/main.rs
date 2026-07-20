use bevy::prelude::*;
use game_server::ServerPlugin;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    App::new()
        .add_plugins(
            MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                std::time::Duration::from_secs_f64(1.0 / 30.0),
            )),
        )
        .add_plugins(bevy::state::app::StatesPlugin)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(ServerPlugin::default())
        .run();
}
