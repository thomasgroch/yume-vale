use bevy::prelude::*;
use game_core::id::PlayerId;
use game_protocol::Welcome;
use lightyear::prelude::MessageReceiver;

use super::identity_hello::store_welcome_token;
use super::token_store::IdentityToken;

/// Tracks the local player's assigned ID (set on receiving Welcome).
#[derive(Resource, Default)]
pub struct LocalPlayerId {
    pub id: Option<game_core::id::PlayerId>,
}

pub(crate) fn handle_welcome(
    mut receivers: Query<&mut MessageReceiver<Welcome>>,
    mut local_id: ResMut<LocalPlayerId>,
    mut identity_token: ResMut<IdentityToken>,
) {
    for mut receiver in receivers.iter_mut() {
        for welcome in receiver.receive() {
            local_id.id = Some(PlayerId::new(welcome.player_id));
            store_welcome_token(&mut identity_token, &welcome.token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_player_id_default_is_none() {
        let lid = LocalPlayerId::default();
        assert!(lid.id.is_none());
    }
}
