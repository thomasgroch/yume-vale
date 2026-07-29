use bevy::asset::AssetServer;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;

use game_core::arena::ArenaModel;
use game_core::world_config::CreatureKind;

use crate::arena::ArenaAssets;
use crate::flow::{AppFlow, LoadingError, WorldConfigResource};
use crate::visuals::creatures::CreatureAssets;
use crate::visuals::fox::FoxAssets;

use crate::loading::queue::{self, SeqLoader};

/// Create the sequential loader queue from the parsed world config.
///
/// Must run after `load_world_config` in the same `OnEnter(Loading)` chain
/// so that `WorldConfigResource` exists.
pub(crate) fn create_loading_queue(
    mut commands: Commands,
    config: Option<Res<WorldConfigResource>>,
) {
    let Some(config) = config else {
        commands.insert_resource(LoadingError {
            message: "WorldConfig not parsed before loading queue creation".into(),
        });
        return;
    };
    commands.insert_resource(SeqLoader::from_config(&config.0));
}

/// Single-frame tick of the sequential loader.
///
/// Polls the active asset handle, advances the queue, and finalizes typed
/// resources when the manifest is exhausted. Runs in `Update` gated by
/// `in_state(AppFlow::Loading)`.
///
/// When the `SeqLoader` is absent (e.g. config parse failed before queue
/// creation), returns immediately — the `LoadingError` resource drives the UI.
pub(crate) fn poll_and_finalize(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    loader: Option<ResMut<SeqLoader>>,
    config: Option<Res<WorldConfigResource>>,
    mut next_state: ResMut<NextState<AppFlow>>,
) {
    let Some(mut loader) = loader else {
        return;
    };

    // 1. Frozen on failure — UI shows the path.
    if loader.failing_path.is_some() {
        return;
    }

    // 2. Poll the active load.
    if loader.active.is_some() {
        let _ = loader.poll_active(&asset_server);
        if loader.failing_path.is_some() {
            return;
        }
    }

    // 3. Start the next queued item (at most one is active at a time).
    if loader.try_start_next(&asset_server) {
        return;
    }

    // 4. All items processed — finalize or transition.
    if !loader.all_loaded() {
        return;
    }

    if loader.total == 0 {
        commands.remove_resource::<SeqLoader>();
        next_state.set(AppFlow::Menu);
        return;
    }

    // ── Finalize: construct typed resources from cached handles ──────────

    let arena = ArenaAssets {
        portal: load_scene(&asset_server, ArenaModel::Portal.asset_path()),
        wall: load_scene(&asset_server, ArenaModel::Wall.asset_path()),
        pillar: load_scene(&asset_server, ArenaModel::Pillar.asset_path()),
        crystal_big: load_scene(&asset_server, ArenaModel::CrystalBig.asset_path()),
        crystal_small: load_scene(&asset_server, ArenaModel::CrystalSmall.asset_path()),
        rock: load_scene(&asset_server, ArenaModel::Rock.asset_path()),
    };

    let config = match config {
        Some(c) => c,
        None => {
            commands.insert_resource(LoadingError {
                message: "WorldConfig missing during finalization".into(),
            });
            loader.failing_path = Some("(config)".into());
            return;
        }
    };

    let fluffball_path = match config
        .0
        .creatures
        .iter()
        .find(|cr| cr.kind == CreatureKind::Fluffball)
        .map(|cr| {
            cr.model_path
                .strip_prefix("assets/")
                .unwrap_or(&cr.model_path)
        }) {
        Some(p) => p.to_string(),
        None => {
            commands.insert_resource(LoadingError {
                message: "Fluffball creature config not found".into(),
            });
            loader.failing_path = Some("(config)".into());
            return;
        }
    };

    let glimmerwing_path = match config
        .0
        .creatures
        .iter()
        .find(|cr| cr.kind == CreatureKind::Glimmerwing)
        .map(|cr| {
            cr.model_path
                .strip_prefix("assets/")
                .unwrap_or(&cr.model_path)
        }) {
        Some(p) => p.to_string(),
        None => {
            commands.insert_resource(LoadingError {
                message: "Glimmerwing creature config not found".into(),
            });
            loader.failing_path = Some("(config)".into());
            return;
        }
    };

    let creatures = CreatureAssets {
        fluffball: load_scene(&asset_server, &fluffball_path),
        glimmerwing: load_scene(&asset_server, &glimmerwing_path),
    };

    let fox_scene: Handle<WorldAsset> = load_scene(&asset_server, queue::paths::FOX_RIG);
    let idle: Handle<AnimationClip> = load_anim(&asset_server, queue::paths::FOX_IDLE);
    let walk: Handle<AnimationClip> = load_anim(&asset_server, queue::paths::FOX_WALK);
    let run: Handle<AnimationClip> = load_anim(&asset_server, queue::paths::FOX_RUN);
    let wave: Handle<AnimationClip> = load_anim(&asset_server, queue::paths::FOX_WAVE);
    let (graph, indices) = AnimationGraph::from_clips([idle, walk, run, wave]);
    let fox = FoxAssets {
        scene: fox_scene,
        graph: graphs.add(graph),
        idle: indices[0],
        walk: indices[1],
        run: indices[2],
        wave: indices[3],
    };

    commands.insert_resource(arena);
    commands.insert_resource(creatures);
    commands.insert_resource(fox);
    commands.remove_resource::<SeqLoader>();
    next_state.set(AppFlow::Menu);
}

// ── Private helpers ─────────────────────────────────────────────────────────

fn load_scene(server: &AssetServer, path: &str) -> Handle<WorldAsset> {
    server.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))
}

fn load_anim(server: &AssetServer, path: &str) -> Handle<AnimationClip> {
    server.load(GltfAssetLabel::Animation(0).from_asset(path.to_string()))
}
