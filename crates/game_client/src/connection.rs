use bevy::prelude::*;
use core::net::SocketAddr;
use game_protocol::{PRIVATE_KEY, PROTOCOL_ID, Welcome};
use lightyear::connection::client::Connect;
use lightyear::netcode::{auth::Authentication, client_plugin::NetcodeConfig};
use lightyear::prelude::client::NetcodeClient;
use lightyear::prelude::*;

#[cfg(target_arch = "wasm32")]
use lightyear::prelude::client::{WebSocketClientIo, WebSocketScheme, WebTransportClientIo};

use crate::config::ClientConfig;

/// Tracks the local player's assigned ID (set on receiving Welcome).
#[derive(Resource, Default)]
pub struct LocalPlayerId {
    pub id: Option<game_core::id::PlayerId>,
}

const RECONNECT_BACKOFF_S: f32 = 2.0;

type DisconnectedClients<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<Client>,
        With<Disconnected>,
        Without<Connected>,
        Without<Connecting>,
    ),
>;

/// Unique netcode client id per instance: the server drops connection requests
/// with an already-connected id (anti-spoofing). On native, `YUME_CLIENT_ID` env
/// overrides for tests. On wasm, a random id is generated via getrandom.
fn derive_client_id() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        client_id_from_env(std::env::var("YUME_CLIENT_ID").ok().as_deref())
            .unwrap_or_else(time_based_client_id)
    }
    #[cfg(target_arch = "wasm32")]
    {
        random_client_id()
    }
}

/// Parses `YUME_CLIENT_ID` env override. Returns `None` if absent or invalid.
/// Only used on native (env vars unavailable on wasm).
#[cfg(not(target_arch = "wasm32"))]
fn client_id_from_env(raw: Option<&str>) -> Option<u64> {
    raw.and_then(|s| s.parse::<u64>().ok()).map(|id| id.max(1))
}

/// `YUME_SERVER_ADDR` env override (`host:port`) for the native server address,
/// so friends can point the binary at a remote host. Native only (wasm resolves
/// the address from the page URL).
#[cfg(not(target_arch = "wasm32"))]
pub fn server_addr_from_env(raw: Option<String>) -> Option<String> {
    raw.filter(|s| !s.is_empty())
}

/// Native: time + process-id based client id (not entropy-safe, but unique per
/// local process instance — good enough for dev).
#[cfg(not(target_arch = "wasm32"))]
fn time_based_client_id() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x59c3_7a6e);
    (nanos ^ ((std::process::id() as u64) << 32)).max(1)
}

/// Wasm: random client id via getrandom (SystemTime and process::id unavailable).
#[cfg(target_arch = "wasm32")]
fn random_client_id() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).expect("getrandom failed to generate client id");
    u64::from_le_bytes(buf).max(1)
}

fn parse_addr(addr: &str, field: &str) -> Option<SocketAddr> {
    match addr.parse() {
        Ok(addr) => Some(addr),
        Err(e) => {
            error!("invalid {field} in ClientConfig ({addr:?}): {e}");
            None
        }
    }
}

