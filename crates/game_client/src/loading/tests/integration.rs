use crate::flow::{AppFlow, LoadingError, WorldConfigResource};
use crate::loading::queue::SeqLoader;
use crate::loading::systems::{create_loading_queue, poll_and_finalize};
use bevy::prelude::*;
use game_core::world_config::WorldConfig;

// ── create_loading_queue ───────────────────────────────────────────────────

#[test]
fn create_loading_queue_inserts_manifest() {
    let mut app = super::loader_app();
    app.insert_resource(WorldConfigResource(super::production_config()));
    app.add_systems(OnEnter(AppFlow::Loading), create_loading_queue);
    app.update();
    let loader = app.world().resource::<SeqLoader>();
    assert_eq!(loader.total, 16);
    assert_eq!(loader.progress, 0);
}

#[test]
fn create_loading_queue_errors_without_config() {
    let mut app = super::loader_app();
    app.add_systems(OnEnter(AppFlow::Loading), create_loading_queue);
    app.update();
    let err = app.world().resource::<LoadingError>();
    assert!(err.message.contains("WorldConfig"), "err: {}", err.message);
}

// ── poll_and_finalize ──────────────────────────────────────────────────────

#[test]
fn empty_loader_transitions_to_menu() {
    let mut app = super::loader_app();
    app.insert_resource(WorldConfigResource(WorldConfig::default()));
    app.add_systems(Update, poll_and_finalize.run_if(in_state(AppFlow::Loading)));
    app.world_mut().insert_resource(SeqLoader {
        queue: vec![],
        active: None,
        completed: vec![],
        progress: 0,
        total: 0,
        failing_path: None,
    });
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<State<AppFlow>>().get(),
        &AppFlow::Menu
    );
}

#[test]
fn failing_loader_stays_in_loading() {
    let mut app = super::loader_app();
    app.insert_resource(WorldConfigResource(WorldConfig::default()));
    app.add_systems(Update, poll_and_finalize.run_if(in_state(AppFlow::Loading)));
    app.world_mut().insert_resource(SeqLoader {
        queue: vec![],
        active: None,
        completed: vec![],
        progress: 0,
        total: 1,
        failing_path: Some("models/broken.glb".into()),
    });
    app.update();
    assert_eq!(
        app.world().resource::<State<AppFlow>>().get(),
        &AppFlow::Loading
    );
    assert_eq!(
        app.world().resource::<SeqLoader>().failing_path.as_deref(),
        Some("models/broken.glb"),
    );
}

#[test]
fn no_loader_noop_no_panic() {
    let mut app = super::loader_app();
    app.insert_resource(WorldConfigResource(WorldConfig::default()));
    app.add_systems(Update, poll_and_finalize.run_if(in_state(AppFlow::Loading)));
    app.update();
    assert_eq!(
        app.world().resource::<State<AppFlow>>().get(),
        &AppFlow::Loading
    );
}

// ── Finalization: config/creature validation ───────────────────────────────

#[test]
fn missing_config_at_finalization_stays_in_loading() {
    let mut app = super::loader_app();
    app.add_systems(Update, poll_and_finalize.run_if(in_state(AppFlow::Loading)));

    // Insert a complete loader but NO WorldConfigResource.
    app.world_mut().insert_resource(SeqLoader {
        queue: vec![],
        active: None,
        completed: vec![bevy::asset::UntypedHandle::default_for_type(
            std::any::TypeId::of::<bevy::world_serialization::WorldAsset>(),
        )],
        progress: 1,
        total: 1,
        failing_path: None,
    });

    app.update();
    assert_eq!(
        app.world().resource::<State<AppFlow>>().get(),
        &AppFlow::Loading,
        "must stay in Loading when config is absent at finalization"
    );
    assert!(
        app.world()
            .resource::<LoadingError>()
            .message
            .contains("WorldConfig"),
        "LoadingError must mention WorldConfig"
    );
    assert_eq!(
        app.world().resource::<SeqLoader>().failing_path.as_deref(),
        Some("(config)"),
        "failing_path must be set to indicate config error",
    );
}

#[test]
fn missing_fluffball_at_finalization_stays_in_loading() {
    let mut app = super::loader_app();
    let mut config = WorldConfig::default();
    // Only include Glimmerwing, skip Fluffball.
    config
        .creatures
        .push(game_core::world_config::CreatureConfig {
            id: game_core::id::CreatureId::new(2),
            kind: game_core::world_config::CreatureKind::Glimmerwing,
            center: bevy::math::Vec3::ZERO,
            wander_radius: 5.0,
            food_kind: game_core::resources::ResourceKind::Crystal,
            model_path: "assets/models/creatures/glimmerwing.glb".into(),
        });
    app.insert_resource(WorldConfigResource(config));
    app.add_systems(Update, poll_and_finalize.run_if(in_state(AppFlow::Loading)));

    app.world_mut().insert_resource(SeqLoader {
        queue: vec![],
        active: None,
        completed: vec![bevy::asset::UntypedHandle::default_for_type(
            std::any::TypeId::of::<bevy::world_serialization::WorldAsset>(),
        )],
        progress: 1,
        total: 1,
        failing_path: None,
    });

    app.update();
    assert_eq!(
        app.world().resource::<State<AppFlow>>().get(),
        &AppFlow::Loading,
        "must stay in Loading when Fluffball is missing from config"
    );
    assert!(
        app.world()
            .resource::<LoadingError>()
            .message
            .contains("Fluffball"),
        "LoadingError must mention Fluffball"
    );
}
