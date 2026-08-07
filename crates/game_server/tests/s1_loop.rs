//! Full S1 loop: connect → auth → collect → verify inventory snapshot.
//! Also tests reliable action delivery and monotonic sequence enforcement.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use core::time::Duration;
use game_core::actions::ActionKind;
use game_core::constants::INTERACT_RADIUS;
use game_core::inventory::ItemKind;
use game_core::resources::ResourceKind;
use game_core::world_config::{ResourceConfig, WorldConfig};
use game_protocol::channels::ReliableChannel;
use game_protocol::messages::ActionIntent;
use game_protocol::{IdentityHello, PROTOCOL_ID, ProtocolPlugin};
use lightyear::connection::client::Connect;
use lightyear::crossbeam::CrossbeamIo;
use lightyear::prelude::client::{ClientPlugins, RawClient};
use lightyear::prelude::server::{LinkOf, RawServer, ServerPlugins, Started};
use lightyear::prelude::*;
use player::{Player, PlayerPlugin};
use resources::components::PlayerInventory;
use std::net::{Ipv4Addr, SocketAddr};

use game_server::systems::auth;
use game_server::systems::persistence::PersistenceCoordinator;
use game_server::systems::setup::WorldConfigResource;
use game_server::systems::{
    NextPlayerColor, ServerSystems, apply_client_input, handle_action_intent,
    handle_new_client_link, initialize_player_components, tick_player_cooldowns,
};

// ---------------------------------------------------------------------------
// Constants and test config
// ---------------------------------------------------------------------------

const TICK: Duration = Duration::from_millis(16);
const MAX_FRAMES: usize = 400;

fn resource_test_config() -> WorldConfig {
    WorldConfig {
        resources: vec![ResourceConfig {
            id: game_core::id::ResourceId::new(1),
            kind: ResourceKind::Wood,
            count: 1,
            yield_amount: 2,
            respawn_seconds: 30.0,
            positions: vec![Vec3::new(1.0, 0.0, 0.0)],
            model_path: "wood.glb".into(),
        }],
        creatures: vec![],
    }
}

fn server_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(avian3d::PhysicsPlugins::default());
    app.add_plugins(ServerPlugins {
        tick_duration: TICK,
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin));
    app.init_resource::<NextPlayerColor>();
    app.init_resource::<PersistenceCoordinator>();
    app.insert_resource(WorldConfigResource(resource_test_config()));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(TICK));
    app.add_observer(handle_new_client_link);
    app.add_observer(auth::on_client_connected);
    app.add_systems(
        FixedUpdate,
        (
            auth::handle_identity_hello,
            apply_client_input,
            handle_action_intent,
            initialize_player_components,
            tick_player_cooldowns,
            resources::systems::tick_resource_respawn,
        )
            .in_set(ServerSystems),
    );
    app.add_systems(
        PostStartup,
        |commands: Commands, config: Res<WorldConfigResource>| {
            resources::systems::spawn_resource_nodes(commands, &config.0);
        },
    );
    app.finish();
    app
}

