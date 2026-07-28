use game_core::constants::{MAX_PLAYERS, TICK_RATE_HZ};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Bind address for all listeners (default `0.0.0.0` = all interfaces,
    /// so friends over LAN/Tailscale/tunnel can connect).
    pub host: String,
    /// Port for the UDP/Netcode listener.
    pub port: u16,
    /// Port for the WebTransport listener (browser via WT).
    pub web_transport_port: u16,
    /// Port for the WebSocket listener (browser via WS fallback).
    pub websocket_port: u16,
    pub max_players: usize,
    pub tick_rate: u32,
    /// Path to a PEM certificate file for WebTransport TLS (production).
    /// When `None`, falls back to the `YUME_TLS_CERT` env var, then self-signed.
    pub tls_cert_path: Option<String>,
    /// Path to a PEM private key file for WebTransport TLS (production, PKCS#8 only).
    /// When `None`, falls back to the `YUME_TLS_KEY` env var, then self-signed.
    pub tls_key_path: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 5000,
            web_transport_port: 5001,
            websocket_port: 5002,
            max_players: MAX_PLAYERS,
            tick_rate: TICK_RATE_HZ,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

/// Build a server config for the given host and port, using defaults for
/// max players and tick rate from `game_core`.
pub fn build_server_config(host: &str, port: u16) -> ServerConfig {
    ServerConfig {
        host: host.to_string(),
        port,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_core_constants() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.max_players, MAX_PLAYERS);
        assert_eq!(cfg.tick_rate, TICK_RATE_HZ);
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 5000);
        assert_eq!(cfg.web_transport_port, 5001);
        assert_eq!(cfg.websocket_port, 5002);
    }

    #[test]
    fn build_server_config_sets_host_and_port() {
        let cfg = build_server_config("0.0.0.0", 12345);
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 12345);
        assert_eq!(cfg.web_transport_port, 5001);
        assert_eq!(cfg.websocket_port, 5002);
        assert_eq!(cfg.max_players, MAX_PLAYERS);
        assert_eq!(cfg.tick_rate, TICK_RATE_HZ);
    }
}
