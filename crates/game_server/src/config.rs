use game_core::constants::{MAX_PLAYERS, TICK_RATE_HZ};

/// Configuration for the game server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to.
    pub host: String,
    /// Port to bind to.
    pub port: u16,
    /// Maximum number of concurrent players.
    pub max_players: usize,
    /// Simulation tick rate in Hz.
    pub tick_rate: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5000,
            max_players: MAX_PLAYERS,
            tick_rate: TICK_RATE_HZ,
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
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 5000);
    }

    #[test]
    fn build_server_config_sets_host_and_port() {
        let cfg = build_server_config("0.0.0.0", 12345);
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 12345);
        assert_eq!(cfg.max_players, MAX_PLAYERS);
        assert_eq!(cfg.tick_rate, TICK_RATE_HZ);
    }
}
