use bevy::prelude::*;

/// Unique netcode client id per instance: the server drops connection requests
/// with an already-connected id (anti-spoofing). On native, `YUME_CLIENT_ID` env
/// overrides for tests. On wasm, a random id is generated via getrandom.
pub(crate) fn derive_client_id() -> u64 {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn derive_client_id_returns_nonzero() {
        let id = derive_client_id();
        assert!(id > 0, "client id must be > 0");
    }
}
