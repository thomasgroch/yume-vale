//! Tests for transport fallback state machine.

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
