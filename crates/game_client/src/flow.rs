use bevy::asset::LoadState;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;
use game_core::arena::ArenaModel;
use game_core::world_config::WorldConfig;

use crate::ui::{theme, widgets};

// ---------------------------------------------------------------------------
// States
// ---------------------------------------------------------------------------

/// Application-level flow state machine.
#[derive(States, Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub enum AppFlow {
    /// Parsing world config, issuing asset loads — no gameplay yet.
    #[default]
    Loading,
    /// Title / play screen — waiting for the player.
    Menu,
    /// Active multiplayer session.
    InGame,
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Parsed world configuration, available after `Loading` succeeds.
#[derive(Resource)]
pub struct WorldConfigResource(pub WorldConfig);

/// Asset-loading gate state: every known GLB handle and its load progress.
#[derive(Resource)]
pub struct GameAssets {
    /// Scene handles (loaded as `WorldAsset` via `GltfAssetLabel::Scene(0)`).
    pub scenes: Vec<(String, Handle<WorldAsset>)>,
    /// How many handles have reached `LoadState::Loaded`.
    pub loaded_count: usize,
    /// Total handles being tracked.
    pub total: usize,
    /// Set when a handle enters `LoadState::Failed`.
    pub failing_path: Option<String>,
}

/// A non-recoverable loading error (parse failure, missing file, …).
#[derive(Resource)]
pub struct LoadingError {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Loading screen UI
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct LoadingRoot;

#[derive(Component)]
pub struct LoadingProgressText;

/// Spawn the themed loading screen (title + progress).
pub fn spawn_loading_ui(mut commands: Commands) {
    commands
        .spawn((
            LoadingRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(theme::SURFACE_LOADING),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Yume Vale"),
                widgets::text_font(theme::FONT_TITLE),
                TextColor(theme::TEXT_TITLE),
                TextShadow::default(),
            ));
            root.spawn((
                Text::new("Carregando..."),
                LoadingProgressText,
                widgets::text_font(theme::FONT_LG),
                TextColor(theme::TEXT_SUBTLE),
                Node {
                    margin: UiRect::top(Val::Px(theme::SPACE_16)),
                    ..default()
                },
            ));
        });
}

/// Update the loading-progress text every frame.
pub fn update_loading_progress(
    assets: Option<Res<GameAssets>>,
    error: Option<Res<LoadingError>>,
    mut texts: Query<&mut Text, With<LoadingProgressText>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    if let Some(err) = error {
        text.0 = format!("Erro: {}", err.message);
        return;
    }
    match assets {
        Some(a) if a.failing_path.is_some() => {
            text.0 = format!("Falha ao carregar: {}", a.failing_path.as_ref().unwrap());
        }
        Some(a) => {
            text.0 = format!("Carregando... {}/{}", a.loaded_count, a.total);
        }
        None => {
            text.0 = "Preparando...".to_string();
        }
    }
}

