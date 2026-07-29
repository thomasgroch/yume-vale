//! Emote broadcast handler.

use bevy::prelude::*;
use game_protocol::channels::ReliableChannel;
use game_protocol::{EmoteBroadcast, EmoteIntent};
use lightyear::prelude::{MessageReceiver, MessageSender};

use crate::systems::{ConnectedRoster, PlayerClientMap, SocialClientPlayer};

/// Validate and broadcast `EmoteIntent` to all connected clients.
pub fn handle_emote_intent(
    mut receivers: Query<(&mut MessageReceiver<EmoteIntent>, &SocialClientPlayer)>,
    client_map: Res<PlayerClientMap>,
    mut emote_senders: Query<&mut MessageSender<EmoteBroadcast>>,
    roster: Res<ConnectedRoster>,
) {
    for (mut receiver, client_player) in receivers.iter_mut() {
        for msg in receiver.receive() {
            if !roster.is_connected(client_player.player_id) {
                continue;
            }

            let broadcast = EmoteBroadcast {
                from_player: client_player.player_id.get(),
                emote: msg.emote,
            };

            for &pid in &roster.players {
                if let Some(target_entity) = client_map.get(pid) {
                    if let Ok(mut sender) = emote_senders.get_mut(target_entity) {
                        sender.send::<ReliableChannel>(broadcast.clone());
                    }
                }
            }
        }
    }
}
