use bevy::prelude::*;
use core::net::SocketAddr;
use game_protocol::{PRIVATE_KEY, PROTOCOL_ID};
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
        client_timeout_secs: 10,
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
    let cid = client_id::derive_client_id();
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
            let entity = commands
                .spawn((
                    Client::default(),
                    LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
                    PeerAddr(token_addr),
                    Link::new(None),
                    client,
                    ReplicationReceiver,
                    WebSocketClientIo::from_url(aeronet_websocket::client::ClientConfig, url),
                ))
                .id();
            commands.entity(entity).trigger(|e| Connect { entity: e });
            return;
        }

        if transport.explicit_ws_override || transport.mode == TransportMode::WebSocket {
            if let Some(wss_url) = transport.prod_wss_url() {
                let Some(token_addr) = parse_addr(&config.websocket_addr, "websocket_addr") else {
                    return;
                };
                let Some(client) = build_netcode_client(token_addr, cid, &netcode_config) else {
                    return;
                };
                let entity = commands
                    .spawn((
                        Client::default(),
                        LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
                        PeerAddr(token_addr),
                        Link::new(None),
                        client,
                        ReplicationReceiver,
                        WebSocketClientIo::from_url(
                            aeronet_websocket::client::ClientConfig,
                            wss_url,
                        ),
                    ))
                    .id();
                commands.entity(entity).trigger(|e| Connect { entity: e });
                return;
            }

            let addr_string = config.websocket_addr.clone();
            let Some(addr) = parse_addr(&addr_string, "websocket_addr") else {
                return;
            };
            let Some(client) = build_netcode_client(addr, cid, &netcode_config) else {
                return;
            };
            let entity = commands
                .spawn((
                    Client::default(),
                    LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
                    PeerAddr(addr),
                    Link::new(None),
                    client,
                    ReplicationReceiver,
                    WebSocketClientIo::from_addr(
                        aeronet_websocket::client::ClientConfig,
                        WebSocketScheme::Plain,
                    ),
                ))
                .id();
            commands.entity(entity).trigger(|e| Connect { entity: e });
            return;
        }

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

        let entity = commands
            .spawn((
                Client::default(),
                LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
                PeerAddr(addr),
                Link::new(None),
                client,
                ReplicationReceiver,
                WebTransportClientIo {
                    certificate_digest: digest,
                },
            ))
            .id();
        commands.entity(entity).trigger(|e| Connect { entity: e });
        return;
    };
    commands.entity(entity).trigger(|e| Connect { entity: e });
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::net::SocketAddr;
    use lightyear::netcode::client_plugin::NetcodeConfig;

    #[test]
    fn parse_addr_valid_ipv4() {
        let addr = parse_addr("127.0.0.1:5000", "test");
        assert!(addr.is_some());
        assert_eq!(addr.unwrap().port(), 5000);
    }

    #[test]
    fn parse_addr_valid_ipv6() {
        let addr = parse_addr("[::1]:5001", "test");
        assert!(addr.is_some());
        assert_eq!(addr.unwrap().port(), 5001);
    }

    #[test]
    fn parse_addr_invalid_format_returns_none() {
        let addr = parse_addr("not-an-addr", "test");
        assert!(addr.is_none());
    }

    #[test]
    fn build_netcode_client_creates_ok() {
        let addr: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let cfg = NetcodeConfig {
            client_timeout_secs: 10,
            token_expire_secs: -1,
            ..Default::default()
        };
        let client = build_netcode_client(addr, 42, &cfg);
        assert!(client.is_some(), "NetcodeClient should be created");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn start_connection_spawns_client_entity() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let config = ClientConfig::default();
        let mut commands = app.world_mut().commands();
        let mut transport = TransportState::default();
        start_connection(&mut commands, &config, &mut transport, 0.0);
        app.update();
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<Client>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "start_connection must spawn a Client entity");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn start_connection_respects_server_addr_env() {
        unsafe {
            std::env::set_var("YUME_SERVER_ADDR", "10.0.0.1:5000");
        }
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let config = ClientConfig::default();
        let mut commands = app.world_mut().commands();
        let mut transport = TransportState::default();
        start_connection(&mut commands, &config, &mut transport, 0.0);
        app.update();
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<Client>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "must still spawn Client with env override");
        unsafe {
            std::env::remove_var("YUME_SERVER_ADDR");
        }
    }
}
