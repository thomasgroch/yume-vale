//! Pure reducers: each `apply_*` method mutates [`GameState`] and returns
//! a [`GameEvent`] or [`GameError`].
//!
//! These are extracted into a sibling module so that `game_state/mod.rs`
//! stays under 250 pure LOC. They remain methods of `GameState` via the
//! `impl super::GameState` block.

use crate::constants;
use crate::id::*;
use crate::inventory::ItemKind;

use super::{GameError, GameEvent, GameState, PlotContent, PlotId, PlotState};

impl GameState {
    pub(super) fn apply_tick(&mut self) -> Result<GameEvent, GameError> {
        self.tick += 1;
        let mut respawned = Vec::new();
        for (i, node) in self.resource_nodes.iter_mut().enumerate() {
            if !node.available && self.tick >= node.respawn_at_tick {
                node.available = true;
                respawned.push(i);
            }
        }
        Ok(GameEvent::TickAdvanced {
            tick: self.tick,
            respawned_nodes: respawned,
        })
    }

    pub(super) fn apply_join_player(&mut self, player: PlayerId) -> Result<GameEvent, GameError> {
        if self.players.contains(&player) {
            return Ok(GameEvent::PlayerJoined { player });
        }
        self.players.push(player);
        self.inventories.push((player, Default::default()));
        Ok(GameEvent::PlayerJoined { player })
    }

    pub(super) fn apply_collect(
        &mut self,
        player: PlayerId,
        resource_node: usize,
    ) -> Result<GameEvent, GameError> {
        if resource_node >= self.resource_nodes.len() {
            return Err(GameError::UnknownResourceNode(resource_node));
        }

        let inv_idx = self
            .find_inventory(player)
            .ok_or(GameError::UnknownPlayer(player))?;

        if !self.resource_nodes[resource_node].available {
            return Err(GameError::ResourceNotAvailable(
                resource_node,
                self.resource_nodes[resource_node].respawn_at_tick,
            ));
        }

        let (res_config, _) = self
            .find_resource_for_node(resource_node)
            .ok_or(GameError::UnknownResourceNode(resource_node))?;
        let yield_amount = res_config.yield_amount;
        let kind = ItemKind::Resource(res_config.kind);
        let respawn_seconds = res_config.respawn_seconds;

        self.inventories[inv_idx].1.add(kind, yield_amount)?;

        self.resource_nodes[resource_node].available = false;
        self.resource_nodes[resource_node].respawn_at_tick =
            self.tick + (respawn_seconds * constants::TICK_RATE_HZ as f32) as u64;

        Ok(GameEvent::ResourceCollected {
            player,
            resource_node,
            amount: yield_amount,
        })
    }

    pub(super) fn apply_feed(
        &mut self,
        player: PlayerId,
        creature_id: CreatureId,
        inventory_slot: usize,
    ) -> Result<GameEvent, GameError> {
        let creature_config = self
            .find_creature_config(creature_id)
            .ok_or(GameError::UnknownCreature(creature_id))?;

        let inv_idx = self
            .find_inventory(player)
            .ok_or(GameError::UnknownPlayer(player))?;

        let slot = self.inventories[inv_idx]
            .1
            .slots
            .get(inventory_slot)
            .and_then(|s| s.as_ref())
            .ok_or(GameError::EmptyInventorySlot(inventory_slot))?;

        if slot.kind != ItemKind::Resource(creature_config.food_kind) {
            return Err(GameError::WrongFood(inventory_slot));
        }

        self.inventories[inv_idx].1.remove(inventory_slot, 1)?;

        let bond_level = if let Some(idx) = self.find_creature_bond(player, creature_id) {
            self.creature_bonds[idx].2 += 1;
            self.creature_bonds[idx].2
        } else {
            self.creature_bonds.push((player, creature_id, 1));
            1
        };

        Ok(GameEvent::CreatureFed {
            player,
            creature_id,
            bond_level,
        })
    }

    pub(super) fn apply_place_on_plot(
        &mut self,
        player: PlayerId,
        plot_id: PlotId,
        item: ItemKind,
        inventory_slot: usize,
    ) -> Result<GameEvent, GameError> {
        let inv_idx = self
            .find_inventory(player)
            .ok_or(GameError::UnknownPlayer(player))?;

        if let Some(idx) = self.find_plot(plot_id) {
            if self.plots[idx].1.owner != player {
                return Err(GameError::NotPlotOwner(player, plot_id));
            }
            if self.plots[idx].1.content != PlotContent::Empty {
                return Err(GameError::PlotOccupied(plot_id));
            }
        }

        self.inventories[inv_idx].1.remove(inventory_slot, 1)?;

        if let Some(idx) = self.find_plot(plot_id) {
            self.plots[idx].1.content = PlotContent::Item(item);
        } else {
            self.plots.push((
                plot_id,
                PlotState {
                    owner: player,
                    content: PlotContent::Item(item),
                },
            ));
        }

        Ok(GameEvent::ItemPlaced { player, plot_id })
    }

    pub(super) fn apply_remove_from_plot(
        &mut self,
        player: PlayerId,
        plot_id: PlotId,
    ) -> Result<GameEvent, GameError> {
        let plot_idx = self
            .find_plot(plot_id)
            .ok_or(GameError::UnknownPlot(plot_id))?;

        if self.plots[plot_idx].1.owner != player {
            return Err(GameError::NotPlotOwner(player, plot_id));
        }

        if self.plots[plot_idx].1.content == PlotContent::Empty {
            return Err(GameError::PlotOccupied(plot_id));
        }

        let inv_idx = self
            .find_inventory(player)
            .ok_or(GameError::UnknownPlayer(player))?;

        let item_kind = match &self.plots[plot_idx].1.content {
            PlotContent::Item(kind) => *kind,
            PlotContent::Empty => return Err(GameError::PlotOccupied(plot_id)),
        };

        self.inventories[inv_idx].1.add(item_kind, 1)?;

        self.plots[plot_idx].1.content = PlotContent::Empty;

        Ok(GameEvent::ItemRemoved { player, plot_id })
    }
}