fn client_app() -> App {
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

fn step(server: &mut App, client: &mut App, frames: usize) {
    for _ in 0..frames {
        server.update();
        client.update();
    }
}

fn connect_client(server: &mut App, client: &mut App, port: u16) {
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

fn send_identity_hello(server: &mut App, client: &mut App, token: &str) {
    step(server, client, 10);
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<IdentityHello>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(IdentityHello {
            protocol_version: PROTOCOL_ID as u32,
            token: token.to_string(),
        });
    }
    step(server, client, 3);
}

fn wait_until<F>(server: &mut App, client: &mut App, mut cond: F) -> bool
where
    F: FnMut(&mut App, &mut App) -> bool,
{
    for _ in 0..MAX_FRAMES {
        if cond(server, client) {
            return true;
        }
        step(server, client, 1);
    }
    cond(server, client)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Count items of a given kind across all server-side PlayerInventory components.
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

/// Send a collect ActionIntent from client → server with given sequence and target.
fn send_collect(client: &mut App, server: &mut App, sequence: u64, target_id: u64) {
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<ActionIntent>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(ActionIntent {
            sequence,
            kind: ActionKind::Collect,
            target_id: Some(target_id),
        });
    }
    step(server, client, 3);
}

// Move player far away so they're out of interact range.
fn move_player_far(server: &mut App) {
    let player_entity = server
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .iter(server.world())
        .next()
        .expect("player exists");
    server
        .world_mut()
        .entity_mut(player_entity)
        .insert(Transform::from_translation(Vec3::new(
            INTERACT_RADIUS + 10.0,
            0.0,
            0.0,
        )));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Full S1 loop: a fresh player connects, authenticates, collects a resource,
/// and their inventory is updated server-side.
#[test]
fn full_collect_loop_adds_to_inventory() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 50001);
    send_identity_hello(&mut server, &mut client, "");

    // Wait for player to spawn
    let ok = wait_until(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should spawn");

    // Collect resource at node index 0
    send_collect(&mut client, &mut server, 1, 0);

    let count = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(
        count > 0,
        "inventory should have items after collect, got {count}"
    );
}

/// Reliable channel delivery: sending an ActionIntent via the reliable channel
/// results in the server processing it (no message loss across Crossbeam).
#[test]
fn reliable_action_delivery_processed() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 50002);
    send_identity_hello(&mut server, &mut client, "");

    let ok = wait_until(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should spawn");

    // Send 3 collect actions at distinct sequences
    send_collect(&mut client, &mut server, 1, 0);
    let count1 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count1 > 0, "first collect should succeed");

    // Deplete node by restoring it after first collect
    let node_entity = server
        .world_mut()
        .query::<(Entity, &resources::components::ResourceNode)>()
        .iter(server.world())
        .find(|(_, n)| n.node_index == 0)
        .map(|(e, _)| e)
        .expect("resource node exists");

    // Reset the node
    if let Some(mut status) = server
        .world_mut()
        .get_mut::<resources::components::ResourceNodeStatus>(node_entity)
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
    // Reset cooldown and sequence
    let player_entity = server
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .iter(server.world())
        .next()
        .expect("player exists");
    if let Some(mut cd) = server
        .world_mut()
        .get_mut::<resources::components::InteractionCooldown>(player_entity)
    {
        cd.active = false;
    }

    // Second collect should also succeed
    send_collect(&mut client, &mut server, 2, 0);
    let count2 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count2 > count1, "second collect should also succeed");
}

/// Server rejects non-monotonic (stale) action sequences.
#[test]
fn stale_sequence_rejected() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 50003);
    send_identity_hello(&mut server, &mut client, "");

    let ok = wait_until(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should spawn");

    // First collect succeeds
    send_collect(&mut client, &mut server, 1, 0);
    let count1 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert!(count1 > 0, "first collect succeeds");

    // Restore node
    let node_entity = server
        .world_mut()
        .query::<(Entity, &resources::components::ResourceNode)>()
        .iter(server.world())
        .find(|(_, n)| n.node_index == 0)
        .map(|(e, _)| e)
        .expect("node exists");
    if let Some(mut status) = server
        .world_mut()
        .get_mut::<resources::components::ResourceNodeStatus>(node_entity)
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
    // Reset cooldown for the duplicate-sequence attempt
    let player_entity = server
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .iter(server.world())
        .next()
        .expect("player exists");
    if let Some(mut cd) = server
        .world_mut()
        .get_mut::<resources::components::InteractionCooldown>(player_entity)
    {
        cd.active = false;
    }

    // Re-send sequence=1 (stale) — should be rejected
    send_collect(&mut client, &mut server, 1, 0);
    let count2 = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert_eq!(
        count1, count2,
        "stale sequence should be rejected, inventory unchanged"
    );
}

/// Out-of-range collect is rejected (INTERACT_RADIUS enforcement).
#[test]
fn out_of_range_collect_rejected() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 50004);
    send_identity_hello(&mut server, &mut client, "");

    let ok = wait_until(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should spawn");

    move_player_far(&mut server);

    send_collect(&mut client, &mut server, 1, 0);
    let count = server_inventory_count(&mut server, ItemKind::Resource(ResourceKind::Wood));
    assert_eq!(count, 0, "out-of-range collect should be rejected");
}
