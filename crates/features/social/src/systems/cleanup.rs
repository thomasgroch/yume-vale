use crate::systems::{ConnectedRoster, PlayerClientMap, SocialClientPlayer};
use bevy::prelude::*;

/// Clean up social state when a player disconnects.
///
/// Detects disconnection by monitoring `Connected` component removal.
pub fn cleanup_disconnected_player(
    mut removals: bevy::prelude::RemovedComponents<lightyear::prelude::Connected>,
    connected_players: Query<(Entity, &SocialClientPlayer)>,
    mut roster: ResMut<ConnectedRoster>,
    mut player_client_map: ResMut<PlayerClientMap>,
) {
    for entity in removals.read() {
        if let Ok((_, client_player)) = connected_players.get(entity) {
            let pid = client_player.player_id;
            roster.remove(pid);
            player_client_map.remove(pid);
        }
    }
}
