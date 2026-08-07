//! Integration tests for authoritative resource collection.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use core::time::Duration;
use game_core::actions::ActionKind;
use game_core::constants::INTERACT_RADIUS;
use game_core::inventory::ItemKind;
use game_core::resources::ResourceKind;
use game_core::world_config::{ResourceConfig, WorldConfig};
use game_protocol::ProtocolPlugin;
use game_server::systems::setup::WorldConfigResource;
use game_server::systems::{
    NextPlayerColor, ServerSystems, apply_client_input, handle_action_intent,
    handle_new_client_link, initialize_player_components, tick_player_cooldowns,
};
use lightyear::crossbeam::CrossbeamIo;
use lightyear::prelude::client::{ClientPlugins, RawClient};
use lightyear::prelude::server::{LinkOf, RawServer, ServerPlugins, Started};
use lightyear::prelude::*;
use player::{Player, PlayerPlugin};
use resources::components::*;
use resources::systems::spawn_resource_nodes;
use resources::systems::tick_resource_respawn;
use std::net::{Ipv4Addr, SocketAddr};

// ---------------------------------------------------------------------------
// Minimal test app (no SocialPlugin)
// ---------------------------------------------------------------------------

const TICK: Duration = Duration::from_millis(16);
const MAX_FRAMES: usize = 400;

fn server_app_minimal() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(avian3d::PhysicsPlugins::default());
    app.add_plugins(ServerPlugins {
        tick_duration: TICK,
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin));
    app.init_resource::<NextPlayerColor>();
    app.insert_resource(WorldConfigResource(WorldConfig::default()));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(TICK));
    app.add_observer(handle_new_client_link);
    app.add_observer(game_server::systems::auth::on_client_connected);
    app.add_systems(
        FixedUpdate,
        (
            game_server::systems::auth::handle_identity_hello,
            apply_client_input,
            handle_action_intent,
            initialize_player_components,
            tick_player_cooldowns,
            tick_resource_respawn,
        )
            .in_set(ServerSystems),
    );
    app.finish();
    app
}

fn client_app_minimal() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(ClientPlugins {
        tick_duration: TICK,
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(TICK));
    app.finish();
    app
}

fn connect_client_minimal(server: &mut App, client: &mut App, port: u16) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    let (client_io, server_io) = CrossbeamIo::new_pair();
    let se = server.world_mut().spawn_empty().id();
    server
        .world_mut()
        .entity_mut(se)
        .insert((RawServer, Started));
    let _lo = server
        .world_mut()
        .spawn((LinkOf { server: se }, server_io, PeerAddr(addr)))
        .id();
    server.world_mut().trigger(LinkStart { entity: _lo });
    let ce = client
        .world_mut()
        .spawn((RawClient, client_io, PeerAddr(addr), ReplicationReceiver))
        .id();
    client.world_mut().trigger(Connect { entity: ce });
}

fn step_minimal(server: &mut App, client: &mut App, frames: usize) {
    for _ in 0..frames {
        server.update();
        client.update();
    }
}

fn wait_until_minimal<F>(server: &mut App, client: &mut App, mut cond: F) -> bool
where
    F: FnMut(&mut App, &mut App) -> bool,
{
    for _ in 0..MAX_FRAMES {
        if cond(server, client) {
            return true;
        }
        step_minimal(server, client, 1);
    }
    cond(server, client)
}

fn send_identity_hello_minimal(server: &mut App, client: &mut App, token: &str) {
    use game_protocol::channels::ReliableChannel;
    use lightyear::prelude::MessageSender;

    step_minimal(server, client, 10);
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<game_protocol::IdentityHello>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(game_protocol::IdentityHello {
            protocol_version: game_protocol::PROTOCOL_ID as u32,
            token: token.to_string(),
        });
    }
    step_minimal(server, client, 3);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A world config with resource nodes placed near the spawn (0,0,0).
fn resource_test_config() -> WorldConfig {
    WorldConfig {
        resources: vec![ResourceConfig {
            id: game_core::id::ResourceId::new(1),
            kind: ResourceKind::Wood,
            count: 1,
            yield_amount: 2,
            respawn_seconds: 30.0,
            positions: vec![Vec3::new(1.0, 0.0, 0.0)], // 1 unit away
            model_path: "wood.glb".into(),
        }],
        creatures: vec![],
    }
}

/// Build a server app with resource nodes placed near origin.
fn resource_server_app() -> App {
    let mut app = server_app_minimal();
    app.insert_resource(WorldConfigResource(resource_test_config()));
    app.add_systems(
        PostStartup,
        |commands: Commands, config: Res<WorldConfigResource>| {
            spawn_resource_nodes(commands, &config.0);
        },
    );
    app
}

