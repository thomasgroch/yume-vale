//! WebTransport → WebSocket fallback state machine.
//!
//! On wasm the client first attempts a WebTransport (WT) connection with a
//! bounded timeout. If WT link errors or fails to establish within the
//! timeout, the system despawns the WT entity and spawns a WebSocket (WS)
//! entity in its place. Once on WS the client stays there permanently —
//! reconnects never try WT again.
//!
//! Key rules:
//! - Local (HTTP) dev keeps pinned WT with the dev-cert digest (unchanged).
//! - Production (HTTPS) first attempts WT (empty digest → browser CA).
//! - `?transport=ws` in the URL skips WT entirely.
//! - Protocol/identity rejection (ConnectionRejected) does NOT trigger
//!   fallback — the issue is at the application layer, not the transport.
//! - `YUME_SERVER_WS_URL` compile-time override still wins unconditionally.

use bevy::prelude::*;
use core::time::Duration;

#[cfg(target_arch = "wasm32")]
use lightyear::prelude::client::{WebSocketClientIo, WebSocketScheme, WebTransportClientIo};
#[cfg(target_arch = "wasm32")]
use lightyear::prelude::{Client, Connected, Link, PeerAddr, ReplicationReceiver};

/// How long we wait for WebTransport to connect before falling back to WS.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const WT_TIMEOUT: Duration = Duration::from_secs(8);

/// Transport selection mode — what to try on the *next* connection attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TransportMode {
    /// Try WebTransport first.
    #[default]
    WebTransport,
    /// Permanently on WebSocket (fallback already activated).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    WebSocket,
}

impl TransportMode {
    pub(crate) fn short_name(&self) -> &'static str {
        match self {
            TransportMode::WebTransport => "WT",
            TransportMode::WebSocket => "WS",
        }
    }
}

/// State machine for the WebTransport → WebSocket fallback.
///
/// This resource is initialised once at app startup. `detect()` reads the
/// browser environment on wasm; on native it returns a default (no-op).
#[derive(Resource)]
pub(crate) struct TransportState {
    /// Current transport mode for the next connection attempt.
    pub mode: TransportMode,
    /// Wall-clock time (via `Time::elapsed_seconds_f64`) when the WT attempt
    /// was launched. `None` = not yet attempted.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) wt_start: Option<f64>,
    /// If `true` the server sent `ConnectionRejected` — do NOT fall back
    /// (application-layer rejection, not transport failure).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) rejection_received: bool,
    /// Page hostname for URL construction (wasm only).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) page_host: Option<String>,
    /// Whether the page protocol is HTTPS.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) page_https: bool,
    /// Whether the page host is localhost / 127.0.0.1 / [::1].
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) page_is_local: bool,
    /// `?transport=ws` in the URL — skip WT entirely.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) explicit_ws_override: bool,
}

// ---------------------------------------------------------------------------
// Default
// ---------------------------------------------------------------------------

impl Default for TransportState {
    fn default() -> Self {
        Self {
            mode: TransportMode::default(),
            wt_start: None,
            rejection_received: false,
            page_host: None,
            page_https: false,
            page_is_local: true,
            explicit_ws_override: false,
        }
    }
}

impl TransportState {
    /// Detect browser environment. On native returns [`Default`].
    pub(crate) fn detect() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::detect_from_window()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::default()
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn detect_from_window() -> Self {
        use wasm_bindgen::JsCast;

        let window = web_sys::window();
        let query = window
            .as_ref()
            .and_then(|w| w.location().search().ok())
            .unwrap_or_default();
        let page_host = window.as_ref().and_then(|w| w.location().hostname().ok());
        let page_https = window
            .as_ref()
            .and_then(|w| w.location().protocol().ok())
            .map(|p| p == "https:")
            .unwrap_or(false);
        let page_is_local = page_host
            .as_deref()
            .map(|h| h == "localhost" || h == "127.0.0.1" || h == "[::1]")
            .unwrap_or(true);
        let explicit_ws_override = query.contains("transport=ws");

        let mode = if explicit_ws_override {
            TransportMode::WebSocket
        } else {
            TransportMode::WebTransport
        };

        Self {
            mode,
            explicit_ws_override,
            page_host,
            page_https,
            page_is_local,
            ..Default::default()
        }
    }

