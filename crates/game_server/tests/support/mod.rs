//! Shared test harness for game_server integration tests.
//!
//! `dead_code` allowed because each `tests/*.rs` crate is compiled separately
//! — functions unused in one binary are still used by others.
#![allow(dead_code)]
//!
//! Provides builders (`server_app`, `client_app`), a connection helper
//! (`connect_client`), single-frame stepping (`step`), polling (`wait_until`),
//! and convenience query wrappers.
//!
//! ## Invariants preserved
//! - `ServerPlugins` registered before `ProtocolPlugin`
//! - `app.finish()` called before any `app.update()`
//! - `Started` inserted manually on the `RawServer` entity
//! - Crossbeam transport + `PeerAddr` / `PeerId` derivation unchanged

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use core::time::Duration;
use game_protocol::{IdentityHello, PROTOCOL_ID, PlayerColor, PlayerPosition, ProtocolPlugin};
use game_server::systems::auth;
use game_server::systems::setup::WorldConfigResource;
use game_server::systems::{
    NextPlayerColor, ServerSystems, apply_client_input, handle_new_client_link,
};
use lightyear::connection::client::Connect;
use lightyear::crossbeam::CrossbeamIo;
use lightyear::prelude::client::{ClientPlugins, RawClient};
use lightyear::prelude::server::{LinkOf, RawServer, ServerPlugins, Started};
use lightyear::prelude::*;
use player::{Player, PlayerPlugin};
use resources::systems::{spawn_resource_nodes, tick_resource_respawn};
use social::SocialPlugin;
use std::net::{Ipv4Addr, SocketAddr};

/// Tick duration used by all test apps (≈60 Hz).
pub const TICK: Duration = Duration::from_millis(16);

/// Maximum frames to poll inside [`wait_until`] before giving up.
pub const MAX_FRAMES: usize = 400;

/// Build a server `App` with protocol, player, auth, and core server systems.
pub fn server_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(avian3d::PhysicsPlugins::default());
    app.add_plugins(ServerPlugins {
        tick_duration: TICK,
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin, SocialPlugin));
    app.init_resource::<NextPlayerColor>();
    app.init_resource::<game_server::systems::persistence::PersistenceCoordinator>();
    // World config resource
    app.insert_resource(WorldConfigResource(
        game_core::world_config::WorldConfig::default(),
    ));
    app.add_observer(handle_new_client_link);
    app.add_observer(auth::on_client_connected);
    app.add_systems(
        FixedUpdate,
        (
            auth::handle_identity_hello,
            apply_client_input,
            game_server::systems::handle_action_intent,
            game_server::systems::initialize_player_components,
            game_server::systems::tick_player_cooldowns,
            tick_resource_respawn,
        )
            .in_set(ServerSystems),
    );
    app.add_systems(
        PostStartup,
        |commands: Commands, config: Res<WorldConfigResource>| {
            spawn_resource_nodes(commands, &config.0);
        },
    );
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(TICK));
    app.finish();
    app
}

/// Build a client `App` with protocol, player, and client plugins.
pub fn client_app() -> App {
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

/// Advance both `server` and `client` by `frames` ticks each.
pub fn step(server: &mut App, client: &mut App, frames: usize) {
    for _ in 0..frames {
        server.update();
        client.update();
    }
}

/// Wire up a server/client pair over an in-memory Crossbeam channel.
///
/// Spawns the required lightyear entities (`RawServer` + `Started` on the
/// server side, `LinkOf` for the transport link, `RawClient` +
/// `ReplicationReceiver` on the client side) and triggers the appropriate
/// startup events (`LinkStart`, `Connect`).
pub fn connect_client(server: &mut App, client: &mut App, port: u16) {
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

/// Send `IdentityHello` from the client to the server.
///
/// The client must have an active `MessageSender<IdentityHello>` (i.e. the
/// connection must have been established after `connect_client`).
pub fn send_identity_hello(server: &mut App, client: &mut App, token: &str) {
    use game_protocol::channels::ReliableChannel;
    use lightyear::prelude::MessageSender;

    // Step both sides to let the connection establish.
    // The netcode handshake may need several frames.
    step(server, client, 10);

    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<IdentityHello>>();
    let found = query.iter_mut(client.world_mut()).next().is_some();
    if found {
        let mut q2 = client
            .world_mut()
            .query::<&mut MessageSender<IdentityHello>>();
        if let Some(mut sender) = q2.iter_mut(client.world_mut()).next() {
            sender.send::<ReliableChannel>(IdentityHello {
                protocol_version: PROTOCOL_ID as u32,
                token: token.to_string(),
            });
        }
    }

    // Flush the send queue so the server receives the IdentityHello.
    step(server, client, 3);
}

/// Number of `Player` entities on the server.
pub fn server_player_count(s: &mut App) -> usize {
    s.world_mut().query::<&Player>().iter(s.world()).count()
}

/// Whether the client world has at least one entity with `Player`,
/// `PlayerColor`, and `PlayerPosition` components.
pub fn client_has_player(c: &mut App) -> bool {
    c.world_mut()
        .query_filtered::<Entity, (With<Player>, With<PlayerColor>, With<PlayerPosition>)>()
        .iter(c.world())
        .next()
        .is_some()
}

/// Run up to `MAX_FRAMES` frames of `step` waiting for `cond` to return true.
///
/// Returns `true` if the condition became true within the frame budget,
/// `false` otherwise (the caller should then `assert!` with a descriptive
/// message).
pub fn wait_until<F>(server: &mut App, client: &mut App, mut cond: F) -> bool
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
