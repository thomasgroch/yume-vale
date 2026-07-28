use bevy::prelude::*;
use game_core::id::PlayerId;
use lightyear::prelude::*;

use crate::config::ServerConfig;

/// Maps a client entity (LinkOf) to its player entity.
#[derive(Component)]
pub struct ClientPlayer {
    pub player_entity: Entity,
    pub player_id: PlayerId,
}

/// Wraps server config for Bevy resource access.
#[derive(Resource, Clone)]
pub struct ServerConfigResource(pub ServerConfig);

/// Round-robin counter for assigning distinct `PlayerColor` palette indices.
#[derive(Resource, Default)]
pub struct NextPlayerColor(pub u8);

/// System set for server game logic.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerSystems;

/// Adds `ReplicationSender` to new client link entities.
pub fn handle_new_client_link(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(ReplicationSender);
}
