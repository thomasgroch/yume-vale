//! Game error types.

use crate::id::*;
use crate::inventory::InventoryError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::PlotId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum GameError {
    #[error("unknown player: {0}")]
    UnknownPlayer(PlayerId),
    #[error("unknown resource node index: {0}")]
    UnknownResourceNode(usize),
    #[error("resource node {0} is not available (respawns at tick {1})")]
    ResourceNotAvailable(usize, u64),
    #[error("inventory error: {0}")]
    Inventory(#[from] InventoryError),
    #[error("unknown creature: {0}")]
    UnknownCreature(CreatureId),
    #[error("inventory slot {0} is empty")]
    EmptyInventorySlot(usize),
    #[error("item in slot {0} is not the food for creature")]
    WrongFood(usize),
    #[error("unknown plot: {0}")]
    UnknownPlot(PlotId),
    #[error("player {0} does not own plot {1}")]
    NotPlotOwner(PlayerId, PlotId),
    #[error("plot {0} already has content")]
    PlotOccupied(PlotId),
}
