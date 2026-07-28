use bevy::prelude::*;
use game_core::id::PlayerId;

/// Attached to client link entities by the server after authentication.
/// Provides the housing systems with player identity without depending on
/// game_server internals.
#[derive(Component)]
pub struct HousingPlayer {
    pub player_entity: Entity,
    pub player_id: PlayerId,
}

/// Marks a spawned decoration entity on a housing plot.
#[derive(Component)]
pub struct PlotDecorationMarker {
    pub player_id: u64,
    pub slot_index: usize,
}
