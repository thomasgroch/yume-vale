//! Wasm-only fallback runtime: WebTransport → WebSocket.
//!
//! This module implements the actual fallback system and WS entity spawning
//! that only exist on wasm targets. Parent module re-exports
//! [`handle_transport_fallback`] as `pub(crate)`.

use bevy::prelude::*;

use lightyear::prelude::client::{WebSocketClientIo, WebSocketScheme, WebTransportClientIo};
use lightyear::prelude::{
    Client, Connect, Connected, Link, LocalAddr, PeerAddr, PredictionManager, ReplicationReceiver,
};

use super::super::{client_id, transport};
use super::{TransportMode, TransportState, wt_timed_out};

/// Detect WebTransport failure and switch to WebSocket.
pub(crate) fn handle_transport_fallback(
    mut commands: Commands,
    mut state: ResMut<TransportState>,
    time: Res<Time>,
    clients: Query<(Entity, Option<&Connected>, Has<WebTransportClientIo>)>,
    config: Res<crate::config::ClientConfig>,
) {
    if state.mode == TransportMode::WebSocket {
        return;
    }

    if state.rejection_received {
        state.wt_start = None;
        return;
    }

    let mut wt_entity: Option<Entity> = None;

    for (entity, connected, has_wt) in clients.iter() {
        if has_wt {
            if connected.is_some() {
                state.wt_start = None;
                return;
            }
            wt_entity = Some(entity);
        }
    }

    let Some(entity) = wt_entity else {
        return;
    };

    let now = time.elapsed_secs_f64();
    let start = match state.wt_start {
        Some(s) => s,
        None => {
            state.wt_start = Some(now);
            return;
        }
    };

    if !wt_timed_out(state.wt_start, now) {
        return;
    }

    info!(
        "WebTransport attempt timed out after {:.1}s — falling back to WebSocket",
        now - start
    );

    commands.entity(entity).despawn();
    state.mode = TransportMode::WebSocket;
    state.wt_start = None;
    spawn_ws_client(&mut commands, &config, &state);
}

/// Spawn a client entity with WebSocket IO (either local WS or production WSS).
///
/// Exposed to `transport::start_connection` so a WebTransport attempt whose
/// address can't even be parsed (see its call site) can fall back to WS
/// immediately instead of silently leaving the client with no connection
/// entity at all.
pub(crate) fn spawn_ws_client(
    commands: &mut Commands,
    config: &crate::config::ClientConfig,
    state: &TransportState,
) {
    let Some(cid) = client_id::derive_client_id() else {
        return;
    };
    let netcode_config = transport::netcode_config();

    // Try production WSS URL first.
    if let Some(wss_url) = state.prod_wss_url() {
        let Some(token_addr) = transport::parse_addr(&config.websocket_addr, "websocket_addr")
        else {
            return;
        };
        let Some(client) = transport::build_netcode_client(token_addr, cid, &netcode_config) else {
            return;
        };
        let entity = commands
            .spawn((
                Client::default(),
                LocalAddr(core::net::SocketAddr::from(([0, 0, 0, 0], 0))),
                PeerAddr(token_addr),
                Link::new(None),
                client,
                ReplicationReceiver,
                PredictionManager::default(),
                WebSocketClientIo::from_url(aeronet_websocket::client::ClientConfig, wss_url),
            ))
            .id();
        commands.entity(entity).trigger(|e| Connect { entity: e });
        return;
    }

    let addr_string = config.websocket_addr.clone();
    let Some(addr) = transport::parse_addr(&addr_string, "websocket_addr") else {
        return;
    };
    let Some(client) = transport::build_netcode_client(addr, cid, &netcode_config) else {
        return;
    };
    let entity = commands
        .spawn((
            Client::default(),
            LocalAddr(core::net::SocketAddr::from(([0, 0, 0, 0], 0))),
            PeerAddr(addr),
            Link::new(None),
            client,
            ReplicationReceiver,
            PredictionManager::default(),
            WebSocketClientIo::from_addr(
                aeronet_websocket::client::ClientConfig,
                WebSocketScheme::Plain,
            ),
        ))
        .id();
    commands.entity(entity).trigger(|e| Connect { entity: e });
}
