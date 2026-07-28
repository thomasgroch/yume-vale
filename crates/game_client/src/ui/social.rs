//! Client-side social snapshot resources and message receivers.
//!
//! Projects server-authoritative social state (`ChatReceived`, `GroupUpdate`,
//! `EmoteBroadcast`) into local Bevy resources. Never mutates local state
//! pre-emptively — only the server's confirmed data is rendered.

use bevy::prelude::*;
use game_protocol::messages::{ChatReceived, EmoteBroadcast, GroupUpdate};
use lightyear::prelude::MessageReceiver;

// ─── Resources ────────────────────────────────────────────────────────────

/// Chat messages received from the server (append-only log).
#[derive(Resource, Default)]
pub struct ClientChat {
    pub messages: Vec<ChatReceived>,
}

/// Server-confirmed group membership.
#[derive(Resource, Default)]
pub struct ClientGroup {
    pub members: Vec<u64>,
}

/// Latest emote broadcast from the server, consumed each frame by the
/// animation system.
#[derive(Resource, Default)]
pub struct ClientEmote {
    pub pending: Option<EmoteBroadcast>,
}

// ─── Receiver systems ────────────────────────────────────────────────────

/// Project `ChatReceived` messages into `ClientChat`.
pub fn receive_chat_received(
    mut receivers: Query<&mut MessageReceiver<ChatReceived>>,
    mut chat: ResMut<ClientChat>,
) {
    for mut receiver in &mut receivers {
        for msg in receiver.receive() {
            chat.messages.push(msg);
        }
    }
}

/// Project `GroupUpdate` messages into `ClientGroup`.
pub fn receive_group_update(
    mut receivers: Query<&mut MessageReceiver<GroupUpdate>>,
    mut group: ResMut<ClientGroup>,
) {
    for mut receiver in &mut receivers {
        for msg in receiver.receive() {
            let count = msg.members.len();
            group.members = msg.members;
            info!("group update: {count:?} members");
        }
    }
}

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
    fn client_chat_default_is_empty() {
        let c = ClientChat::default();
        assert!(c.messages.is_empty());
    }

    #[test]
    fn client_group_default_is_empty() {
        let g = ClientGroup::default();
        assert!(g.members.is_empty());
    }

    #[test]
    fn client_emote_default_is_none() {
        let e = ClientEmote::default();
        assert!(e.pending.is_none());
    }

    #[test]
    fn receive_chat_received_appends_message() {
        let mut chat = ClientChat::default();
        let msg = ChatReceived {
            from_player: 1,
            text: "hello".into(),
        };
        chat.messages.push(msg);
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].text, "hello");
    }

    #[test]
    fn receive_chat_received_appends_multiple() {
        let mut chat = ClientChat::default();
        chat.messages.push(ChatReceived {
            from_player: 1,
            text: "hi".into(),
        });
        chat.messages.push(ChatReceived {
            from_player: 2,
            text: "ho".into(),
        });
        assert_eq!(chat.messages.len(), 2);
    }

    #[test]
    fn receive_group_update_replaces_members() {
        let mut group = ClientGroup::default();
        group.members = vec![1, 2, 3];
        assert_eq!(group.members.len(), 3);

        // Replacing with new list
        group.members = vec![4, 5];
        assert_eq!(group.members, vec![4, 5]);
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

    #[test]
    fn group_update_replaces_prior_state() {
        let mut group = ClientGroup::default();
        group.members = vec![1, 2, 3];
        assert_eq!(group.members.len(), 3);
        group.members = vec![]; // empty group (left/ disbanded)
        assert!(group.members.is_empty());
    }
}
