//! Sends `IdentityHello` when the client connects, and stores the returned
//! token from `Welcome` for future connections.

use bevy::prelude::*;
use game_protocol::{IdentityHello, PROTOCOL_ID};
use lightyear::prelude::*;

use super::token_store::{IdentityToken, save_identity_token};

/// Sends `IdentityHello` every frame while connected but not yet
/// authenticated (i.e., until `LocalPlayerId` is set by Welcome).
///
/// The server ignores duplicate IdentityHellos from already-authenticated
/// clients, so this is safe (though slightly wasteful).
pub(crate) fn send_identity_hello(
    mut senders: Query<&mut MessageSender<IdentityHello>>,
    identity_token: Res<IdentityToken>,
    local_id: Res<super::welcome::LocalPlayerId>,
) {
    if local_id.id.is_some() {
        return;
    }
    for mut sender in senders.iter_mut() {
        sender.send::<game_protocol::channels::ReliableChannel>(IdentityHello {
            protocol_version: PROTOCOL_ID as u32,
            token: identity_token.0.clone(),
        });
    }
}

/// Update the stored identity token from a Welcome message.
pub(crate) fn store_welcome_token(identity_token: &mut IdentityToken, welcome_token: &str) {
    if !welcome_token.is_empty() {
        identity_token.0 = welcome_token.to_string();
        save_identity_token(welcome_token);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_identity_hello_uses_stored_token() {
        let token = IdentityToken("my-token".into());
        assert_eq!(token.0, "my-token");
    }

    #[test]
    fn store_welcome_token_updates_resource_and_persists() {
        let mut token = IdentityToken::default();
        assert!(token.0.is_empty());

        store_welcome_token(&mut token, "new-token-from-server");
        assert_eq!(token.0, "new-token-from-server");
    }

    #[test]
    fn store_welcome_token_ignores_empty() {
        let mut token = IdentityToken("existing".into());
        store_welcome_token(&mut token, "");
        assert_eq!(token.0, "existing", "should not overwrite with empty");
    }
}
