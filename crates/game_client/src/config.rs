use bevy::prelude::Resource;

/// Configuration for the Yume Vale game client.
#[derive(Debug, Clone, Resource)]
pub struct ClientConfig {
    /// Server address in `host:port` format (default `127.0.0.1:5000`).
    pub server_addr: String,
    /// Display name sent to the server on connect.
    pub player_name: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:5000".to_string(),
            player_name: "Player".to_string(),
        }
    }
}

/// Build a client config for the given server address and player name.
pub fn build_client_config(server_addr: &str, player_name: &str) -> ClientConfig {
    ClientConfig {
        server_addr: server_addr.to_string(),
        player_name: player_name.to_string(),
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
        assert_eq!(cfg.player_name, "Player");
    }

    #[test]
    fn build_client_config_custom() {
        let cfg = build_client_config("192.168.1.1:9999", "TestPlayer");
        assert_eq!(cfg.server_addr, "192.168.1.1:9999");
        assert_eq!(cfg.player_name, "TestPlayer");
    }

    #[test]
    fn protocol_id_is_fixed() {
        assert_eq!(PROTOCOL_ID, 0x59c3_7a6e);
    }

    #[test]
    fn private_key_is_32_bytes() {
        assert_eq!(PRIVATE_KEY.len(), 32);
    }
}
