use bevy::prelude::*;

use crate::systems::*;

/// Adds social features (chat, groups, invites, emote broadcast) to the server.
pub struct SocialPlugin;

impl Plugin for SocialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SocialStateResource>();
        app.init_resource::<ConnectedRoster>();
        app.init_resource::<PlayerClientMap>();

        app.add_systems(
            bevy::prelude::FixedUpdate,
            (
                handle_chat_send,
                handle_group_invite,
                handle_group_accept,
                handle_group_decline,
                handle_group_leave,
                handle_emote_intent,
            )
                .chain(),
        );
        app.add_systems(bevy::prelude::FixedUpdate, cleanup_disconnected_player);
    }
}
