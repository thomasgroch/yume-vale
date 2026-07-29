mod cache;
mod diet;
mod integration;
mod manifest;
mod queue;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use game_core::world_config::{CreatureConfig, ResourceConfig, WorldConfig};

/// A config matching the shipped world.ron (3 resources + 2 creatures).
fn production_config() -> WorldConfig {
    use bevy::math::Vec3;
    WorldConfig {
        resources: vec![
            ResourceConfig {
                id: game_core::id::ResourceId::new(1),
                kind: game_core::resources::ResourceKind::Wood,
                count: 3,
                yield_amount: 2,
                respawn_seconds: 30.0,
                positions: vec![
                    Vec3::new(8.0, 0.0, 8.0),
                    Vec3::new(10.0, 0.0, 5.0),
                    Vec3::new(6.0, 0.0, 10.0),
                ],
                model_path: "assets/models/resources/wood.glb".into(),
            },
            ResourceConfig {
                id: game_core::id::ResourceId::new(2),
                kind: game_core::resources::ResourceKind::Crystal,
                count: 2,
                yield_amount: 1,
                respawn_seconds: 60.0,
                positions: vec![Vec3::new(-8.0, 0.0, -5.0), Vec3::new(-10.0, 0.0, -8.0)],
                model_path: "assets/models/resources/crystal.glb".into(),
            },
            ResourceConfig {
                id: game_core::id::ResourceId::new(3),
                kind: game_core::resources::ResourceKind::Berry,
                count: 4,
                yield_amount: 3,
                respawn_seconds: 20.0,
                positions: vec![
                    Vec3::new(-3.0, 0.0, 8.0),
                    Vec3::new(5.0, 0.0, -4.0),
                    Vec3::new(-6.0, 0.0, -3.0),
                    Vec3::new(0.0, 0.0, 12.0),
                ],
                model_path: "assets/models/resources/berry.glb".into(),
            },
        ],
        creatures: vec![
            CreatureConfig {
                id: game_core::id::CreatureId::new(1),
                kind: game_core::world_config::CreatureKind::Fluffball,
                center: Vec3::new(10.0, 0.0, 5.0),
                wander_radius: 8.0,
                food_kind: game_core::resources::ResourceKind::Berry,
                model_path: "assets/models/creatures/fluffball.glb".into(),
            },
            CreatureConfig {
                id: game_core::id::CreatureId::new(2),
                kind: game_core::world_config::CreatureKind::Glimmerwing,
                center: Vec3::new(-5.0, 0.0, 15.0),
                wander_radius: 6.0,
                food_kind: game_core::resources::ResourceKind::Crystal,
                model_path: "assets/models/creatures/glimmerwing.glb".into(),
            },
        ],
    }
}

/// Minimal app with task pools, asset types, and states registered.
fn loader_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        StatesPlugin,
    ));
    app.init_asset::<bevy::world_serialization::WorldAsset>();
    app.init_asset::<AnimationClip>();
    app.init_asset::<AnimationGraph>();
    app.init_state::<crate::flow::AppFlow>();
    app
}