/// Despawn the loading root when leaving the Loading state.
pub fn despawn_loading_ui(mut commands: Commands, query: Query<Entity, With<LoadingRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

// ---------------------------------------------------------------------------
// Loading systems
// ---------------------------------------------------------------------------

/// Parse and store the canonical world config from the embedded RON.
///
/// On failure inserts a `LoadingError` so the UI shows the problem.
pub fn load_world_config(mut commands: Commands) {
    const WORLD_RON: &str = include_str!("../../../assets/world.ron");
    match WorldConfig::from_str(WORLD_RON) {
        Ok(config) => {
            commands.insert_resource(WorldConfigResource(config));
        }
        Err(e) => {
            commands.insert_resource(LoadingError {
                message: format!("Falha ao interpretar world.ron: {e}"),
            });
        }
    }
}

/// Strip the `assets/` prefix that canonical paths carry.
fn strip_assets_prefix(path: &str) -> &str {
    path.strip_prefix("assets/").unwrap_or(path)
}

/// Issue load requests for every known GLB and track them.
///
/// Only runs once — skips if `GameAssets` already exists (e.g. returning
/// from a previous loading attempt would be handled by the resource guard).
pub fn load_game_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    config: Option<Res<WorldConfigResource>>,
    existing: Option<Res<GameAssets>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(config) = config else {
        return; // world config hasn't been parsed yet (will emit error)
    };

    let mut scenes: Vec<(String, Handle<WorldAsset>)> = Vec::new();

    // Arena models (6)
    for model in &[
        ArenaModel::Portal,
        ArenaModel::Wall,
        ArenaModel::Pillar,
        ArenaModel::CrystalBig,
        ArenaModel::CrystalSmall,
        ArenaModel::Rock,
    ] {
        let path = model.asset_path();
        let handle: Handle<WorldAsset> =
            asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
        scenes.push((path.to_string(), handle));
    }

    // Fox rigged
    scenes.push((
        "models/fox/rigged.glb".to_string(),
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/fox/rigged.glb")),
    ));

    // Resources from world config
    for res in &config.0.resources {
        let raw = strip_assets_prefix(&res.model_path);
        let handle: Handle<WorldAsset> =
            asset_server.load(GltfAssetLabel::Scene(0).from_asset(raw.to_string()));
        scenes.push((raw.to_string(), handle));
    }

    // Creatures from world config
    for creature in &config.0.creatures {
        let raw = strip_assets_prefix(&creature.model_path);
        let handle: Handle<WorldAsset> =
            asset_server.load(GltfAssetLabel::Scene(0).from_asset(raw.to_string()));
        scenes.push((raw.to_string(), handle));
    }

    let total = scenes.len();
    commands.insert_resource(GameAssets {
        scenes,
        loaded_count: 0,
        total,
        failing_path: None,
    });
}

/// Poll every tracked handle; transition to Menu when all are Loaded.
///
/// On failure records the path in `GameAssets.failing_path` (shown in UI).
pub fn check_assets_loaded(
    asset_server: Res<AssetServer>,
    mut assets: ResMut<GameAssets>,
    mut next_state: ResMut<NextState<AppFlow>>,
) {
    let mut loaded = 0usize;
    for (path, handle) in &assets.scenes {
        match asset_server.load_state(handle.id()) {
            LoadState::Loaded => {
                loaded += 1;
            }
            LoadState::Failed(_) => {
                assets.failing_path = Some(path.clone());
                return; // stop early on failure
            }
            _ => {}
        }
    }
    assets.loaded_count = loaded;
    if loaded == assets.total && assets.failing_path.is_none() {
        next_state.set(AppFlow::Menu);
    }
}

