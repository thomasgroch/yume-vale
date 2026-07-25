use bevy::prelude::{Component, Reflect};
use game_core::id::PlayerId;
use game_core::math::Direction;
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
}

#[derive(Component, Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerName(pub String);

#[derive(Component, Debug, Clone, PartialEq)]
pub struct LocalPlayer;

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerMovement {
    pub direction: Direction,
    pub running: bool,
    pub jump: bool,
}

impl Default for PlayerMovement {
    fn default() -> Self {
        Self {
            direction: Direction::zero(),
            running: false,
            jump: false,
        }
    }
}