fn build_netcode_client(
    addr: SocketAddr,
    client_id: u64,
    config: &NetcodeConfig,
) -> Option<NetcodeClient> {
    let auth = Authentication::Manual {
        server_addr: addr,
        client_id,
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

pub fn start_connection(commands: &mut Commands, config: &ClientConfig) {
    let client_id = derive_client_id();
    // 10s handshake window: first run after a rebuild stalls ~4s compiling
    // Metal shaders, which used to trip the 3s default and ghost the session.
    // Token never expires: NetcodeClient::new bakes the token once at startup,
    // so a client retrying for >30s would otherwise be rejected forever with
    // TokenExpired (dev-only key, so the replay window is not a concern here).
    let netcode_config = NetcodeConfig {
        client_timeout_secs: 10,
        token_expire_secs: -1,
        ..Default::default()
    };

    #[cfg(not(target_arch = "wasm32"))]
    let entity = {
        let Some(addr) = parse_addr(&config.server_addr, "server_addr") else {
            return;
        };
        let Some(client) = build_netcode_client(addr, client_id, &netcode_config) else {
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

    // Wasm: página HTTPS -> wss://{host}/ws (Traefik faz strip para o servidor);
    // HTTP fora de localhost -> ws://{host}:5002; localhost -> WebTransport (cert
    // dev); ?transport=ws força WS em localhost. YUME_SERVER_WS_URL (compile-time)
    // sobrepõe tudo. Endereço no token netcode é nominal — servidor não valida.
    #[cfg(target_arch = "wasm32")]
    let entity = {
        let (query, page_host, page_https) = web_sys::window()
            .map(|w| {
                (
                    w.location().search().ok().unwrap_or_default(),
                    w.location().hostname().ok(),
                    w.location()
                        .protocol()
                        .map(|p| p == "https:")
                        .unwrap_or(false),
                )
            })
            .unwrap_or_default();
        let is_local = page_host
            .as_deref()
            .map(|h| h == "localhost" || h == "127.0.0.1" || h == "[::1]")
            .unwrap_or(true);

        let explicit_url = option_env!("YUME_SERVER_WS_URL")
            .filter(|u| !u.is_empty())
            .map(str::to_string);
        let derived_wss = if !is_local && page_https {
            page_host.as_deref().map(|h| format!("wss://{h}/ws"))
        } else {
            None
        };

        if let Some(url) = explicit_url.or(derived_wss) {
            let Some(token_addr) = parse_addr(&config.websocket_addr, "websocket_addr") else {
                return;
            };
            let Some(client) = build_netcode_client(token_addr, client_id, &netcode_config)
            else {
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

        let use_ws = query.contains("transport=ws") || !is_local;

        let template = if use_ws {
            &config.websocket_addr
        } else {
            &config.web_transport_addr
        };
        let field = if use_ws {
            "websocket_addr"
        } else {
            "web_transport_addr"
        };
        let addr_string = match (is_local, page_host) {
            (false, Some(host)) => {
                let port = template.rsplit(':').next().unwrap_or("5002");
                format!("{host}:{port}")
            }
            _ => template.clone(),
        };
        let Some(addr) = parse_addr(&addr_string, field) else {
            return;
        };
        let Some(client) = build_netcode_client(addr, client_id, &netcode_config) else {
            return;
        };
        let mut spawned = commands.spawn((
            Client::default(),
            LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
            PeerAddr(addr),
            Link::new(None),
            client,
            ReplicationReceiver,
        ));
        if use_ws {
            spawned.insert(WebSocketClientIo::from_addr(
                aeronet_websocket::client::ClientConfig,
                WebSocketScheme::Plain,
            ));
        } else {
            spawned.insert(WebTransportClientIo {
                certificate_digest: include_str!("../../../certs/digest.txt").trim().to_string(),
            });
        }
        spawned.id()
    };

    commands.entity(entity).trigger(|e| Connect { entity: e });
}

/// Re-triggers `Connect` (with backoff) on client entities that lost their
/// connection. The netcode server silently absorbs a quick reconnect while the
/// old session is still alive, so a single failed attempt must not be final.
pub fn retry_connect_when_disconnected(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    clients: DisconnectedClients,
) {
    let timer =
        timer.get_or_insert_with(|| Timer::from_seconds(RECONNECT_BACKOFF_S, TimerMode::Repeating));
    timer.tick(time.delta());
    if !timer.just_finished() {
        return;
    }
    for entity in clients.iter() {
        info!("retrying connection for disconnected client {entity:?}");
        commands.entity(entity).trigger(|e| Connect { entity: e });
    }
}

pub fn handle_welcome(
    mut receivers: Query<&mut MessageReceiver<Welcome>>,
    mut local_id: ResMut<LocalPlayerId>,
) {
    for mut receiver in receivers.iter_mut() {
        for welcome in receiver.receive() {
            local_id.id = Some(welcome.player_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn client_id_from_env_parses_valid_id() {
        assert_eq!(client_id_from_env(Some("42")), Some(42));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn client_id_from_env_rejects_zero() {
        assert_eq!(client_id_from_env(Some("0")), Some(1));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn client_id_from_env_ignores_garbage() {
        assert_eq!(client_id_from_env(Some("not-a-number")), None);
        assert_eq!(client_id_from_env(None), None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn server_addr_from_env_accepts_host_port() {
        assert_eq!(
            server_addr_from_env(Some("100.64.0.1:5000".to_string())),
            Some("100.64.0.1:5000".to_string())
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn server_addr_from_env_rejects_empty_or_missing() {
        assert_eq!(server_addr_from_env(Some(String::new())), None);
        assert_eq!(server_addr_from_env(None), None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn time_based_client_id_is_nonzero_and_unique() {
        let a = time_based_client_id();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let b = time_based_client_id();
        assert_ne!(a, 0);
        assert_ne!(b, 0);
        assert_ne!(a, b, "two client instances must not share a netcode id");
    }

    #[test]
    fn retry_connect_retriggers_after_backoff() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, retry_connect_when_disconnected);

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        app.add_observer(move |_: On<Connect>| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        app.world_mut()
            .spawn((Client::default(), Disconnected::default()));

        app.update();
        assert_eq!(
            counter.load(Ordering::SeqCst),
            0,
            "no retry before the backoff elapses"
        );

        // Virtual time clamps delta to max_delta (250ms default); raise it so a
        // single ManualDuration can exceed the 2s backoff.
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_secs(10));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs(3),
        ));
        app.update();
        assert!(
            counter.load(Ordering::SeqCst) >= 1,
            "Connect should be re-triggered after the backoff"
        );
    }
}
