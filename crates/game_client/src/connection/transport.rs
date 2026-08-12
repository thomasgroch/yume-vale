use bevy::prelude::*;
use core::net::SocketAddr;
use game_protocol::{CLIENT_TIMEOUT_SECS, PRIVATE_KEY, PROTOCOL_ID};
use lightyear::connection::client::Connect;
use lightyear::netcode::{auth::Authentication, client_plugin::NetcodeConfig};
use lightyear::prelude::client::NetcodeClient;
use lightyear::prelude::*;

#[cfg(target_arch = "wasm32")]
use lightyear::prelude::client::{WebSocketClientIo, WebSocketScheme, WebTransportClientIo};

use crate::config::ClientConfig;

use super::client_id;
use super::transport_fallback::TransportState;
#[cfg(target_arch = "wasm32")]
use super::transport_fallback::{TransportMode, derive_wasm_addr};

pub(crate) fn netcode_config() -> NetcodeConfig {
    NetcodeConfig {
        client_timeout_secs: CLIENT_TIMEOUT_SECS,
        token_expire_secs: -1,
        ..Default::default()
    }
}

pub(crate) fn parse_addr(addr: &str, field: &str) -> Option<SocketAddr> {
    match addr.parse() {
        Ok(addr) => Some(addr),
        Err(e) => {
            error!("invalid {field} in ClientConfig ({addr:?}): {e}");
            None
        }
    }
}

pub(crate) fn build_netcode_client(
    addr: SocketAddr,
    cid: u64,
    config: &NetcodeConfig,
) -> Option<NetcodeClient> {
    let auth = Authentication::Manual {
        server_addr: addr,
        client_id: cid,
        private_key: PRIVATE_KEY,
        protocol_id: PROTOCOL_ID,
    };
    match NetcodeClient::new(auth, config.clone()) {
        Ok(client) => Some(client),
        Err(e) => {
            error!("failed to create NetcodeClient: {e}");
            None
        }
    }
}

pub(crate) fn start_connection(
    commands: &mut Commands,
    config: &ClientConfig,
    #[allow(unused_variables)] transport: &mut TransportState,
    #[allow(unused_variables)] now_seconds: f64,
) {
    let Some(cid) = client_id::derive_client_id() else {
        return;
    };
    let netcode_config = netcode_config();

    #[cfg(not(target_arch = "wasm32"))]
    let entity = {
        let Some(addr) = parse_addr(&config.server_addr, "server_addr") else {
            return;
        };
        let Some(client) = build_netcode_client(addr, cid, &netcode_config) else {
            return;
        };
        commands
            .spawn((
                Client::default(),
                LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
                PeerAddr(addr),
                Link::new(None),
                client,
                UdpIo::default(),
                ReplicationReceiver,
                PredictionManager::default(),
            ))
            .id()
    };

    #[cfg(target_arch = "wasm32")]
    let entity = {
        let explicit_url = option_env!("YUME_SERVER_WS_URL")
            .filter(|u| !u.is_empty())
            .map(str::to_string);

        if let Some(url) = explicit_url {
            let Some(token_addr) = parse_addr(&config.websocket_addr, "websocket_addr") else {
                return;
            };
            let Some(client) = build_netcode_client(token_addr, cid, &netcode_config) else {
                return;
            };
            commands
                .spawn((
                    Client::default(),
                    LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
                    PeerAddr(token_addr),
                    Link::new(None),
                    client,
                    ReplicationReceiver,
                    PredictionManager::default(),
                    WebSocketClientIo::from_url(aeronet_websocket::client::ClientConfig, url),
                ))
                .id()
        } else if transport.explicit_ws_override || transport.mode == TransportMode::WebSocket {
            if let Some(wss_url) = transport.prod_wss_url() {
                let Some(token_addr) = parse_addr(&config.websocket_addr, "websocket_addr") else {
                    return;
                };
                let Some(client) = build_netcode_client(token_addr, cid, &netcode_config) else {
                    return;
                };
                commands
                    .spawn((
                        Client::default(),
                        LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
                        PeerAddr(token_addr),
                        Link::new(None),
                        client,
                        ReplicationReceiver,
                        PredictionManager::default(),
                        WebSocketClientIo::from_url(
                            aeronet_websocket::client::ClientConfig,
                            wss_url,
                        ),
                    ))
                    .id()
            } else {
                let addr_string = config.websocket_addr.clone();
                let Some(addr) = parse_addr(&addr_string, "websocket_addr") else {
                    return;
                };
                let Some(client) = build_netcode_client(addr, cid, &netcode_config) else {
                    return;
                };
                commands
                    .spawn((
                        Client::default(),
                        LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
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
                    .id()
            }
        } else {
            let page_host = transport.page_host.as_deref();
            let is_local = transport.page_is_local;
            let wt_port_override = option_env!("YUME_TEST_WT_PORT");
            let template = match wt_port_override {
                Some(port) => format!("127.0.0.1:{port}"),
                None => config.web_transport_addr.clone(),
            };
            let addr_string = derive_wasm_addr(&template, is_local, page_host, "5001");
            let Some(addr) = parse_addr(&addr_string, "web_transport_addr") else {
                return;
            };
            let Some(client) = build_netcode_client(addr, cid, &netcode_config) else {
                return;
            };
            let digest = if is_local {
                include_str!("../../../../certs/digest.txt")
                    .trim()
                    .to_string()
            } else {
                String::new()
            };

            transport.start_wt_attempt(now_seconds);

            commands
                .spawn((
                    Client::default(),
                    LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
                    PeerAddr(addr),
                    Link::new(None),
                    client,
                    ReplicationReceiver,
                    PredictionManager::default(),
                    WebTransportClientIo {
                        certificate_digest: digest,
                    },
                ))
                .id()
        }
    };
    commands.entity(entity).trigger(|e| Connect { entity: e });
}

#[cfg(test)]
mod tests;
