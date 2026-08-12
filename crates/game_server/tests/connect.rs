//! Integration tests: connection lifecycle — spawn, replicate, reconnect.

mod support;
use support::*;

use bevy::prelude::*;
use game_protocol::channels::ReliableChannel;
use game_protocol::{IdentityHello, PROTOCOL_ID};
use game_server::systems::NextPlayerColor;
use lightyear::connection::client::Connect;
use lightyear::crossbeam::CrossbeamIo;
use lightyear::prelude::client::RawClient;
use lightyear::prelude::server::{LinkOf, RawServer, Started};
use lightyear::prelude::*;
use player::Player;
use std::net::{Ipv4Addr, SocketAddr};

#[test]
fn connect_spawns_and_replicates_player() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20001);

    // Auth: client must send IdentityHello before server spawns a player.
    send_identity_hello(&mut server, &mut client, "");

    let ok = wait_until(&mut server, &mut client, |s, c| {
        server_player_count(s) >= 1 && client_has_player(c)
    });
    assert!(ok, "client should receive a replicated player");
    assert_eq!(server_player_count(&mut server), 1);
}

#[test]
fn reconnect_same_id_leaves_single_player() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20004);

    // Auth: first connection
    send_identity_hello(&mut server, &mut client, "test-reconnect-token");

    let ok = wait_until(&mut server, &mut client, |s, _c| {
        server_player_count(s) == 1
    });
    assert!(ok, "first connection established");

    let _player_entity = server
        .world_mut()
        .query::<(Entity, &Player)>()
        .iter(server.world())
        .next()
        .unwrap()
        .0;

    let old_client = client
        .world_mut()
        .query_filtered::<Entity, With<RawClient>>()
        .iter(client.world())
        .next()
        .unwrap();
    client.world_mut().despawn(old_client);

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 20004);
    let (new_client_io, new_server_io) = CrossbeamIo::new_pair();
    let se = server
        .world_mut()
        .query_filtered::<Entity, With<RawServer>>()
        .iter(server.world())
        .next()
        .unwrap_or_else(|| {
            let e = server.world_mut().spawn_empty().id();
            server.world_mut().entity_mut(e).insert(RawServer);
            server.world_mut().entity_mut(e).insert(Started);
            e
        });
    let _new_link_of = server
        .world_mut()
        .spawn((LinkOf { server: se }, new_server_io, PeerAddr(addr)))
        .id();
    server.world_mut().trigger(LinkStart {
        entity: _new_link_of,
    });

    let new_client = client
        .world_mut()
        .spawn((
            RawClient,
            new_client_io,
            PeerAddr(addr),
            ReplicationReceiver,
        ))
        .id();
    client.world_mut().trigger(Connect { entity: new_client });

    // Auth: reconnect uses the same token
    send_identity_hello(&mut server, &mut client, "test-reconnect-token");

    let ok = wait_until(&mut server, &mut client, |s, _c| {
        server_player_count(s) == 1
    });
    assert!(
        ok,
        "reconnect should produce exactly 1 player (stale despawned)"
    );
}

#[test]
fn second_client_reusing_same_token_evicts_first_not_shares_it() {
    // Regression test: two browser tabs sharing localStorage used to send
    // the same identity token, and the server would hand the SAME PlayerId
    // to both live connections — both clients then controlled one fox.
    // The first client's connection must be disconnected (not left
    // controlling the entity alongside the second), and exactly one player
    // must remain.
    let mut server = server_app();
    let mut client1 = client_app();
    let mut client2 = client_app();

    connect_client(&mut server, &mut client1, 20005);
    send_identity_hello(&mut server, &mut client1, "shared-token");
    let ok = wait_until(&mut server, &mut client1, |s, _c| {
        server_player_count(s) == 1
    });
    assert!(ok, "first client should authenticate");
    assert_eq!(server_client_connection_count(&mut server), 1);

    // Second client connects and authenticates with the SAME token while
    // the first client's connection is still alive (never despawned).
    connect_client(&mut server, &mut client2, 20006);
    send_identity_hello(&mut server, &mut client2, "shared-token");

    let ok = wait_until(&mut server, &mut client2, |s, _c| {
        server_player_count(s) == 1 && server_client_connection_count(s) == 1
    });
    assert!(
        ok,
        "server should end up with exactly one player and one live connection, \
         not two connections sharing the same character"
    );
}

#[test]
fn duplicate_identity_hello_produces_one_player() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20002);

    // Let connection establish so client has MessageSender<IdentityHello>.
    step(&mut server, &mut client, 10);

    // Enqueue three identical IdentityHellos without stepping between them.
    // They all arrive at the server receive buffer before one FixedUpdate.
    let mut q = client
        .world_mut()
        .query::<&mut MessageSender<IdentityHello>>();
    let mut sender = q
        .iter_mut(client.world_mut())
        .next()
        .expect("client has MessageSender after connection");
    let hello = IdentityHello {
        protocol_version: PROTOCOL_ID as u32,
        token: String::new(),
    };
    sender.send::<ReliableChannel>(hello.clone());
    sender.send::<ReliableChannel>(hello.clone());
    sender.send::<ReliableChannel>(hello);

    // Step far enough for auth, replication to settle.
    step(&mut server, &mut client, 50);

    assert_eq!(
        server_player_count(&mut server),
        1,
        "duplicate IdentityHellos must not produce duplicate players"
    );
    assert_eq!(
        server.world().resource::<NextPlayerColor>().0,
        1,
        "NextPlayerColor must advance exactly once"
    );
}
