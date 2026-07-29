use super::*;
use crate::flow::LoadingRoot;
use bevy::asset::AssetPlugin;
use bevy::tasks::IoTaskPool;

fn ensure_io_pool() {
    IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
}

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

#[test]
fn loading_root_is_removed_when_production_flow_leaves_loading() {
    ensure_io_pool();

    // Given: a real Bevy app with the production loading flow configured.
    let mut app = App::new();
    app.add_plugins((AssetPlugin::default(), bevy::state::app::StatesPlugin));
    app.init_asset::<bevy::world_serialization::WorldAsset>();
    app.init_asset::<AnimationGraph>();
    app.init_state::<AppFlow>();
    configure_loading_flow(&mut app);

    // When: the app starts and enters Loading.
    app.update();

    // Then: exactly one LoadingRoot exists.
    let mut roots = app
        .world_mut()
        .query_filtered::<Entity, With<LoadingRoot>>();
    assert_eq!(
        roots.iter(app.world()).count(),
        1,
        "loading UI spawned in Loading state"
    );

    // When: we transition to Menu.
    app.world_mut()
        .resource_mut::<NextState<AppFlow>>()
        .set(AppFlow::Menu);
    app.update(); // OnExit(Loading) despawns LoadingRoot, OnEnter(Menu) runs
    app.update(); // State settles in Menu

    // Then: the LoadingRoot is despawned (no stale root underneath).
    let mut roots = app
        .world_mut()
        .query_filtered::<Entity, With<LoadingRoot>>();
    assert_eq!(
        roots.iter(app.world()).count(),
        0,
        "loading UI despawned after leaving Loading"
    );
}
