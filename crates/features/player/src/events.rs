use bevy::prelude::Event;
use game_core::actions::ActionKind;
use game_core::id::PlayerId;

#[derive(Event, Debug, Clone, PartialEq)]
pub struct PlayerSpawned {
    pub player_id: PlayerId,
}

#[derive(Event, Debug, Clone, PartialEq)]
pub struct PlayerDespawned {
    pub player_id: PlayerId,
}

#[derive(Event, Debug, Clone, PartialEq)]
pub struct InventoryChanged {
    pub player_id: PlayerId,
}

#[derive(Event, Debug, Clone, PartialEq)]
pub struct ActionStarted {
    pub player_id: PlayerId,
    pub action: ActionKind,
}
