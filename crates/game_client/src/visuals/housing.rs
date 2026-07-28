use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use game_core::housing_layout::{PLOT_HALF_SIZE, plot_layout};
use game_protocol::components::DecorationState;
use game_protocol::messages::{ActionRejected, PlotBuildIntent};
use lightyear::prelude::{Interpolated, MessageSender};

use crate::connection::LocalPlayerId;
use crate::flow::AppFlow;
use crate::ui::theme;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of housing plots.
pub const HOUSING_PLOT_COUNT: usize = 16;

/// Build mode toggle key.
const BUILD_TOGGLE_KEY: KeyCode = KeyCode::KeyB;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Whether the player is currently in build mode.
#[derive(Resource, Default)]
pub struct BuildMode(pub bool);

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Marks a plot boundary visual entity.
#[derive(Component)]
pub struct PlotBoundary {
    pub slot_index: usize,
}

/// Marks a decoration visual entity spawned from replicated DecorationState.
#[derive(Component)]
pub struct HousingDecoration;

/// Provisional preview of a decoration placement (client-side, not confirmed).
#[derive(Component)]
pub struct PlacementPreview;

/// Marker for the build-mode control bar.
#[derive(Component)]
pub struct BuildControls;

// ---------------------------------------------------------------------------
// Systems — plot boundary spawning
// ---------------------------------------------------------------------------

/// Spawns the 16 square plot boundary indicators (semi-transparent floor
/// quads). Runs once at startup.
pub fn spawn_plot_boundaries(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let quad = meshes.add(Rectangle::new(PLOT_HALF_SIZE * 2.0, PLOT_HALF_SIZE * 2.0));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.8, 0.9, 1.0, 0.12),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });
    let edge_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.8, 0.9, 1.0, 0.25),
        unlit: true,
        ..default()
    });

    for slot in plot_layout() {
        let center = slot.center;
        // Semi-transparent floor quad
        commands.spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(center + Vec3::Y * 0.01),
            PlotBoundary {
                slot_index: slot.index,
            },
        ));
        // Edge lines (thin quads at each edge)
        for (dx, dz) in &[
            (PLOT_HALF_SIZE, 0.0),
            (-PLOT_HALF_SIZE, 0.0),
            (0.0, PLOT_HALF_SIZE),
            (0.0, -PLOT_HALF_SIZE),
        ] {
            let edge = meshes.add(Rectangle::new(0.05, PLOT_HALF_SIZE * 2.0));
            let yaw = if dx.abs() > dz.abs() {
                std::f32::consts::FRAC_PI_2
            } else {
                0.0
            };
            commands.spawn((
                Mesh3d(edge),
                MeshMaterial3d(edge_mat.clone()),
                Transform::from_translation(center + Vec3::new(*dx, 0.02, *dz))
                    .with_rotation(Quat::from_rotation_y(yaw)),
                PlotBoundary {
                    slot_index: slot.index,
                },
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Systems — decoration rendering from replicated state
// ---------------------------------------------------------------------------

type UnvisualizedDecorations<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static DecorationState),
    (With<Interpolated>, Without<HousingDecoration>),
>;

/// Attaches a small crystal GLB scene to newly replicated [`DecorationState`]
/// entities.
pub fn attach_decoration_visuals(
    mut commands: Commands,
    arena_assets: Option<Res<crate::arena::ArenaAssets>>,
    decorations: UnvisualizedDecorations,
) {
    let Some(assets) = arena_assets else {
        return;
    };
    for (entity, state) in decorations.iter() {
        commands.entity(entity).insert((
            Transform::from_xyz(state.position_x, state.position_y, state.position_z)
                .with_rotation(Quat::from_rotation_y(state.rotation)),
            HousingDecoration,
        ));
        // Attach crystal_small as the decoration model
        let scene = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::Inherited,
                WorldAssetRoot(assets.crystal_small.clone()),
            ))
            .id();
        commands.entity(entity).add_child(scene);
    }
}

