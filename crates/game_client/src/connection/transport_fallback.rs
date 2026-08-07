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
use game_protocol::RejectionKind;

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
    pub(crate) rejection_reason: Option<RejectionKind>,
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
            mode: select_transport_mode(false, true),
            wt_start: None,
            rejection_received: false,
            rejection_reason: None,
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
        let wt_available = window.as_ref().is_some_and(|window| {
            js_sys::Reflect::has(window.as_ref(), &"WebTransport".into()).unwrap_or(false)
        });
        let mode = select_transport_mode(explicit_ws_override, wt_available);

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

    pub(crate) fn reject(&mut self, reason: RejectionKind) {
        self.rejection_received = true;
        self.rejection_reason = Some(reason);
        self.wt_start = None;
    }

    pub(crate) fn reset_rejection(&mut self) {
        self.rejection_received = false;
        self.rejection_reason = None;
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

pub(crate) fn select_transport_mode(
    explicit_ws_override: bool,
    wt_available: bool,
) -> TransportMode {
    if explicit_ws_override || !wt_available {
        TransportMode::WebSocket
    } else {
        TransportMode::WebTransport
    }
}

// ---------------------------------------------------------------------------
// Fallback system
// ---------------------------------------------------------------------------

/// Wasm: child module with the actual fallback system + WS spawning logic.
#[cfg(target_arch = "wasm32")]
mod web;

/// Re-export wasm handler so `connection/mod.rs`'s alias works.
#[cfg(target_arch = "wasm32")]
pub(crate) use web::handle_transport_fallback;

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
mod tests;
