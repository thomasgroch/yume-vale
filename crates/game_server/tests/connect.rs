//! Integration tests: connection lifecycle — spawn, replicate, reconnect.

mod support;
use support::*;

use bevy::prelude::*;
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
