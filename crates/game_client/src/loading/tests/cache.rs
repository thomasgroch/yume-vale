//! Cache-identity tests: prove that `asset_server.load()` for the same
//! labeled path returns the same `AssetId` as a retained handle, which is
//! the mechanism finalization relies on to avoid a cold second load wave.

use bevy::asset::{AssetPlugin, AssetServer, UntypedHandle};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;

fn cache_app() -> App {
    let mut app = App::new();
    app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()));
    app.init_asset::<WorldAsset>();
    app.init_asset::<AnimationClip>();
    app
}

#[test]
fn scene_label_path_returns_same_id() {
    let app = cache_app();
    let server = app.world().resource::<AssetServer>().clone();

    let path = "cache_test_scene.glb";
    let first: Handle<WorldAsset> = server.load(GltfAssetLabel::Scene(0).from_asset(path));
    let first_id = first.id();
    let _retained: UntypedHandle = first.untyped();

    let second: Handle<WorldAsset> = server.load(GltfAssetLabel::Scene(0).from_asset(path));
    assert_eq!(
        second.id(),
        first_id,
        "Scene(0) re-load must return matching AssetId"
    );
}

#[test]
fn animation_label_path_returns_same_id() {
    let app = cache_app();
    let server = app.world().resource::<AssetServer>().clone();

    let path = "cache_test_anim.glb";
    let first: Handle<AnimationClip> = server.load(GltfAssetLabel::Animation(0).from_asset(path));
    let first_id = first.id();
    let _retained: UntypedHandle = first.untyped();

    let second: Handle<AnimationClip> = server.load(GltfAssetLabel::Animation(0).from_asset(path));
    assert_eq!(
        second.id(),
        first_id,
        "Animation(0) re-load must return matching AssetId"
    );
}

#[test]
fn different_label_same_path_different_id() {
    let app = cache_app();
    let server = app.world().resource::<AssetServer>().clone();

    let path = "cache_test_multi.glb";
    let scene: Handle<WorldAsset> = server.load(GltfAssetLabel::Scene(0).from_asset(path));
    let anim: Handle<AnimationClip> = server.load(GltfAssetLabel::Animation(0).from_asset(path));

    assert_ne!(
        scene.untyped().id(),
        anim.untyped().id(),
        "Scene(0) and Animation(0) from same GLB must have different AssetIds"
    );
}
