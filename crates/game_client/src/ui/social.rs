use bevy::prelude::*;
use game_protocol::messages::EmoteBroadcast;
use lightyear::prelude::MessageReceiver;

// ─── Resources ────────────────────────────────────────────────────────────

/// Latest emote broadcast from the server, consumed each frame by the
/// animation system.
#[derive(Resource, Default)]
pub struct ClientEmote {
    pub pending: Option<EmoteBroadcast>,
}

// ─── Receiver systems ────────────────────────────────────────────────────

/// Project `EmoteBroadcast` messages into `ClientEmote`.
pub fn receive_emote_broadcast(
    mut receivers: Query<&mut MessageReceiver<EmoteBroadcast>>,
    mut emote: ResMut<ClientEmote>,
) {
    for mut receiver in &mut receivers {
        for msg in receiver.receive() {
            emote.pending = Some(msg);
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use game_core::actions::EmoteKind;

    #[test]
    fn client_emote_default_is_none() {
        let e = ClientEmote::default();
        assert!(e.pending.is_none());
    }

    #[test]
    fn receive_emote_broadcast_sets_pending() {
        let mut emote = ClientEmote::default();
        emote.pending = Some(EmoteBroadcast {
            from_player: 1,
            emote: EmoteKind::Wave,
        });
        assert!(emote.pending.is_some());
        assert_eq!(emote.pending.as_ref().unwrap().from_player, 1);
        assert_eq!(emote.pending.as_ref().unwrap().emote, EmoteKind::Wave);
    }

    #[test]
    fn receive_emote_broadcast_overwrites_previous() {
        let mut emote = ClientEmote::default();
        emote.pending = Some(EmoteBroadcast {
            from_player: 1,
            emote: EmoteKind::Wave,
        });
        emote.pending = Some(EmoteBroadcast {
            from_player: 2,
            emote: EmoteKind::Dance,
        });
        assert_eq!(emote.pending.as_ref().unwrap().from_player, 2);
    }
}
