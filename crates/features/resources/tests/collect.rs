//! Integration tests for resource system: spawn, validation, respawn.
//!
//! These tests verify the pure validation logic and the spawn/respawn systems
//! by building minimal Bevy apps.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use game_core::constants::{INTERACT_COOLDOWN_S, INTERACT_RADIUS, TICK_RATE_HZ};
use game_core::id::ResourceId;
use game_core::inventory::{Inventory, ItemKind};
use game_core::resources::ResourceKind;
use game_core::world_config::{ResourceConfig, WorldConfig};
use resources::components::*;
use resources::systems::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a minimal world config with known resource positions for testing.
fn test_world_config() -> WorldConfig {
    WorldConfig {
        resources: vec![
            ResourceConfig {
                id: ResourceId::new(1),
                kind: ResourceKind::Wood,
                count: 2,
                yield_amount: 2,
                respawn_seconds: 30.0,
                positions: vec![Vec3::new(5.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0)],
                model_path: "wood.glb".into(),
            },
            ResourceConfig {
                id: ResourceId::new(2),
                kind: ResourceKind::Berry,
                count: 1,
                yield_amount: 3,
                respawn_seconds: 20.0,
                positions: vec![Vec3::new(0.0, 0.0, 5.0)],
                model_path: "berry.glb".into(),
            },
        ],
        creatures: vec![],
    }
}

/// Build a test app with resource nodes spawned.
fn resource_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_systems(Startup, |commands: Commands| {
        spawn_resource_nodes(commands, &test_world_config());
    });
    app.add_systems(Update, tick_resource_respawn);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f64(1.0 / TICK_RATE_HZ as f64),
    ));
    app.finish();
    app.update(); // Run startup
    app
}

// ---------------------------------------------------------------------------
// RED tests: spawn count & IDs
// ---------------------------------------------------------------------------

#[test]
fn spawns_correct_number_of_resource_nodes() {
    let mut app = resource_app();
    let count = app
        .world_mut()
        .query::<&ResourceNode>()
        .iter(app.world())
        .count();
    assert_eq!(count, 3, "expected 3 resource nodes (2 Wood + 1 Berry)");
}

#[test]
fn resource_nodes_have_deterministic_indices() {
    let mut app = resource_app();
    let mut query = app.world_mut().query::<&ResourceNode>();
    let nodes: Vec<&ResourceNode> = query.iter(app.world()).collect();

    assert_eq!(nodes[0].node_index, 0);
    assert_eq!(nodes[0].kind, ResourceKind::Wood);
    assert_eq!(nodes[0].yield_amount, 2);
    assert_eq!(nodes[0].respawn_seconds, 30.0);

    assert_eq!(nodes[1].node_index, 1);
    assert_eq!(nodes[1].kind, ResourceKind::Wood);

    assert_eq!(nodes[2].node_index, 2);
    assert_eq!(nodes[2].kind, ResourceKind::Berry);
    assert_eq!(nodes[2].yield_amount, 3);
    assert_eq!(nodes[2].respawn_seconds, 20.0);
}

// ---------------------------------------------------------------------------
// RED tests: validate_collect pure function
// ---------------------------------------------------------------------------

#[test]
fn validate_collect_success() {
    let node = ResourceNode {
        node_index: 0,
        kind: ResourceKind::Wood,
        yield_amount: 2,
        respawn_seconds: 30.0,
        position: Vec3::new(0.0, 0.0, 0.0),
    };
    let status = ResourceNodeStatus {
        depleted: false,
        respawn_timer: 0.0,
    };
    let inventory = PlayerInventory::default();
    let cooldown = InteractionCooldown::default();
    let sequence = ActionSequence::default();

    let result = validate_collect(
        Vec3::ZERO, // player at same position
        Some(&node),
        Some(&status),
        Some(&inventory),
        Some(&cooldown),
        Some(&sequence),
        1,
    );
    assert_eq!(result, CollectValidation::Success);
}

#[test]
fn validate_collect_out_of_range() {
    let node = ResourceNode {
        node_index: 0,
        kind: ResourceKind::Wood,
        yield_amount: 2,
        respawn_seconds: 30.0,
        position: Vec3::ZERO,
    };
    let status = ResourceNodeStatus {
        depleted: false,
        respawn_timer: 0.0,
    };

    let far_pos = Vec3::new(INTERACT_RADIUS + 1.0, 0.0, 0.0);
    let result = validate_collect(far_pos, Some(&node), Some(&status), None, None, None, 1);
    assert_eq!(result, CollectValidation::OutOfRange);
}

#[test]
fn validate_collect_unavailable_node() {
    let node = ResourceNode {
        node_index: 0,
        kind: ResourceKind::Wood,
        yield_amount: 2,
        respawn_seconds: 30.0,
        position: Vec3::ZERO,
    };
    let status = ResourceNodeStatus {
        depleted: true,
        respawn_timer: 10.0,
    };

    let result = validate_collect(Vec3::ZERO, Some(&node), Some(&status), None, None, None, 1);
    assert_eq!(result, CollectValidation::NodeNotAvailable);
}

