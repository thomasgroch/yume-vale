use bevy::prelude::Resource;

#[derive(Debug, Clone, Resource)]
pub struct ClientConfig {
    /// Native server address in `host:port` format (UDP, default `127.0.0.1:5000`).
    pub server_addr: String,
    /// WebTransport address for browser clients (default `127.0.0.1:5001`).
    pub web_transport_addr: String,
    /// WebSocket fallback address for browser clients (default `127.0.0.1:5002`).
    pub websocket_addr: String,
    /// Display name sent to the server on connect.
    pub player_name: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:5000".to_string(),
            web_transport_addr: "127.0.0.1:5001".to_string(),
            websocket_addr: "127.0.0.1:5002".to_string(),
            player_name: "Player".to_string(),
        }
    }
}

pub fn build_client_config(server_addr: &str, player_name: &str) -> ClientConfig {
    ClientConfig {
        server_addr: server_addr.to_string(),
        player_name: player_name.to_string(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_protocol::{PRIVATE_KEY, PROTOCOL_ID};

    #[test]
    fn default_config_uses_localhost() {
        let cfg = ClientConfig::default();
        assert_eq!(cfg.server_addr, "127.0.0.1:5000");
        assert_eq!(cfg.web_transport_addr, "127.0.0.1:5001");
        assert_eq!(cfg.websocket_addr, "127.0.0.1:5002");
        assert_eq!(cfg.player_name, "Player");
    }

    #[test]
    fn build_client_config_custom() {
        let cfg = build_client_config("192.168.1.1:9999", "TestPlayer");
        assert_eq!(cfg.server_addr, "192.168.1.1:9999");
        assert_eq!(cfg.player_name, "TestPlayer");
        assert_eq!(cfg.web_transport_addr, "127.0.0.1:5001");
    }

    #[test]
    fn protocol_id_is_fixed() {
        assert_eq!(PROTOCOL_ID, 0x59c3_7a73);
    }

    #[test]
    fn private_key_is_32_bytes() {
        assert_eq!(PRIVATE_KEY.len(), 32);
    }
}
