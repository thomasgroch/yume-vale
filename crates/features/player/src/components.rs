use bevy::prelude::Component;
use game_core::id::PlayerId;
use game_core::math::Direction;
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerName(pub String);

#[derive(Component, Debug, Clone, PartialEq)]
pub struct LocalPlayer;

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerMovement {
    pub direction: Direction,
    pub running: bool,
}

impl Default for PlayerMovement {
    fn default() -> Self {
        Self {
            direction: Direction::zero(),
            running: false,
        }
    }
}

#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Velocity(pub bevy::prelude::Vec3);