#[test]
fn validate_collect_full_inventory() {
    let node = ResourceNode {
        node_index: 0,
        kind: ResourceKind::Wood,
        yield_amount: 2,
        respawn_seconds: 30.0,
        position: Vec3::ZERO,
    };
    let status = ResourceNodeStatus {
        depleted: false,
        respawn_timer: 0.0,
    };
    // Fill inventory
    let mut inv = Inventory::new(1);
    inv.add(ItemKind::Resource(ResourceKind::Wood), 99).unwrap();
    let player_inv = PlayerInventory { inventory: inv };

    let result = validate_collect(
        Vec3::ZERO,
        Some(&node),
        Some(&status),
        Some(&player_inv),
        None,
        None,
        1,
    );
    assert_eq!(result, CollectValidation::InventoryFull);
}

#[test]
fn validate_collect_stale_sequence() {
    let node = ResourceNode {
        node_index: 0,
        kind: ResourceKind::Wood,
        yield_amount: 2,
        respawn_seconds: 30.0,
        position: Vec3::ZERO,
    };
    let status = ResourceNodeStatus {
        depleted: false,
        respawn_timer: 0.0,
    };
    let sequence = ActionSequence { last_sequence: 5 };

    let result = validate_collect(
        Vec3::ZERO,
        Some(&node),
        Some(&status),
        None,
        None,
        Some(&sequence),
        3, // Less than or equal to last_sequence
    );
    assert_eq!(result, CollectValidation::StaleSequence);

    // Equal should also fail
    let result2 = validate_collect(
        Vec3::ZERO,
        Some(&node),
        Some(&status),
        None,
        None,
        Some(&sequence),
        5,
    );
    assert_eq!(result2, CollectValidation::StaleSequence);
}

#[test]
fn validate_collect_cooldown_active() {
    let node = ResourceNode {
        node_index: 0,
        kind: ResourceKind::Wood,
        yield_amount: 2,
        respawn_seconds: 30.0,
        position: Vec3::ZERO,
    };
    let status = ResourceNodeStatus {
        depleted: false,
        respawn_timer: 0.0,
    };
    let cooldown = InteractionCooldown {
        active: true,
        elapsed: INTERACT_COOLDOWN_S * 0.5, // 0.25s < 0.5s
    };

    let result = validate_collect(
        Vec3::ZERO,
        Some(&node),
        Some(&status),
        None,
        Some(&cooldown),
        None,
        1,
    );
    assert_eq!(result, CollectValidation::CooldownActive);
}

#[test]
fn validate_collect_cooldown_expired() {
    let node = ResourceNode {
        node_index: 0,
        kind: ResourceKind::Wood,
        yield_amount: 2,
        respawn_seconds: 30.0,
        position: Vec3::ZERO,
    };
    let status = ResourceNodeStatus {
        depleted: false,
        respawn_timer: 0.0,
    };
    let cooldown = InteractionCooldown {
        active: true,
        elapsed: INTERACT_COOLDOWN_S, // At cooldown boundary
    };

    let result = validate_collect(
        Vec3::ZERO,
        Some(&node),
        Some(&status),
        None,
        Some(&cooldown),
        None,
        1,
    );
    assert_eq!(result, CollectValidation::Success);
}

#[test]
fn validate_collect_unknown_node() {
    let result = validate_collect(Vec3::ZERO, None, None, None, None, None, 1);
    assert_eq!(result, CollectValidation::UnknownNode);
}

// ---------------------------------------------------------------------------
// RED tests: respawn logic
// ---------------------------------------------------------------------------

#[test]
fn node_respawns_after_configured_duration() {
    let mut app = resource_app();
    // Find the Berry node (index 2, respawn_seconds=20)
    let node_entity = {
        let mut query = app.world_mut().query::<(Entity, &ResourceNode)>();
        query
            .iter(app.world())
            .find(|(_, n)| n.kind == ResourceKind::Berry)
            .map(|(e, _)| e)
            .expect("Berry node not found")
    };

    // Deplete it manually
    let mut status = app
        .world_mut()
        .get_mut::<ResourceNodeStatus>(node_entity)
        .unwrap();
    status.depleted = true;
    status.respawn_timer = 20.0;

    // Tick forward less than respawn time
    for _ in 0..(20 * TICK_RATE_HZ - 1) {
        app.update();
    }

    let status = app.world().get::<ResourceNodeStatus>(node_entity).unwrap();
    assert!(
        status.depleted,
        "node should still be depleted before respawn timer"
    );

    // Tick past respawn
    app.update();

    let status = app.world().get::<ResourceNodeStatus>(node_entity).unwrap();
    assert!(!status.depleted, "node should have respawned");
    assert_eq!(status.respawn_timer, 0.0);
}

#[test]
fn resource_node_initial_state_is_available() {
    let mut app = resource_app();
    let mut query = app.world_mut().query::<&ResourceNodeStatus>();
    for status in query.iter(app.world()) {
        assert!(!status.depleted, "all nodes should start as available");
    }
}
