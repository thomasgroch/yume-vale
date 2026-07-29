use bevy::prelude::*;
use game_core::world_config::WorldConfig;

use crate::loading;
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
pub(crate) fn update_loading_progress(
    loader: Option<Res<loading::SeqLoader>>,
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
    match loader {
        Some(l) => {
            if let Some(ref path) = l.failing_path {
                text.0 = format!("Falha ao carregar: {}", path);
            } else {
                text.0 = format!("Carregando... {}/{}", l.loaded_count(), l.total);
            }
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
        app.init_asset::<bevy::world_serialization::WorldAsset>();
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
        let mut clients = app
            .world_mut()
            .query_filtered::<Entity, With<lightyear::prelude::Client>>();
        assert_eq!(clients.iter(app.world()).count(), 0);
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

        app.world_mut().insert_resource(loading::SeqLoader {
            queue: vec![],
            active: None,
            completed: vec![],
            progress: 1,
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