    /// Record that a WT attempt just started (sets the timeout clock).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn start_wt_attempt(&mut self, now: f64) {
        self.wt_start = Some(now);
    }

    /// Mark that the server rejected this connection — suppresses fallback.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn mark_rejection(&mut self) {
        self.rejection_received = true;
    }

    /// Returns `true` if we are still trying WT and have not been rejected.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn trying_wt(&self) -> bool {
        self.mode == TransportMode::WebTransport && !self.rejection_received
    }

    /// Returns the production WSS URL (`wss://{host}/ws`) if applicable.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn prod_wss_url(&self) -> Option<String> {
        if !self.page_is_local && self.page_https {
            self.page_host.as_deref().map(|h| format!("wss://{h}/ws"))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Fallback system (wasm only)
// ---------------------------------------------------------------------------

/// Detect WebTransport failure and switch to WebSocket.
#[cfg(target_arch = "wasm32")]
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
    let mut is_connected = false;

    for (entity, connected, has_wt) in clients.iter() {
        if has_wt {
            wt_entity = Some(entity);
            is_connected = connected.is_some();
            if is_connected {
                state.wt_start = None;
                return;
            }
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

    if now - start < WT_TIMEOUT.as_secs_f64() {
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

/// Native no-op (UDP is used instead of WT/WS).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn handle_transport_fallback(
    _commands: Commands,
    _state: ResMut<TransportState>,
    _time: Res<Time>,
    _config: Res<crate::config::ClientConfig>,
) {
    // No-op: native uses UDP, not WT/WS.
}

// ---------------------------------------------------------------------------
// WS entity spawning (shared by start_connection on first connect + fallback)
// ---------------------------------------------------------------------------

/// Spawn a client entity with WebSocket IO (either local WS or production WSS).
#[cfg(target_arch = "wasm32")]
pub(crate) fn spawn_ws_client(
    commands: &mut Commands,
    config: &crate::config::ClientConfig,
    state: &TransportState,
) {
    let cid = super::client_id::derive_client_id();
    let netcode_config = super::transport::netcode_config();

    // Try production WSS URL first.
    if let Some(wss_url) = state.prod_wss_url() {
        let Some(token_addr) =
            super::transport::parse_addr(&config.websocket_addr, "websocket_addr")
        else {
            return;
        };
        let Some(client) = super::transport::build_netcode_client(token_addr, cid, &netcode_config)
        else {
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
                WebSocketClientIo::from_url(aeronet_websocket::client::ClientConfig, wss_url),
            ))
            .id();
        commands.entity(entity).trigger(|e| Connect { entity: e });
        return;
    }

    let addr_string = config.websocket_addr.clone();
    let Some(addr) = super::transport::parse_addr(&addr_string, "websocket_addr") else {
        return;
    };
    let Some(client) = super::transport::build_netcode_client(addr, cid, &netcode_config) else {
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
            WebSocketClientIo::from_addr(
                aeronet_websocket::client::ClientConfig,
                WebSocketScheme::Plain,
            ),
        ))
        .id();
    commands.entity(entity).trigger(|e| Connect { entity: e });
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub(crate) fn spawn_ws_client(
    _commands: &mut Commands,
    _config: &crate::config::ClientConfig,
    _state: &TransportState,
) {
}

// ---------------------------------------------------------------------------
// Pure helpers (testable on all platforms)
// ---------------------------------------------------------------------------

/// Returns `true` when the WT attempt has timed out.
/// Uses `>` (not `>=`) so the timeout fires *after* the duration elapses.
#[allow(dead_code)]
pub(crate) fn wt_timed_out(wt_start: Option<f64>, now: f64) -> bool {
    match wt_start {
        Some(start) => now - start > WT_TIMEOUT.as_secs_f64(),
        None => false,
    }
}

/// Returns `true` if we should attempt WT this connection.
#[allow(dead_code)]
pub(crate) fn should_try_wt(mode: TransportMode, rejection_received: bool) -> bool {
    mode == TransportMode::WebTransport && !rejection_received
}

/// Derive address string for the wasm environment (handles production vs local).
/// Uses `default_port` when the template has no colon-separated port.
#[allow(dead_code)]
pub(crate) fn derive_wasm_addr(
    template: &str,
    is_local: bool,
    page_host: Option<&str>,
    default_port: &str,
) -> String {
    match (is_local, page_host) {
        (false, Some(host)) => {
            let port = template
                .split(':')
                .nth(1)
                .filter(|p| !p.is_empty())
                .unwrap_or(default_port);
            format!("{host}:{port}")
        }
        _ => template.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    // ── TransportMode ──────────────────────────────────────────────

    #[test]
    fn transport_mode_default_is_wt() {
        assert_eq!(TransportMode::default(), TransportMode::WebTransport);
    }

    #[test]
    fn transport_mode_short_names() {
        assert_eq!(TransportMode::WebTransport.short_name(), "WT");
        assert_eq!(TransportMode::WebSocket.short_name(), "WS");
    }

    // ── TransportState ────────────────────────────────────────────

    #[test]
    fn transport_state_default() {
        let s = TransportState::default();
        assert_eq!(s.mode, TransportMode::WebTransport);
        assert!(s.wt_start.is_none());
        assert!(!s.rejection_received);
        assert!(!s.explicit_ws_override);
        assert!(s.page_is_local);
    }

    #[test]
    fn start_wt_attempt_sets_timer() {
        let mut s = TransportState::default();
        s.start_wt_attempt(42.0);
        assert_eq!(s.wt_start, Some(42.0));
    }

    #[test]
    fn mark_rejection_sets_flag() {
        let mut s = TransportState::default();
        assert!(!s.rejection_received);
        s.mark_rejection();
        assert!(s.rejection_received);
    }

    #[test]
    fn trying_wt_returns_true_when_active() {
        let s = TransportState::default();
        assert!(s.trying_wt());
    }

    #[test]
    fn trying_wt_returns_false_when_rejected() {
        let mut s = TransportState::default();
        s.mark_rejection();
        assert!(!s.trying_wt());
    }

    #[test]
    fn trying_wt_returns_false_when_on_ws() {
        let mut s = TransportState::default();
        s.mode = TransportMode::WebSocket;
        assert!(!s.trying_wt());
    }

    #[test]
    fn prod_wss_url_none_for_local() {
        let s = TransportState::default(); // page_is_local = true
        assert!(s.prod_wss_url().is_none());
    }

    #[test]
    fn prod_wss_url_derived_for_production() {
        let s = TransportState {
            mode: TransportMode::WebTransport,
            page_host: Some("yume.lab.thomasdev.xyz".into()),
            page_https: true,
            page_is_local: false,
            ..Default::default()
        };
        assert_eq!(
            s.prod_wss_url(),
            Some("wss://yume.lab.thomasdev.xyz/ws".to_string())
        );
    }

    #[test]
    fn prod_wss_url_none_when_not_https() {
        let s = TransportState {
            page_host: Some("example.com".into()),
            page_https: false,
            page_is_local: false,
            ..Default::default()
        };
        assert!(s.prod_wss_url().is_none());
    }

    #[test]
    fn detect_on_native_returns_default() {
        // Running on native — detect() should return default.
        let s = TransportState::detect();
        assert_eq!(s.mode, TransportMode::WebTransport);
        assert!(!s.explicit_ws_override);
    }

    // ── Pure helpers ───────────────────────────────────────────────

    #[test]
    fn wt_timed_out_no_start_returns_false() {
        assert!(!wt_timed_out(None, 100.0));
    }

    #[test]
    fn wt_timed_out_within_timeout_returns_false() {
        // WT_TIMEOUT is 8s, start at 0, now at 5s → not timed out.
        assert!(!wt_timed_out(Some(0.0), 5.0));
    }

    #[test]
    fn wt_timed_out_exact_boundary_returns_false() {
        assert!(!wt_timed_out(Some(0.0), 8.0)); // 8.0 < WT_TIMEOUT
    }

    #[test]
    fn wt_timed_out_exceeded_returns_true() {
        assert!(wt_timed_out(Some(0.0), 8.001));
    }

    #[test]
    fn should_try_wt_active() {
        assert!(should_try_wt(TransportMode::WebTransport, false));
    }

    #[test]
    fn should_try_wt_rejected() {
        assert!(!should_try_wt(TransportMode::WebTransport, true));
    }

    #[test]
    fn should_try_wt_ws_mode() {
        assert!(!should_try_wt(TransportMode::WebSocket, false));
    }

    #[test]
    fn derive_wasm_addr_local_uses_template() {
        let addr = derive_wasm_addr("127.0.0.1:5001", true, None, "5001");
        assert_eq!(addr, "127.0.0.1:5001");
    }

    #[test]
    fn derive_wasm_addr_production_uses_host() {
        let addr = derive_wasm_addr(
            "127.0.0.1:5001",
            false,
            Some("yume.lab.thomasdev.xyz"),
            "5001",
        );
        assert_eq!(addr, "yume.lab.thomasdev.xyz:5001");
    }

    #[test]
    fn derive_wasm_addr_production_custom_port() {
        let addr = derive_wasm_addr("127.0.0.1:9999", false, Some("example.com"), "5001");
        assert_eq!(addr, "example.com:9999");
    }

    #[test]
    fn derive_wasm_addr_production_default_port() {
        let addr = derive_wasm_addr("no-port", false, Some("example.com"), "5001");
        assert_eq!(addr, "example.com:5001");
    }
}
