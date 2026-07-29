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
