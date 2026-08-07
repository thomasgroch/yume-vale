pub(crate) mod client_id;
mod identity_hello;
mod rejection;
mod retry;
mod token_store;
mod transport;
pub(crate) mod transport_fallback;
mod visibility;
mod welcome;

// Public API re-exports (visible outside the crate)
#[cfg(not(target_arch = "wasm32"))]
pub use client_id::server_addr_from_env;
pub use token_store::{
    IdentityToken, clear_identity_token, load_identity_token, save_identity_token,
};
pub use welcome::LocalPlayerId;

// Crate-internal re-exports (consumed by plugin.rs, menu.rs, visuals.rs)
pub(crate) use identity_hello::send_identity_hello;
pub(crate) use rejection::handle_connection_rejected;
pub(crate) use retry::retry_connect_when_disconnected;
pub(crate) use transport::start_connection;
pub(crate) use transport_fallback::{TransportState, handle_transport_fallback};
pub(crate) use visibility::{PageLifecycle, handle_page_visibility, install_visibility_listener};
pub(crate) use welcome::handle_welcome;
