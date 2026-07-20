use bevy::prelude::*;
use game_core::id::PlayerId;

/// Tracks the local player's assigned ID (set on receiving Welcome).
#[derive(Resource, Default)]
pub struct LocalPlayerId {
    pub id: Option<PlayerId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_player_id_default() {
        let id = LocalPlayerId::default();
        assert!(id.id.is_none());
    }

    #[test]
    fn local_player_id_set() {
        let id = LocalPlayerId {
            id: Some(PlayerId::new(42)),
        };
        assert_eq!(id.id.unwrap(), PlayerId::new(42));
    }
}
