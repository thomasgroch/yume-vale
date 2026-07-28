//! Persistent identity token storage.
//!
//! The client stores the opaque token issued by the server on first
//! connection. On subsequent connections, the token is re-sent so the
//! server can restore the same stable `PlayerId`.
//!
//! Storage backends:
//! - **Browser**: `window.localStorage` under key `yume_identity_token`
//! - **Native**: `~/.config/yume-vale/identity.json` (atomic write)
//! - **Override**: `YUME_IDENTITY_TOKEN` env var (native only, test override)

use bevy::prelude::*;

/// Bevy resource holding the loaded identity token (empty = new identity).
#[derive(Resource, Default, Clone)]
pub struct IdentityToken(pub String);

/// Load the stored identity token from the platform-specific store.
///
/// Returns `None` if no token exists, the store is corrupt, or the env
/// override is not set.
pub fn load_identity_token() -> Option<String> {
    // Env override takes highest priority (native only).
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(token) = std::env::var("YUME_IDENTITY_TOKEN") {
            if !token.is_empty() {
                return Some(token);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        load_native_token()
    }

    #[cfg(target_arch = "wasm32")]
    {
        load_wasm_token()
    }
}

/// Save the identity token to the platform-specific store.
///
/// On native, the write is atomic (write to temp file, then rename).
pub fn save_identity_token(token: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    save_native_token(token);

    #[cfg(target_arch = "wasm32")]
    save_wasm_token(token);
}

/// Clear the stored identity token (e.g. on rejection or explicit logout).
#[allow(dead_code)]
pub fn clear_identity_token() {
    save_identity_token("");
}

// ---------------------------------------------------------------------------
// Native (file-based) storage
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn token_file_path() -> Option<std::path::PathBuf> {
    let dir = dirs::config_dir().map(|d| d.join("yume-vale"))?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("identity.json"))
}

#[cfg(not(target_arch = "wasm32"))]
fn load_native_token() -> Option<String> {
    let path = token_file_path()?;
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    // Expect JSON: {"token": "..."}
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    let token = parsed.get("token")?.as_str()?.to_string();
    if token.is_empty() { None } else { Some(token) }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_native_token(token: &str) {
    let Some(path) = token_file_path() else {
        return;
    };
    let json = serde_json::json!({ "token": token }).to_string();

    // Atomic write: write to temp file, then rename.
    let tmp_path = path.with_extension("json.tmp");
    if std::fs::write(&tmp_path, &json).is_ok() {
        let _ = std::fs::rename(&tmp_path, &path);
    }
}

// ---------------------------------------------------------------------------
// Wasm (localStorage) storage
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn load_wasm_token() -> Option<String> {
    let storage = storage()?;
    let token = storage.get_item("yume_identity_token").ok()??;
    if token.is_empty() { None } else { Some(token) }
}

#[cfg(target_arch = "wasm32")]
fn save_wasm_token(token: &str) {
    if let Some(storage) = storage() {
        if token.is_empty() {
            let _ = storage.remove_item("yume_identity_token");
        } else {
            let _ = storage.set_item("yume_identity_token", token);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_identity_token_is_empty() {
        let token = IdentityToken::default();
        assert_eq!(token.0, "");
    }

    #[test]
    fn clear_identity_token_sets_empty() {
        // Just verify it doesn't panic.
        clear_identity_token();
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native_tests {
        use super::*;

        /// Helper: run all native token store scenarios sequentially in a
        /// single test, so parallel test execution does not cause HOME env var
        /// conflicts.
        #[test]
        fn native_token_store_full_scenarios() {
            let prefix = format!("yume-native-test-{}", std::process::id());
            let base = std::env::temp_dir().join(&prefix);
            let _ = std::fs::remove_dir_all(&base);

            // ── 1. Missing file returns None ──
            {
                let dir = base.join("missing");
                std::fs::create_dir_all(&dir).unwrap();
                unsafe {
                    std::env::set_var("HOME", &dir);
                }
                assert!(load_identity_token().is_none());
            }

            // ── 2. Save and load roundtrip ──
            {
                let dir = base.join("roundtrip");
                std::fs::create_dir_all(&dir).unwrap();
                unsafe {
                    std::env::set_var("HOME", &dir);
                }
                save_identity_token("test-token-123");
                let loaded = load_identity_token();
                assert_eq!(loaded.as_deref(), Some("test-token-123"));
            }

            // ── 3. Corrupt store returns None ──
            {
                let dir = base.join("corrupt");
                let cfg_dir = dir.join("Library/Application Support/yume-vale");
                std::fs::create_dir_all(&cfg_dir).unwrap();
                std::fs::write(cfg_dir.join("identity.json"), "not-json").unwrap();
                unsafe {
                    std::env::set_var("HOME", &dir);
                }
                assert!(load_identity_token().is_none());
            }

            // ── 4. YUME_IDENTITY_TOKEN env var overrides file ──
            {
                let dir = base.join("override");
                std::fs::create_dir_all(&dir).unwrap();
                unsafe {
                    std::env::set_var("HOME", &dir);
                }
                save_identity_token("file-token");

                unsafe {
                    std::env::set_var("YUME_IDENTITY_TOKEN", "env-token");
                }
                let loaded = load_identity_token();
                assert_eq!(loaded.as_deref(), Some("env-token"));
                unsafe {
                    std::env::remove_var("YUME_IDENTITY_TOKEN");
                }

                let loaded = load_identity_token();
                assert_eq!(loaded.as_deref(), Some("file-token"));
            }

            unsafe {
                std::env::remove_var("HOME");
            }
            let _ = std::fs::remove_dir_all(&base);
        }
    }
}
