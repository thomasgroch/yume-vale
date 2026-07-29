pub(crate) mod cleanup;
pub(crate) mod emote;

pub use cleanup::*;
pub use emote::*;

use bevy::prelude::*;
use game_core::id::PlayerId;

// ---------------------------------------------------------------------------
// Bevy resources & components
// ---------------------------------------------------------------------------

/// Added to client link entities so social systems can identify the player
/// without depending on game_server's `ClientPlayer`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SocialClientPlayer {
    pub player_id: PlayerId,
    pub player_entity: Entity,
}

/// Tracks which PlayerIds are currently connected+authenticated.
#[derive(Resource, Default)]
pub struct ConnectedRoster {
    pub players: Vec<PlayerId>,
}

impl ConnectedRoster {
    pub fn is_connected(&self, player: PlayerId) -> bool {
        self.players.contains(&player)
    }

    pub fn add(&mut self, player: PlayerId) {
        if !self.players.contains(&player) {
            self.players.push(player);
        }
    }

    pub fn remove(&mut self, player: PlayerId) {
        self.players.retain(|p| *p != player);
    }
}

/// Maps PlayerId -> client link Entity for sending messages to specific clients.
#[derive(Resource, Default)]
pub struct PlayerClientMap {
    mapping: Vec<(PlayerId, Entity)>,
}

impl PlayerClientMap {
    pub fn set(&mut self, player: PlayerId, entity: Entity) {
        self.mapping.retain(|(p, _)| *p != player);
        self.mapping.push((player, entity));
    }

    pub fn remove(&mut self, player: PlayerId) {
        self.mapping.retain(|(p, _)| *p != player);
    }

    pub fn get(&self, player: PlayerId) -> Option<Entity> {
        self.mapping
            .iter()
            .find(|(p, _)| *p == player)
            .map(|(_, e)| *e)
    }
}