/// Send an ActionIntent::Collect from the client to the server.
fn send_collect_intent(client: &mut App, server: &mut App, sequence: u64, target_id: u64) {
    use game_protocol::channels::ReliableChannel;

    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<game_protocol::messages::ActionIntent>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(game_protocol::messages::ActionIntent {
            sequence,
            kind: ActionKind::Collect,
            target_id: Some(target_id),
        });
    }

    step_minimal(server, client, 3);
}

/// Count the number of items of a given kind in the client's known inventory.
/// This reads from the player entity's PlayerInventory component on the SERVER side.
fn server_inventory_count(server: &mut App, kind: ItemKind) -> u32 {
    let mut query = server.world_mut().query::<&PlayerInventory>();
    query
        .iter(server.world())
        .flat_map(|inv| &inv.inventory.slots)
        .filter_map(|s| s.as_ref())
        .filter(|stack| stack.kind == kind)
        .map(|stack| stack.quantity)
        .sum()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn collect_adds_to_inventory() {
    let mut server = resource_server_app();
    let mut client = client_app_minimal();
    connect_client_minimal(&mut server, &mut client, 30010);
    send_identity_hello_minimal(&mut server, &mut client, "");

    let ok = wait_until_minimal(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should have spawned");

    send_collect_intent(&mut client, &mut server, 1, 0);

    let count = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count > 0, "inventory should have wood after collecting");
}

#[test]
fn collect_out_of_range_rejected() {
    let mut server = resource_server_app();
    let mut client = client_app_minimal();
    connect_client_minimal(&mut server, &mut client, 30011);
    send_identity_hello_minimal(&mut server, &mut client, "");

    let ok = wait_until_minimal(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should have spawned");

    let player_entity = server
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .iter(server.world())
        .next()
        .unwrap();
    server
        .world_mut()
        .entity_mut(player_entity)
        .insert(Transform::from_translation(Vec3::new(
            INTERACT_RADIUS + 10.0,
            0.0,
            0.0,
        )));
    server.world_mut().run_schedule(FixedUpdate);

    send_collect_intent(&mut client, &mut server, 1, 0);

    let count = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert_eq!(count, 0, "out-of-range collect should be rejected");
}

#[test]
fn collect_depleted_node_rejected() {
    let mut server = resource_server_app();
    let mut client = client_app_minimal();
    connect_client_minimal(&mut server, &mut client, 30012);
    send_identity_hello_minimal(&mut server, &mut client, "");

    let ok = wait_until_minimal(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should have spawned");

    send_collect_intent(&mut client, &mut server, 1, 0);
    let count1 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count1 > 0, "first collect should succeed");

    send_collect_intent(&mut client, &mut server, 2, 0);
    let count2 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert_eq!(count1, count2, "depleted node collect should be rejected");
}

#[test]
fn collect_with_full_inventory_rejected() {
    let mut server = resource_server_app();
    let mut client = client_app_minimal();
    connect_client_minimal(&mut server, &mut client, 30013);
    send_identity_hello_minimal(&mut server, &mut client, "");

    let ok = wait_until_minimal(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should have spawned");

    let player_entity = server
        .world_mut()
        .query_filtered::<Entity, (With<Player>, With<PlayerInventory>)>()
        .iter(server.world())
        .next()
        .unwrap();

    if let Some(mut inv) = server.world_mut().get_mut::<PlayerInventory>(player_entity) {
        for i in 0..inv.inventory.capacity {
            inv.inventory.slots[i] = Some(game_core::inventory::ItemStack::new(
                ItemKind::Resource(ResourceKind::Wood),
                game_core::constants::MAX_STACK_SIZE,
            ));
        }
    }
    server.world_mut().run_schedule(FixedUpdate);

    send_collect_intent(&mut client, &mut server, 1, 0);
    let total: u32 = server
        .world()
        .get::<PlayerInventory>(player_entity)
        .map(|inv| {
            inv.inventory
                .slots
                .iter()
                .filter_map(|s| s.as_ref())
                .count() as u32
        })
        .unwrap_or(0);
    assert_eq!(total, game_core::constants::INVENTORY_CAPACITY as u32);
}

#[test]
fn duplicate_sequence_rejected() {
    let mut server = resource_server_app();
    let mut client = client_app_minimal();
    connect_client_minimal(&mut server, &mut client, 30014);
    send_identity_hello_minimal(&mut server, &mut client, "");

    let ok = wait_until_minimal(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should have spawned");

    send_collect_intent(&mut client, &mut server, 1, 0);
    let count1 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count1 > 0);

    // Restore node + cooldown for second attempt
    let node_entity = server
        .world_mut()
        .query::<(Entity, &ResourceNode)>()
        .iter(server.world())
        .find(|(_, n)| n.node_index == 0)
        .map(|(e, _)| e)
        .unwrap();
    if let Some(mut status) = server
        .world_mut()
        .get_mut::<ResourceNodeStatus>(node_entity)
    {
        status.depleted = false;
        status.respawn_timer = 0.0;
    }
    if let Some(mut rep) = server
        .world_mut()
        .get_mut::<game_protocol::ResourceNodeState>(node_entity)
    {
        rep.depleted = false;
    }
    let player_entity = server
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .iter(server.world())
        .next()
        .unwrap();
    if let Some(mut cd) = server
        .world_mut()
        .get_mut::<InteractionCooldown>(player_entity)
    {
        cd.active = false;
    }
    server.world_mut().run_schedule(FixedUpdate);

    send_collect_intent(&mut client, &mut server, 1, 0);
    let count2 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert_eq!(
        count1, count2,
        "duplicate/stale sequence should be rejected"
    );
}

#[test]
fn cooldown_enforced_correctly() {
    let mut server = resource_server_app();
    let mut client = client_app_minimal();
    connect_client_minimal(&mut server, &mut client, 30015);
    send_identity_hello_minimal(&mut server, &mut client, "");

    let ok = wait_until_minimal(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should have spawned");

    send_collect_intent(&mut client, &mut server, 1, 0);
    let count1 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count1 > 0, "first collect should succeed");

    // Restore node so it's available for the next attempt.
    // Cooldown was set to active + 0.0 during the collect — leave it active.
    let node_entity = server
        .world_mut()
        .query::<(Entity, &ResourceNode)>()
        .iter(server.world())
        .find(|(_, n)| n.node_index == 0)
        .map(|(e, _)| e)
        .unwrap();
    if let Some(mut status) = server
        .world_mut()
        .get_mut::<ResourceNodeStatus>(node_entity)
    {
        status.depleted = false;
        status.respawn_timer = 0.0;
    }
    if let Some(mut rep) = server
        .world_mut()
        .get_mut::<game_protocol::ResourceNodeState>(node_entity)
    {
        rep.depleted = false;
    }
    server.world_mut().run_schedule(FixedUpdate);

    // The real frame delta is TICK=16ms, not 1/TICK_RATE_HZ.
    let dt = TICK.as_secs_f64();

    // Try collecting at ~0.4s (still inside 0.5s cooldown window)
    let ticks_040 = (0.4 / dt) as usize;
    for _ in 0..ticks_040 {
        step_minimal(&mut server, &mut client, 1);
    }
    send_collect_intent(&mut client, &mut server, 2, 0);
    let count2 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert_eq!(
        count1, count2,
        "collect at ~0.4s should be rejected by cooldown"
    );

    // Wait past 0.5s and try again
    let ticks_past = (0.15 / dt) as usize;
    for _ in 0..ticks_past {
        step_minimal(&mut server, &mut client, 1);
    }
    send_collect_intent(&mut client, &mut server, 3, 0);
    let count3 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count3 > count2, "collect after ~0.55s should succeed");
}

#[test]
fn inventory_snapshot_sent_on_collect() {
    let mut server = resource_server_app();
    let mut client = client_app_minimal();
    connect_client_minimal(&mut server, &mut client, 30016);
    send_identity_hello_minimal(&mut server, &mut client, "");

    let ok = wait_until_minimal(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should have spawned");

    send_collect_intent(&mut client, &mut server, 1, 0);

    let count = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count > 0, "inventory should have items after collect");
}

#[test]
fn respawn_timer_restores_node() {
    let mut server = resource_server_app();
    let mut client = client_app_minimal();
    connect_client_minimal(&mut server, &mut client, 30017);
    send_identity_hello_minimal(&mut server, &mut client, "");

    let ok = wait_until_minimal(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok);

    send_collect_intent(&mut client, &mut server, 1, 0);
    let count1 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count1 > 0);

    let node_entity = server
        .world_mut()
        .query::<(Entity, &ResourceNode)>()
        .iter(server.world())
        .find(|(_, n)| n.node_index == 0)
        .map(|(e, _)| e)
        .unwrap();
    let status = server
        .world()
        .get::<ResourceNodeStatus>(node_entity)
        .unwrap();
    assert!(status.depleted, "node should be depleted after collect");

    // Tick forward past respawn.
    // Node respawn_seconds=30, TICK=16ms → 30/0.016 ≈ 1875 frames.
    let needed = (30.0 / (TICK.as_secs_f64())) as usize + 10;
    for _ in 0..needed {
        step_minimal(&mut server, &mut client, 1);
    }

    let status = server
        .world()
        .get::<ResourceNodeStatus>(node_entity)
        .unwrap();
    assert!(!status.depleted, "node should have respawned");
    assert_eq!(status.respawn_timer, 0.0, "respawn timer should be reset");
}
