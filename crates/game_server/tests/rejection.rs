//! S3 rejection/cooldown tests: protocol mismatch, capacity, cooldown rejection.
//!
//! Verifies that:
//! - Wrong protocol version → no player spawned (rejected at auth)
//! - Server full (MAX_PLAYERS clients) → 17th connection ignored
//! - Action too fast (cooldown active) → second collect rejected
//! - Depleted resource → collect rejected

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use core::time::Duration;

use game_core::actions::ActionKind;
use game_core::constants::MAX_PLAYERS;
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
use game_server::systems::connection::ClientPlayer;
use game_server::systems::persistence::PersistenceCoordinator;
use game_server::systems::setup::WorldConfigResource;
use game_server::systems::{
    NextPlayerColor, ServerSystems, apply_client_input, handle_action_intent,
    handle_new_client_link, initialize_player_components, tick_player_cooldowns,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TICK: Duration = Duration::from_millis(16);
const MAX_FRAMES: usize = 400;

// ---------------------------------------------------------------------------
// App builders with resources for cooldown/range/depletion tests
// ---------------------------------------------------------------------------

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

fn resource_server_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
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

fn server_player_count(server: &mut App) -> usize {
    server
        .world_mut()
        .query::<&Player>()
        .iter(server.world())
        .count()
}

fn wait_until_custom<F>(server: &mut App, client: &mut App, mut cond: F) -> bool
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
// Tests
// ---------------------------------------------------------------------------

/// Wrong PROTOCOL_ID → no player is spawned (rejected at auth).
#[test]
fn wrong_protocol_rejected() {
    let mut server = resource_server_app();
    let mut client = client_app();

    connect_client(&mut server, &mut client, 53001);
    step(&mut server, &mut client, 10);

    // Send IdentityHello with wrong protocol version
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<IdentityHello>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(IdentityHello {
            protocol_version: 0xdead_beef,
            token: "".to_string(),
        });
    }
    step(&mut server, &mut client, 10);

    assert_eq!(
        server_player_count(&mut server),
        0,
        "wrong protocol should not spawn a player"
    );
}

/// 17th connection attempt (server already at MAX_PLAYERS) is rejected.
#[test]
fn server_full_rejected() {
    let mut server = resource_server_app();
    let mut clients: Vec<App> = Vec::new();
    let base_port = 53100;

    // Fill the server with MAX_PLAYERS authenticated clients
    for i in 0..MAX_PLAYERS {
        let mut c = client_app();
        connect_client(&mut server, &mut c, base_port + i as u16);
        send_identity_hello(&mut server, &mut c, &format!("full-test-{i}"));
        clients.push(c);
    }

    // Wait until all MAX_PLAYERS players have spawned.
    let all_spawned = wait_until_custom(&mut server, &mut clients[0], |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= MAX_PLAYERS
    });
    assert!(
        all_spawned,
        "should have {MAX_PLAYERS} authenticated players"
    );

    // Attempt a 17th connection
    let mut overflow = client_app();
    let overflow_port = base_port + MAX_PLAYERS as u16;
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), overflow_port);
    let (client_io, server_io) = CrossbeamIo::new_pair();
    let se = server
        .world_mut()
        .query_filtered::<Entity, With<RawServer>>()
        .iter(server.world())
        .next()
        .expect("raw server exists");
    let _lo = server
        .world_mut()
        .spawn((LinkOf { server: se }, server_io, PeerAddr(addr)))
        .id();
    server.world_mut().trigger(LinkStart { entity: _lo });
    let ce = overflow
        .world_mut()
        .spawn((RawClient, client_io, PeerAddr(addr), ReplicationReceiver))
        .id();
    overflow.world_mut().trigger(Connect { entity: ce });

    step(&mut server, &mut overflow, 15);

    let auth_count = server
        .world_mut()
        .query::<&ClientPlayer>()
        .iter(server.world())
        .count();
    assert_eq!(auth_count, MAX_PLAYERS, "17th client should be rejected");
    drop(overflow);
}

/// Cooldown-enforced rejection: second collect while cooldown is active
/// does not increase inventory.
#[test]
fn cooldown_rejects_action_too_fast() {
    let mut server = resource_server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 53200);
    send_identity_hello(&mut server, &mut client, "");

    let ok = wait_until_custom(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should spawn");

    // First collect succeeds
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<ActionIntent>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(ActionIntent {
            sequence: 1,
            kind: ActionKind::Collect,
            target_id: Some(0),
        });
    }
    step(&mut server, &mut client, 5);

    let count1: u32 = server
        .world_mut()
        .query::<&PlayerInventory>()
        .iter(server.world())
        .flat_map(|inv| &inv.inventory.slots)
        .filter_map(|s| s.as_ref())
        .filter(|stack| stack.kind == ItemKind::Resource(ResourceKind::Wood))
        .map(|stack| stack.quantity)
        .sum();
    assert!(count1 > 0, "first collect should succeed");

    // Send second collect immediately (cooldown still active)
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(ActionIntent {
            sequence: 2,
            kind: ActionKind::Collect,
            target_id: Some(0),
        });
    }
    step(&mut server, &mut client, 3);

    let count2: u32 = server
        .world_mut()
        .query::<&PlayerInventory>()
        .iter(server.world())
        .flat_map(|inv| &inv.inventory.slots)
        .filter_map(|s| s.as_ref())
        .filter(|stack| stack.kind == ItemKind::Resource(ResourceKind::Wood))
        .map(|stack| stack.quantity)
        .sum();

    assert_eq!(
        count1, count2,
        "cooldown should reject second collect, inventory unchanged"
    );
}

/// Collect on a depleted resource node is rejected.
#[test]
fn action_rejected_for_depleted_resource() {
    let mut server = resource_server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 53300);
    send_identity_hello(&mut server, &mut client, "");

    let ok = wait_until_custom(&mut server, &mut client, |s, _c| {
        s.world_mut().query::<&Player>().iter(s.world()).count() >= 1
    });
    assert!(ok, "player should spawn");

    // First collect succeeds (depletes the node)
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<ActionIntent>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(ActionIntent {
            sequence: 1,
            kind: ActionKind::Collect,
            target_id: Some(0),
        });
    }
    step(&mut server, &mut client, 5);

    let count1: u32 = server
        .world_mut()
        .query::<&PlayerInventory>()
        .iter(server.world())
        .flat_map(|inv| &inv.inventory.slots)
        .filter_map(|s| s.as_ref())
        .filter(|stack| stack.kind == ItemKind::Resource(ResourceKind::Wood))
        .map(|stack| stack.quantity)
        .sum();
    assert!(count1 > 0, "first collect should succeed");

    // Second collect on depleted node should be rejected
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(ActionIntent {
            sequence: 2,
            kind: ActionKind::Collect,
            target_id: Some(0),
        });
    }
    step(&mut server, &mut client, 3);

    let count2: u32 = server
        .world_mut()
        .query::<&PlayerInventory>()
        .iter(server.world())
        .flat_map(|inv| &inv.inventory.slots)
        .filter_map(|s| s.as_ref())
        .map(|s| s.quantity)
        .sum();
    assert_eq!(
        count2, count1,
        "depleted node collect should be rejected, inventory unchanged"
    );
}