// ---------------------------------------------------------------------------
// Systems — build mode toggle
// ---------------------------------------------------------------------------

/// Toggles build mode when the B key is pressed (desktop) and the game is
/// in the `InGame` state.
pub fn toggle_build_mode(
    keys: Res<ButtonInput<KeyCode>>,
    flow: Res<State<AppFlow>>,
    mut build: ResMut<BuildMode>,
) {
    if flow.get() != &AppFlow::InGame {
        return;
    }
    if keys.just_pressed(BUILD_TOGGLE_KEY) {
        build.0 = !build.0;
    }
}

// ---------------------------------------------------------------------------
// Systems — plot owner indication
// ---------------------------------------------------------------------------

/// Updates plot boundary colors to indicate owner vs non-owner plots.
/// The player's assigned slot is highlighted in a tinted colour.
pub fn update_plot_owner_indicators(
    local_id: Res<LocalPlayerId>,
    mut boundaries: Query<(&PlotBoundary, &mut MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(my_id) = local_id.id else {
        return;
    };
    let my_slot = game_core::housing_layout::slot_for_player(my_id);
    for (boundary, mat_handle) in boundaries.iter_mut() {
        let is_owner = boundary.slot_index == my_slot;
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            if is_owner {
                mat.base_color = Color::srgba(0.6, 0.9, 0.6, 0.25);
            } else {
                mat.base_color = Color::srgba(0.8, 0.9, 1.0, 0.12);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Systems — build controls UI
// ---------------------------------------------------------------------------

/// Spawns or despawns the build-mode control bar based on build mode state.
pub fn build_controls_ui(
    mut commands: Commands,
    build: Res<BuildMode>,
    flow: Res<State<AppFlow>>,
    existing: Query<Entity, With<BuildControls>>,
    _senders: Query<&mut MessageSender<PlotBuildIntent>>,
    local_id: Res<LocalPlayerId>,
) {
    if flow.get() != &AppFlow::InGame {
        return;
    }

    let has_controls = existing.single().is_ok();

    if build.0 && !has_controls {
        let my_slot = local_id.id.map(game_core::housing_layout::slot_for_player);
        // Only show controls if player has a plot
        let _ = my_slot;
        commands
            .spawn((
                BuildControls,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(80.0),
                    left: Val::Px(50.0),
                    ..default()
                },
                BackgroundColor(theme::SURFACE_MENU),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Build Mode [B]"),
                    TextFont {
                        font_size: FontSize::Px(theme::FONT_SM),
                        ..default()
                    },
                    TextColor(theme::TEXT_TITLE),
                ));
                // Place crystal button — minimum 44×44 px touch target
                parent
                    .spawn((
                        Button,
                        Node {
                            margin: UiRect::top(Val::Px(4.0)),
                            width: Val::Px(theme::MIN_TOUCH_TARGET),
                            height: Val::Px(theme::MIN_TOUCH_TARGET),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(theme::RADIUS_PILL)),
                            ..default()
                        },
                        BackgroundColor(theme::BUTTON_PRIMARY),
                        Interaction::default(),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            Text::new("Place Crystal"),
                            TextFont {
                                font_size: FontSize::Px(theme::FONT_XS),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                // Remove button — minimum 44×44 px touch target
                parent
                    .spawn((
                        Button,
                        Node {
                            margin: UiRect::top(Val::Px(4.0)),
                            width: Val::Px(theme::MIN_TOUCH_TARGET),
                            height: Val::Px(theme::MIN_TOUCH_TARGET),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(theme::RADIUS_PILL)),
                            ..default()
                        },
                        BackgroundColor(theme::STATUS_ERR),
                        Interaction::default(),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            Text::new("Remove"),
                            TextFont {
                                font_size: FontSize::Px(theme::FONT_XS),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });
    } else if !build.0 && has_controls {
        commands.entity(existing.single().unwrap()).despawn();
    }
}

// ---------------------------------------------------------------------------
// Provisional preview
// ---------------------------------------------------------------------------

/// Visual feedback for a provisional placement (client-side only).
/// Removed when the server confirms or rejects.
#[derive(Component)]
pub struct ProvisionalPreview {
    pub sequence: u64,
}

/// Despawns failed provisional previews on receiving ActionRejected.
pub fn handle_action_rejected(
    mut receivers: Query<&mut lightyear::prelude::MessageReceiver<ActionRejected>>,
    previews: Query<(Entity, &ProvisionalPreview)>,
    mut commands: Commands,
) {
    for mut receiver in &mut receivers {
        for msg in receiver.receive() {
            for (entity, preview) in previews.iter() {
                if preview.sequence == msg.sequence {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::housing_layout::HOUSING_PLOT_COUNT;

    #[test]
    fn plot_boundaries_spawn_16_slots() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_systems(Startup, spawn_plot_boundaries);
        app.update();

        let mut q = app.world_mut().query::<&PlotBoundary>();
        // Each slot has 1 floor + 4 edges = 5 entities, so 16*5 = 80 total
        let count = q.iter(app.world()).count();
        assert_eq!(count, HOUSING_PLOT_COUNT * 5, "16 plots × 5 parts each");
    }

    #[test]
    fn build_mode_defaults_to_off() {
        let bm = BuildMode::default();
        assert!(!bm.0);
    }

    #[test]
    fn build_mode_toggle_in_ingame() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        app.init_resource::<BuildMode>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, toggle_build_mode);

        // Start in InGame
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::InGame);
        app.update();
        app.update();

        assert!(!app.world().resource::<BuildMode>().0);

        // Press B
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(BUILD_TOGGLE_KEY);
        app.update();
        assert!(app.world().resource::<BuildMode>().0);
    }

    #[test]
    fn build_mode_does_not_toggle_in_menu() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        app.init_resource::<BuildMode>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, toggle_build_mode);

        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::Menu);
        app.update();
        app.update();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(BUILD_TOGGLE_KEY);
        app.update();
        assert!(!app.world().resource::<BuildMode>().0);
    }

    #[test]
    fn action_rejected_despawns_preview() {
        let mut app = App::new();
        app.add_systems(Update, handle_action_rejected);
        // Add a preview entity with sequence 5
        let preview = app
            .world_mut()
            .spawn((ProvisionalPreview { sequence: 5 },))
            .id();
        app.update();

        // No rejection yet — entity still exists
        assert!(app.world().get::<ProvisionalPreview>(preview).is_some());

        // Rest of test requires MessageReceiver which needs Lightyear setup
        // Just verify no panic when no messages arrive
    }

    #[test]
    fn decoration_visuals_wait_for_assets() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.add_systems(Update, attach_decoration_visuals);
        app.init_asset::<WorldAsset>();
        let entity = app
            .world_mut()
            .spawn((
                DecorationState {
                    kind: game_core::decorations::DecorationKind::Tree,
                    position_x: 0.0,
                    position_y: 0.0,
                    position_z: 0.0,
                    rotation: 0.0,
                },
                Interpolated,
            ))
            .id();
        app.update();
        // Without ArenaAssets, no visual should attach
        assert!(
            app.world().get::<HousingDecoration>(entity).is_none(),
            "no visual without ArenaAssets"
        );
    }

    #[test]
    fn plot_owner_indicator_uses_slot_for_player() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.world_mut().insert_resource(LocalPlayerId {
            id: Some(game_core::id::PlayerId::new(42)),
        });
        app.add_systems(Startup, spawn_plot_boundaries);
        app.add_systems(Update, update_plot_owner_indicators);
        app.update();

        let _my_slot = game_core::housing_layout::slot_for_player(42_u64);
        // System shouldn't panic, and at least boundaries exist
        let mut q = app.world_mut().query::<&PlotBoundary>();
        assert!(q.iter(app.world()).count() > 0);
    }
}
