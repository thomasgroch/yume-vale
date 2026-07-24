use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use core::time::Duration;
use game_protocol::{PlayerColor, PlayerPosition, ProtocolPlugin};
use game_server::systems::{
    NextPlayerColor, ServerSystems, apply_client_input, handle_new_client_link, on_client_connected,
};
use lightyear::connection::client::Connect;
use lightyear::crossbeam::CrossbeamIo;
use lightyear::prelude::client::{ClientPlugins, RawClient};
use lightyear::prelude::server::{LinkOf, RawServer, ServerPlugins, Started};
use lightyear::prelude::*;
use player::{Player, PlayerPlugin};
use std::net::{Ipv4Addr, SocketAddr};

const TICK: Duration = Duration::from_millis(16);
const MAX_FRAMES: usize = 400;

fn server_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(ServerPlugins {
        tick_duration: TICK,
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin));
    app.init_resource::<NextPlayerColor>();
    app.init_resource::<game_server::systems::WalkConfig>();
    app.add_observer(handle_new_client_link);
    app.add_observer(on_client_connected);
    app.add_systems(FixedUpdate, apply_client_input.in_set(ServerSystems));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(TICK));
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
    server.world_mut().entity_mut(se).insert(RawServer);
    server.world_mut().entity_mut(se).insert(Started);
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

fn server_player_count(s: &mut App) -> usize {
    s.world_mut().query::<&Player>().iter(s.world()).count()
}

fn client_has_player(c: &mut App) -> bool {
    c.world_mut()
        .query_filtered::<Entity, (With<Player>, With<PlayerColor>, With<PlayerPosition>)>()
        .iter(c.world())
        .next()
        .is_some()
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

#[test]
fn replicated_player_has_interpolated_marker() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20004);

    let ok = wait_until(&mut server, &mut client, |_s, c| {
        c.world_mut()
            .query_filtered::<Entity, (With<Player>, With<Interpolated>)>()
            .iter(c.world())
            .count()
            >= 1
    });
    assert!(
        ok,
        "client player entity should have the Interpolated marker"
    );
}

#[test]
fn connect_spawns_and_replicates_player() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20001);

    let ok = wait_until(&mut server, &mut client, |s, c| {
        server_player_count(s) >= 1 && client_has_player(c)
    });
    assert!(ok, "client should receive a replicated player");
    assert_eq!(server_player_count(&mut server), 1);
}

#[test]
fn two_clients_get_distinct_colors_and_see_each_other() {
    let mut server = server_app();
    let mut client1 = client_app();
    let mut client2 = client_app();
    connect_client(&mut server, &mut client1, 20002);
    connect_client(&mut server, &mut client2, 20003);

    let ok = wait_until(&mut server, &mut client1, |s, _c| {
        server_player_count(s) == 2
    });
    assert!(ok, "server should have 2 players");

    let colors: Vec<PlayerColor> = server
        .world_mut()
        .query::<&PlayerColor>()
        .iter(server.world())
        .copied()
        .collect();
    assert!(colors.contains(&PlayerColor(0)));
    assert!(colors.contains(&PlayerColor(1)));
    assert_eq!(colors.len(), 2);

    wait_until(&mut server, &mut client1, |_s, c| client_has_player(c));
    wait_until(&mut server, &mut client2, |_s, c| client_has_player(c));

    for (i, client) in [&mut client1, &mut client2].iter_mut().enumerate() {
        let client_colors: Vec<PlayerColor> = client
            .world_mut()
            .query::<&PlayerColor>()
            .iter(client.world())
            .copied()
            .collect();
        assert_eq!(client_colors.len(), 2, "client {i} sees 2 players");
        assert!(client_colors.contains(&PlayerColor(0)));
        assert!(client_colors.contains(&PlayerColor(1)));
    }
}

#[test]
fn reconnect_same_id_leaves_single_player() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20004);

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

    let ok = wait_until(&mut server, &mut client, |s, _c| {
        server_player_count(s) == 1
    });
    assert!(
        ok,
        "reconnect should produce exactly 1 player (stale despawned)"
    );
}