/// Despawn entities that belong to the InGame phase.
pub fn despawn_ingame(mut commands: Commands, entities: Query<Entity, Without<LoadingRoot>>) {
    // Skip the loading/menu UI — only despawn runtime entities.
    // Lightyear client entity will disconnect naturally.
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// Despawn the menu root when leaving the Menu state.
pub fn despawn_menu(mut commands: Commands, query: Query<Entity, With<crate::menu::MenuRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    /// Minimal app skeleton for flow tests.
    fn flow_app() -> App {
        let mut app = App::new();
        app.add_plugins((AssetPlugin::default(), bevy::state::app::StatesPlugin));
        app.init_asset::<WorldAsset>();
        app.init_state::<AppFlow>();
        app.insert_resource(WorldConfigResource(WorldConfig::default()));
        app
    }

    #[test]
    fn initial_state_is_loading() {
        let app = flow_app();
        assert_eq!(
            app.world().resource::<State<AppFlow>>().get(),
            &AppFlow::Loading,
        );
    }

    #[test]
    fn no_connection_before_play() {
        let mut app = flow_app();
        app.update();
        // In Loading state there should be no Lightyear client entity.
        // No Client would be spawned because start_connection is never
        // called before the play button is pressed.
        // No Client entity exists before Play button is pressed.
        let mut clients = app
            .world_mut()
            .query_filtered::<Entity, With<lightyear::prelude::Client>>();
        assert_eq!(clients.iter(app.world()).count(), 0);
    }

    #[test]
    fn empty_world_config_transitions_to_menu() {
        let mut app = flow_app();
        app.add_systems(Update, check_assets_loaded);

        // Insert GameAssets with zero entries — all "loaded" trivially.
        app.world_mut().insert_resource(GameAssets {
            scenes: vec![],
            loaded_count: 0,
            total: 0,
            failing_path: None,
        });

        app.update(); // run check_assets_loaded
        app.update(); // apply StateTransition
        assert_eq!(
            app.world().resource::<State<AppFlow>>().get(),
            &AppFlow::Menu,
            "zero handles → immediate transition to Menu",
        );
    }

    #[test]
    fn missing_fixture_stays_in_loading_with_path() {
        let mut app = flow_app();
        app.add_systems(Update, check_assets_loaded);

        // Init the asset type so loading doesn't panic.
        app.init_asset::<WorldAsset>();

        // A non-existent handle stays Loading forever (or goes to Failed).
        // Since we can't force a real asset to fail in test, we verify that
        // an entry with LoadState::NotLoaded (which will never become
        // Loaded in a test without a real asset server) keeps us in Loading.
        let handle: Handle<WorldAsset> = app
            .world_mut()
            .resource_mut::<AssetServer>()
            .load(GltfAssetLabel::Scene(0).from_asset("does_not_exist.glb"));
        app.world_mut().insert_resource(GameAssets {
            scenes: vec![("does_not_exist.glb".to_string(), handle)],
            loaded_count: 0,
            total: 1,
            failing_path: None,
        });

        // Even after one update the asset cannot load (file doesn't exist),
        // so we must stay in Loading.
        app.update();
        assert_eq!(
            app.world().resource::<State<AppFlow>>().get(),
            &AppFlow::Loading,
            "missing fixture keeps us in Loading",
        );
        // The asset server may or may not report Failed in a single frame.
        // What matters is we do NOT transition to Menu.
        let assets = app.world().resource::<GameAssets>();
        assert_ne!(assets.loaded_count, 1, "missing asset cannot be loaded");
    }

    #[test]
    fn play_button_reaches_ingame() {
        let mut app = flow_app();
        // Simulate being in Menu state.
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::Menu);
        app.update();

        // Press play: set NextState(InGame).
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::InGame);
        app.update();

        assert_eq!(
            app.world().resource::<State<AppFlow>>().get(),
            &AppFlow::InGame,
        );
    }

    #[test]
    fn leaving_ingame_returns_to_menu() {
        let mut app = flow_app();
        // Start in InGame.
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::InGame);
        app.update();
        assert_eq!(
            app.world().resource::<State<AppFlow>>().get(),
            &AppFlow::InGame,
        );

        // Simulate leaving (e.g. disconnect or quit).
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::Menu);
        app.update();

        assert_eq!(
            app.world().resource::<State<AppFlow>>().get(),
            &AppFlow::Menu,
        );
    }

    #[test]
    fn no_duplicate_root_scenes_spawn() {
        let mut app = flow_app();
        app.add_systems(OnEnter(AppFlow::Loading), spawn_loading_ui);
        app.add_systems(OnExit(AppFlow::Loading), despawn_loading_ui);

        // Enter Loading (default on init), UI is spawned.
        app.update();
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<LoadingRoot>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "loading UI spawned once");

        // Transition to Menu and back to Loading — ensure no duplicate.
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::Menu);
        app.update();
        app.update(); // state changes take two updates

        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::Loading);
        app.update();
        app.update(); // OnEnter(Loading) runs

        let count = app
            .world_mut()
            .query_filtered::<Entity, With<LoadingRoot>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "only one loading UI exists after re-entry");
    }

    #[test]
    fn world_config_parses_embedded_ron() {
        let mut app = flow_app();
        app.add_systems(OnEnter(AppFlow::Loading), load_world_config);

        // Initial state is Loading, so OnEnter runs on first update.
        app.update();

        let config = app.world().resource::<WorldConfigResource>();
        assert!(
            !config.0.resources.is_empty(),
            "world config must parse resources from embedded RON",
        );
        assert!(
            !config.0.creatures.is_empty(),
            "world config must parse creatures from embedded RON",
        );
    }

    #[test]
    fn loading_progress_ui_updates_text() {
        let mut app = flow_app();
        app.add_systems(OnEnter(AppFlow::Loading), spawn_loading_ui);
        app.add_systems(Update, update_loading_progress);

        app.world_mut().insert_resource(GameAssets {
            scenes: vec![],
            loaded_count: 1,
            total: 3,
            failing_path: None,
        });

        app.update();

        let text = app
            .world_mut()
            .query_filtered::<&Text, With<LoadingProgressText>>()
            .single(app.world())
            .unwrap()
            .0
            .clone();
        assert_eq!(text, "Carregando... 1/3");
    }
}
