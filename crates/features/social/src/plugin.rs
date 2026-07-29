use bevy::prelude::*;

use crate::systems::*;

pub struct SocialPlugin;

impl Plugin for SocialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConnectedRoster>();
        app.init_resource::<PlayerClientMap>();

        app.add_systems(bevy::prelude::FixedUpdate, handle_emote_intent);
        app.add_systems(bevy::prelude::FixedUpdate, cleanup_disconnected_player);
    }
}
