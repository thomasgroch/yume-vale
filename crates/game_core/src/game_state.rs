use crate::id::*;
use crate::inventory::{Inventory, InventoryError, ItemKind};
use crate::world_config::{CreatureConfig, QuestConfig, QuestReward, ResourceConfig, WorldConfig};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Globally unique plot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlotId(pub u64);

impl std::fmt::Display for PlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PlotId({})", self.0)
    }
}

/// Globally unique group identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub u64);

impl std::fmt::Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GroupId({})", self.0)
    }
}

/// State of a single resource node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceNodeState {
    pub available: bool,
    /// Tick at which this node becomes available again (if not available).
    pub respawn_at_tick: u64,
}

/// Bond level with a creature.
pub type BondLevel = u32;

/// Progress on a single quest for a single player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestProgress {
    /// Current progress per objective (index matches quest definition).
    pub objective_progress: Vec<u32>,
    /// Whether all objectives are met.
    pub completed: bool,
}

/// What sits on a housing plot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlotContent {
    Empty,
    Item(ItemKind),
}

/// Ownership and content of a single plot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotState {
    pub owner: PlayerId,
    pub content: PlotContent,
}

/// A player group for collaborative quest credit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: GroupId,
    pub members: Vec<PlayerId>,
}

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

/// An action a player may take — pure, no floating-point, no physics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Intent {
    /// Advance the simulation by one tick (respawn timers, etc.).
    Tick,
    /// Register a new player (gives them an empty inventory).
    JoinPlayer { player: PlayerId },
    /// Harvest from a resource node (index into `resource_nodes`).
    Collect {
        player: PlayerId,
        resource_node: usize,
    },
    /// Feed a creature from a specific inventory slot.
    Feed {
        player: PlayerId,
        creature_id: CreatureId,
        inventory_slot: usize,
    },
    /// Place an item from inventory onto an owned plot.
    PlaceOnPlot {
        player: PlayerId,
        plot_id: PlotId,
        item: ItemKind,
        inventory_slot: usize,
    },
    /// Remove and reclaim the item from an owned plot.
    RemoveFromPlot { player: PlayerId, plot_id: PlotId },
    /// Claim rewards for a completed quest.
    ClaimQuestReward { player: PlayerId, quest_id: QuestId },
    /// Create a new group.
    CreateGroup {
        group_id: GroupId,
        members: Vec<PlayerId>,
    },
    /// A player joins an existing group.
    JoinGroup { player: PlayerId, group_id: GroupId },
    /// A player leaves a group.
    LeaveGroup { player: PlayerId, group_id: GroupId },
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Every successful intent produces exactly one event describing the outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameEvent {
    PlayerJoined {
        player: PlayerId,
    },
    TickAdvanced {
        tick: u64,
        respawned_nodes: Vec<usize>,
    },
    ResourceCollected {
        player: PlayerId,
        resource_node: usize,
        amount: u32,
    },
    CreatureFed {
        player: PlayerId,
        creature_id: CreatureId,
        bond_level: BondLevel,
    },
    ItemPlaced {
        player: PlayerId,
        plot_id: PlotId,
    },
    ItemRemoved {
        player: PlayerId,
        plot_id: PlotId,
    },
    QuestRewardClaimed {
        player: PlayerId,
        quest_id: QuestId,
        rewards: Vec<ItemKind>,
    },
    GroupCreated {
        group_id: GroupId,
    },
    PlayerJoinedGroup {
        player: PlayerId,
        group_id: GroupId,
    },
    PlayerLeftGroup {
        player: PlayerId,
        group_id: GroupId,
    },
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

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
    #[error("unknown quest: {0}")]
    UnknownQuest(QuestId),
    #[error("quest {0} is not yet completed")]
    QuestNotCompleted(QuestId),
    #[error("quest {0} rewards already claimed")]
    RewardsAlreadyClaimed(QuestId),
    #[error("unknown plot: {0}")]
    UnknownPlot(PlotId),
    #[error("player {0} does not own plot {1}")]
    NotPlotOwner(PlayerId, PlotId),
    #[error("plot {0} already has content")]
    PlotOccupied(PlotId),
    #[error("unknown group: {0}")]
    UnknownGroup(GroupId),
    #[error("player {0} is not in group {1}")]
    NotInGroup(PlayerId, GroupId),
    #[error("player {0} is already in group {1}")]
    AlreadyInGroup(PlayerId, GroupId),
    #[error("duplicate collaborative credit for quest {0} in group {1}")]
    DuplicateGroupCredit(QuestId, GroupId),
}

// ---------------------------------------------------------------------------
// Pure GameState
// ---------------------------------------------------------------------------

/// The entire deterministic simulation state.
///
/// No Bevy entities, clocks, floating-point physics, Tnua, or Avian.
/// All collections are `Vec` (no HashMap) for stable serialization.
/// Players, quests, creatures, plots, groups are tracked as linear lists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    /// Deterministic seed (included in checksum).
    pub seed: u64,
    /// Current tick counter.
    pub tick: u64,
    /// The world config that defines the game rules.
    pub config: WorldConfig,
    /// Flat list of resource nodes (deterministic: same order as config).
    pub resource_nodes: Vec<ResourceNodeState>,
    /// Known players.
    pub players: Vec<PlayerId>,
    /// Player inventories — pairs are kept in insertion order, sorted for checksum.
    pub inventories: Vec<(PlayerId, Inventory)>,
    /// Quest progress per player: (player, quest, progress).
    pub quest_progress: Vec<(PlayerId, QuestId, QuestProgress)>,
    /// Bond level per (player, creature)
    pub creature_bonds: Vec<(PlayerId, CreatureId, BondLevel)>,
    /// Plot registry.
    pub plots: Vec<(PlotId, PlotState)>,
    /// Active groups.
    pub groups: Vec<(GroupId, Group)>,
    /// Which (player, quest) pairs have had rewards claimed.
    pub claimed_rewards: Vec<(PlayerId, QuestId)>,
    /// Which (group, quest) pairs have been credited collaboratively.
    pub group_quest_credits: Vec<(GroupId, QuestId)>,
}

impl GameState {
    /// Create a new game state from a seed and world config.
    pub fn new(seed: u64, config: WorldConfig) -> Self {
        let resource_nodes: Vec<ResourceNodeState> = config
            .resources
            .iter()
            .flat_map(|rc| {
                (0..rc.count).map(|_| ResourceNodeState {
                    available: true,
                    respawn_at_tick: 0,
                })
            })
            .collect();

        Self {
            seed,
            tick: 0,
            config,
            resource_nodes,
            players: Vec::new(),
            inventories: Vec::new(),
            quest_progress: Vec::new(),
            creature_bonds: Vec::new(),
            plots: Vec::new(),
            groups: Vec::new(),
            claimed_rewards: Vec::new(),
            group_quest_credits: Vec::new(),
        }
    }

    /// Apply an intent, mutating state. Returns either a `GameEvent` or an error.
    pub fn apply(&mut self, intent: Intent) -> Result<GameEvent, GameError> {
        match intent {
            Intent::Tick => self.apply_tick(),
            Intent::JoinPlayer { player } => self.apply_join_player(player),
            Intent::Collect {
                player,
                resource_node,
            } => self.apply_collect(player, resource_node),
            Intent::Feed {
                player,
                creature_id,
                inventory_slot,
            } => self.apply_feed(player, creature_id, inventory_slot),
            Intent::PlaceOnPlot {
                player,
                plot_id,
                item,
                inventory_slot,
            } => self.apply_place_on_plot(player, plot_id, item, inventory_slot),
            Intent::RemoveFromPlot { player, plot_id } => {
                self.apply_remove_from_plot(player, plot_id)
            }
            Intent::ClaimQuestReward { player, quest_id } => {
                self.apply_claim_quest_reward(player, quest_id)
            }
            Intent::CreateGroup { group_id, members } => self.apply_create_group(group_id, members),
            Intent::JoinGroup { player, group_id } => self.apply_join_group(player, group_id),
            Intent::LeaveGroup { player, group_id } => self.apply_leave_group(player, group_id),
        }
    }

    /// Helper: find the inventory index for a player (linear scan, small N).
    fn find_inventory(&self, player: PlayerId) -> Option<usize> {
        self.inventories.iter().position(|(p, _)| *p == player)
    }

    /// Helper: find the quest config by id.
    fn find_quest_config(&self, quest_id: QuestId) -> Option<&QuestConfig> {
        self.config.quests.iter().find(|q| q.id == quest_id)
    }

    /// Helper: find the creature config by id.
    fn find_creature_config(&self, creature_id: CreatureId) -> Option<&CreatureConfig> {
        self.config.creatures.iter().find(|c| c.id == creature_id)
    }

    /// Helper: find the resource config and node offset for a flat node index.
    fn find_resource_for_node(&self, node_idx: usize) -> Option<(&ResourceConfig, usize)> {
        let mut offset = 0;
        for rc in &self.config.resources {
            if node_idx < offset + rc.count as usize {
                return Some((rc, node_idx - offset));
            }
            offset += rc.count as usize;
        }
        None
    }

    fn find_plot(&self, plot_id: PlotId) -> Option<usize> {
        self.plots.iter().position(|(id, _)| *id == plot_id)
    }

    fn find_group(&self, group_id: GroupId) -> Option<usize> {
        self.groups.iter().position(|(id, _)| *id == group_id)
    }

    fn find_quest_progress(&self, player: PlayerId, quest_id: QuestId) -> Option<usize> {
        self.quest_progress
            .iter()
            .position(|(p, q, _)| *p == player && *q == quest_id)
    }

    fn find_creature_bond(&self, player: PlayerId, creature_id: CreatureId) -> Option<usize> {
        self.creature_bonds
            .iter()
            .position(|(p, c, _)| *p == player && *c == creature_id)
    }

    // -----------------------------------------------------------------------
    // Reducers
    // -----------------------------------------------------------------------

    fn apply_tick(&mut self) -> Result<GameEvent, GameError> {
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

    fn apply_join_player(&mut self, player: PlayerId) -> Result<GameEvent, GameError> {
        if self.players.contains(&player) {
            // Idempotent: player already exists.
            return Ok(GameEvent::PlayerJoined { player });
        }
        self.players.push(player);
        self.inventories.push((player, Inventory::default()));
        Ok(GameEvent::PlayerJoined { player })
    }

    fn apply_collect(
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

        // Extract config data before mutable borrows.
        let (yield_amount, resource_kind, respawn_seconds) = {
            let (res_config, _) = self
                .find_resource_for_node(resource_node)
                .ok_or(GameError::UnknownResourceNode(resource_node))?;
            (
                res_config.yield_amount,
                res_config.kind,
                res_config.respawn_seconds,
            )
        };

        let kind = ItemKind::Resource(resource_kind);

        // Add to inventory (mutable borrow on inventories)
        let inventory = &mut self.inventories[inv_idx].1;
        inventory.add(kind, yield_amount)?;

        // Mark node as unavailable and schedule respawn
        let respawn_ticks = (respawn_seconds * crate::constants::TICK_RATE_HZ as f32) as u64;
        let node = &mut self.resource_nodes[resource_node];
        node.available = false;
        node.respawn_at_tick = self.tick + respawn_ticks;

        Ok(GameEvent::ResourceCollected {
            player,
            resource_node,
            amount: yield_amount,
        })
    }

    fn apply_feed(
        &mut self,
        player: PlayerId,
        creature_id: CreatureId,
        inventory_slot: usize,
    ) -> Result<GameEvent, GameError> {
        // Validate creature exists
        let creature_config = self
            .find_creature_config(creature_id)
            .ok_or(GameError::UnknownCreature(creature_id))?;

        let inv_idx = self
            .find_inventory(player)
            .ok_or(GameError::UnknownPlayer(player))?;

        // Check slot has an item
        let inventory = &self.inventories[inv_idx].1;
        let slot = inventory
            .slots
            .get(inventory_slot)
            .and_then(|s| s.as_ref())
            .ok_or(GameError::EmptyInventorySlot(inventory_slot))?;

        // Check item is the right food
        let expected_food = ItemKind::Resource(creature_config.food_kind);
        if slot.kind != expected_food {
            return Err(GameError::WrongFood(inventory_slot));
        }

        // Consume one food item
        let inventory = &mut self.inventories[inv_idx].1;
        inventory.remove(inventory_slot, 1)?;

        // Increment bond
        let bond_idx = self.find_creature_bond(player, creature_id);
        let bond_level = if let Some(idx) = bond_idx {
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

    fn apply_place_on_plot(
        &mut self,
        player: PlayerId,
        plot_id: PlotId,
        item: ItemKind,
        inventory_slot: usize,
    ) -> Result<GameEvent, GameError> {
        let inv_idx = self
            .find_inventory(player)
            .ok_or(GameError::UnknownPlayer(player))?;

        let plot_idx = self.find_plot(plot_id);
        if let Some(idx) = plot_idx {
            // Plot exists — must be owned by player and empty
            let plot = &self.plots[idx].1;
            if plot.owner != player {
                return Err(GameError::NotPlotOwner(player, plot_id));
            }
            if plot.content != PlotContent::Empty {
                return Err(GameError::PlotOccupied(plot_id));
            }
        }

        // Remove item from inventory
        let inventory = &mut self.inventories[inv_idx].1;
        inventory.remove(inventory_slot, 1)?;

        // Create or update plot
        if let Some(idx) = plot_idx {
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

    fn apply_remove_from_plot(
        &mut self,
        player: PlayerId,
        plot_id: PlotId,
    ) -> Result<GameEvent, GameError> {
        let plot_idx = self
            .find_plot(plot_id)
            .ok_or(GameError::UnknownPlot(plot_id))?;

        let plot = &self.plots[plot_idx].1;
        if plot.owner != player {
            return Err(GameError::NotPlotOwner(player, plot_id));
        }

        // Only non-empty plots can be removed from
        if plot.content == PlotContent::Empty {
            return Err(GameError::PlotOccupied(plot_id)); // reuse: nothing to remove
        }

        // Reclaim the item into inventory
        let inv_idx = self
            .find_inventory(player)
            .ok_or(GameError::UnknownPlayer(player))?;

        let item_kind = match &plot.content {
            PlotContent::Item(kind) => *kind,
            PlotContent::Empty => unreachable!(),
        };

        let inventory = &mut self.inventories[inv_idx].1;
        inventory.add(item_kind, 1)?;

        self.plots[plot_idx].1.content = PlotContent::Empty;

        Ok(GameEvent::ItemRemoved { player, plot_id })
    }

    fn apply_claim_quest_reward(
        &mut self,
        player: PlayerId,
        quest_id: QuestId,
    ) -> Result<GameEvent, GameError> {
        // Check quest exists
        let quest_config = self
            .find_quest_config(quest_id)
            .ok_or(GameError::UnknownQuest(quest_id))?;

        // Check not already claimed
        if self
            .claimed_rewards
            .iter()
            .any(|(p, q)| *p == player && *q == quest_id)
        {
            return Err(GameError::RewardsAlreadyClaimed(quest_id));
        }

        // Check player exists
        self.find_inventory(player)
            .ok_or(GameError::UnknownPlayer(player))?;

        // Check quest is completed
        let progress_idx = self
            .find_quest_progress(player, quest_id)
            .ok_or(GameError::QuestNotCompleted(quest_id))?;

        if !self.quest_progress[progress_idx].2.completed {
            return Err(GameError::QuestNotCompleted(quest_id));
        }

        // Grant rewards
        let rewards: Vec<ItemKind> = quest_config
            .rewards
            .iter()
            .map(|r| match r {
                QuestReward::Item(kind) => *kind,
            })
            .collect();

        let inv_idx = self.find_inventory(player).unwrap();
        for item in &rewards {
            self.inventories[inv_idx].1.add(*item, 1)?;
        }

        self.claimed_rewards.push((player, quest_id));

        Ok(GameEvent::QuestRewardClaimed {
            player,
            quest_id,
            rewards,
        })
    }

    fn apply_create_group(
        &mut self,
        group_id: GroupId,
        members: Vec<PlayerId>,
    ) -> Result<GameEvent, GameError> {
        if self.find_group(group_id).is_some() {
            // Group already exists — idempotent
            return Ok(GameEvent::GroupCreated { group_id });
        }

        for &player in &members {
            if !self.players.contains(&player) {
                return Err(GameError::UnknownPlayer(player));
            }
        }

        self.groups.push((
            group_id,
            Group {
                id: group_id,
                members: members.clone(),
            },
        ));

        Ok(GameEvent::GroupCreated { group_id })
    }

    fn apply_join_group(
        &mut self,
        player: PlayerId,
        group_id: GroupId,
    ) -> Result<GameEvent, GameError> {
        let group_idx = self
            .find_group(group_id)
            .ok_or(GameError::UnknownGroup(group_id))?;

        if !self.players.contains(&player) {
            return Err(GameError::UnknownPlayer(player));
        }

        let group = &mut self.groups[group_idx].1;
        if group.members.contains(&player) {
            return Err(GameError::AlreadyInGroup(player, group_id));
        }

        group.members.push(player);
        Ok(GameEvent::PlayerJoinedGroup { player, group_id })
    }

    fn apply_leave_group(
        &mut self,
        player: PlayerId,
        group_id: GroupId,
    ) -> Result<GameEvent, GameError> {
        let group_idx = self
            .find_group(group_id)
            .ok_or(GameError::UnknownGroup(group_id))?;

        let group = &mut self.groups[group_idx].1;
        let pos = group
            .members
            .iter()
            .position(|p| *p == player)
            .ok_or(GameError::NotInGroup(player, group_id))?;

        group.members.remove(pos);
        Ok(GameEvent::PlayerLeftGroup { player, group_id })
    }

    // -----------------------------------------------------------------------
    // Collaborative quest credit
    // -----------------------------------------------------------------------

    /// Mark a quest as collaboratively completed for a group.
    /// Fails if the group has already been credited for this quest.
    pub fn credit_group_quest(
        &mut self,
        group_id: GroupId,
        quest_id: QuestId,
    ) -> Result<(), GameError> {
        if self.find_group(group_id).is_none() {
            return Err(GameError::UnknownGroup(group_id));
        }

        if self
            .group_quest_credits
            .iter()
            .any(|(g, q)| *g == group_id && *q == quest_id)
        {
            return Err(GameError::DuplicateGroupCredit(quest_id, group_id));
        }

        self.group_quest_credits.push((group_id, quest_id));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{INVENTORY_CAPACITY, MAX_STACK_SIZE};
    use crate::id::ResourceId;
    use crate::resources::ResourceKind;
    use crate::world_config::{CreatureKind, ObjectiveKind, QuestObjective};
    use glam::Vec3;

    // Shared test config — accessible from other test modules.
    #[allow(dead_code)]
    pub fn test_config() -> WorldConfig {
        WorldConfig {
            resources: vec![ResourceConfig {
                id: ResourceId::new(1),
                kind: ResourceKind::Berry,
                count: 2,
                yield_amount: 3,
                respawn_seconds: 20.0,
                positions: vec![Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)],
                model_path: "berry.glb".into(),
            }],
            creatures: vec![
                CreatureConfig {
                    id: CreatureId::new(1),
                    kind: CreatureKind::Fluffball,
                    center: Vec3::new(0.0, 0.0, 0.0),
                    wander_radius: 5.0,
                    food_kind: ResourceKind::Berry,
                    model_path: "fluff.glb".into(),
                },
                CreatureConfig {
                    id: CreatureId::new(2),
                    kind: CreatureKind::Glimmerwing,
                    center: Vec3::new(5.0, 0.0, 5.0),
                    wander_radius: 4.0,
                    food_kind: ResourceKind::Crystal,
                    model_path: "glim.glb".into(),
                },
            ],
            quests: vec![QuestConfig {
                id: QuestId::new(1),
                title: "Test Quest".into(),
                description: ".".into(),
                objectives: vec![QuestObjective {
                    kind: ObjectiveKind::Collect(ResourceKind::Berry),
                    target_quantity: 5,
                }],
                rewards: vec![QuestReward::Item(ItemKind::Resource(ResourceKind::Fiber))],
            }],
        }
    }

    fn berry_item() -> ItemKind {
        ItemKind::Resource(ResourceKind::Berry)
    }

    // -----------------------------------------------------------------------
    // RED tests — success paths
    // -----------------------------------------------------------------------

    #[test]
    fn collect_from_resource_adds_to_inventory() {
        let config = test_config();
        let mut state = GameState::new(42, config);
        let player = PlayerId::new(1);

        // Player joins
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Collect from node 0 (Berry, yield_amount = 3)
        let event = state
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap();

        assert_eq!(
            event,
            GameEvent::ResourceCollected {
                player,
                resource_node: 0,
                amount: 3,
            }
        );

        // Node should now be unavailable
        assert!(!state.resource_nodes[0].available);

        // Player should have 3 berries
        let (_, inv) = state
            .inventories
            .iter()
            .find(|(p, _)| *p == player)
            .unwrap();
        assert_eq!(inv.count_item_kind(&berry_item()), 3);
    }

    #[test]
    fn tick_respawns_resource_node() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Collect (respawn_seconds=20, TICK_RATE=30 → 600 ticks)
        state
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap();
        assert!(!state.resource_nodes[0].available);

        // Advance time past respawn
        for _ in 0..601 {
            state.apply(Intent::Tick).unwrap();
        }

        // Node should be available again
        assert!(state.resource_nodes[0].available);
    }

    #[test]
    fn collect_from_depleted_resource_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Collect once (depletes node 0)
        state
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap();

        // Try again — should fail
        let err = state
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap_err();
        assert!(matches!(err, GameError::ResourceNotAvailable(0, _)));
    }

    #[test]
    fn collect_with_full_inventory_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Fill inventory by adding items to each slot
        // Default capacity is 24 slots, max_stack is 99
        // We can fill by just adding a LOT of items
        let wood = ItemKind::Resource(ResourceKind::Wood);
        for _ in 0..INVENTORY_CAPACITY {
            // Each add fills one slot with 99 wood (until all 24 slots full)
            // Actually, add will stack in existing slots first
        }

        // Simpler: fill manually
        {
            let (_, inv) = state
                .inventories
                .iter_mut()
                .find(|(p, _)| *p == player)
                .unwrap();
            // Fill all slots
            for i in 0..INVENTORY_CAPACITY {
                inv.slots[i] = Some(crate::inventory::ItemStack::new(wood, MAX_STACK_SIZE));
            }
        }

        // Now collect should fail
        let err = state
            .apply(Intent::Collect {
                player,
                resource_node: 1,
            })
            .unwrap_err();
        assert!(matches!(err, GameError::Inventory(InventoryError::Full)));
    }

    #[test]
    fn feed_creature_increases_bond() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Collect berries (node 0 → 3 berries in slot 0)
        state
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap();

        // Feed creature 1 (Fluffball, food=Berry) from slot 0
        let event = state
            .apply(Intent::Feed {
                player,
                creature_id: CreatureId::new(1),
                inventory_slot: 0,
            })
            .unwrap();

        assert_eq!(
            event,
            GameEvent::CreatureFed {
                player,
                creature_id: CreatureId::new(1),
                bond_level: 1,
            }
        );

        // Bond level should be 1
        let (_, _, bond) = state
            .creature_bonds
            .iter()
            .find(|(p, c, _)| *p == player && *c == CreatureId::new(1))
            .unwrap();
        assert_eq!(*bond, 1);
    }

    #[test]
    fn feed_without_food_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Try to feed creature with empty inventory
        let err = state
            .apply(Intent::Feed {
                player,
                creature_id: CreatureId::new(1),
                inventory_slot: 0,
            })
            .unwrap_err();

        assert!(matches!(err, GameError::EmptyInventorySlot(0)));
    }

    #[test]
    fn feed_wrong_food_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Manually put non-food item in slot 0
        {
            let (_, inv) = state
                .inventories
                .iter_mut()
                .find(|(p, _)| *p == player)
                .unwrap();
            inv.slots[0] = Some(crate::inventory::ItemStack::new(
                ItemKind::Resource(ResourceKind::Wood),
                1,
            ));
        }

        // Try to feed Fluffball (eats Berry) with Wood
        let err = state
            .apply(Intent::Feed {
                player,
                creature_id: CreatureId::new(1),
                inventory_slot: 0,
            })
            .unwrap_err();

        assert!(matches!(err, GameError::WrongFood(0)));
    }

    // -----------------------------------------------------------------------
    // Quest reward idempotence
    // -----------------------------------------------------------------------

    #[test]
    fn quest_reward_granted_only_once() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Complete the quest by collecting enough berries
        // Objective: Collect 5 Berry. Node yield = 3, so collect from both nodes
        state
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap(); // +3
        state
            .apply(Intent::Collect {
                player,
                resource_node: 1,
            })
            .unwrap(); // +3 → total 6

        // Mark quest progress (simulate completion)
        let quest_id = QuestId::new(1);
        let progress_idx = state.find_quest_progress(player, quest_id);
        if let Some(idx) = progress_idx {
            state.quest_progress[idx].2.objective_progress[0] = 6;
            state.quest_progress[idx].2.completed = true;
        } else {
            state.quest_progress.push((
                player,
                quest_id,
                QuestProgress {
                    objective_progress: vec![6],
                    completed: true,
                },
            ));
        }

        // Claim reward — first time succeeds
        let event = state
            .apply(Intent::ClaimQuestReward { player, quest_id })
            .unwrap();
        assert!(matches!(event, GameEvent::QuestRewardClaimed { .. }));

        // Claim reward — second time fails
        let err = state
            .apply(Intent::ClaimQuestReward { player, quest_id })
            .unwrap_err();
        assert!(matches!(err, GameError::RewardsAlreadyClaimed(QuestId(1))));
    }

    // -----------------------------------------------------------------------
    // Non-owner plot mutation
    // -----------------------------------------------------------------------

    #[test]
    fn non_owner_plot_mutation_rejected() {
        let mut state = GameState::new(42, test_config());
        let owner = PlayerId::new(1);
        let intruder = PlayerId::new(2);
        state.apply(Intent::JoinPlayer { player: owner }).unwrap();
        state
            .apply(Intent::JoinPlayer { player: intruder })
            .unwrap();

        let plot_id = PlotId(1);

        // Owner places an item
        {
            let (_, inv) = state
                .inventories
                .iter_mut()
                .find(|(p, _)| *p == owner)
                .unwrap();
            inv.slots[0] = Some(crate::inventory::ItemStack::new(berry_item(), 1));
        }
        state
            .apply(Intent::PlaceOnPlot {
                player: owner,
                plot_id,
                item: berry_item(),
                inventory_slot: 0,
            })
            .unwrap();

        // Intruder tries to remove — should fail
        let err = state
            .apply(Intent::RemoveFromPlot {
                player: intruder,
                plot_id,
            })
            .unwrap_err();
        assert!(matches!(err, GameError::NotPlotOwner(_, _)));

        // Intruder tries to place — should also fail (not owner)
        {
            let (_, inv) = state
                .inventories
                .iter_mut()
                .find(|(p, _)| *p == intruder)
                .unwrap();
            inv.slots[0] = Some(crate::inventory::ItemStack::new(berry_item(), 1));
        }
        let err = state
            .apply(Intent::PlaceOnPlot {
                player: intruder,
                plot_id,
                item: berry_item(),
                inventory_slot: 0,
            })
            .unwrap_err();
        assert!(matches!(err, GameError::NotPlotOwner(_, _)));
    }

    // -----------------------------------------------------------------------
    // Duplicate group credit
    // -----------------------------------------------------------------------

    #[test]
    fn duplicate_group_credit_rejected() {
        let mut state = GameState::new(42, test_config());
        let player_a = PlayerId::new(1);
        let player_b = PlayerId::new(2);
        state
            .apply(Intent::JoinPlayer { player: player_a })
            .unwrap();
        state
            .apply(Intent::JoinPlayer { player: player_b })
            .unwrap();

        let group_id = GroupId(1);
        state
            .apply(Intent::CreateGroup {
                group_id,
                members: vec![player_a, player_b],
            })
            .unwrap();

        let quest_id = QuestId::new(1);

        // First credit succeeds
        state.credit_group_quest(group_id, quest_id).unwrap();

        // Second credit fails
        let err = state.credit_group_quest(group_id, quest_id).unwrap_err();
        assert!(matches!(
            err,
            GameError::DuplicateGroupCredit(QuestId(1), GroupId(1))
        ));
    }

    // -----------------------------------------------------------------------
    // Plot place / remove lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn plot_lifecycle_place_and_remove() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();
        let plot_id = PlotId(10);

        // Put item in inventory
        {
            let (_, inv) = state
                .inventories
                .iter_mut()
                .find(|(p, _)| *p == player)
                .unwrap();
            inv.slots[0] = Some(crate::inventory::ItemStack::new(berry_item(), 1));
        }

        // Place on plot
        state
            .apply(Intent::PlaceOnPlot {
                player,
                plot_id,
                item: berry_item(),
                inventory_slot: 0,
            })
            .unwrap();

        assert_eq!(state.plots[0].1.content, PlotContent::Item(berry_item()));
        assert_eq!(state.plots[0].1.owner, player);

        // Remove from plot
        state
            .apply(Intent::RemoveFromPlot { player, plot_id })
            .unwrap();
        assert_eq!(state.plots[0].1.content, PlotContent::Empty);

        // Item should be back in inventory
        let (_, inv) = state
            .inventories
            .iter()
            .find(|(p, _)| *p == player)
            .unwrap();
        assert_eq!(inv.count_item_kind(&berry_item()), 1);
    }

    // -----------------------------------------------------------------------
    // Group membership lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn group_join_leave_lifecycle() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();
        let group_id = GroupId(5);

        // Create group
        state
            .apply(Intent::CreateGroup {
                group_id,
                members: vec![],
            })
            .unwrap();

        // Join
        state.apply(Intent::JoinGroup { player, group_id }).unwrap();

        let (_, group) = state.groups.iter().find(|(id, _)| *id == group_id).unwrap();
        assert!(group.members.contains(&player));

        // Leave
        state
            .apply(Intent::LeaveGroup { player, group_id })
            .unwrap();
        let (_, group) = state.groups.iter().find(|(id, _)| *id == group_id).unwrap();
        assert!(!group.members.contains(&player));
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn join_player_is_idempotent() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);

        state.apply(Intent::JoinPlayer { player }).unwrap();
        let count_before = state.inventories.len();

        state.apply(Intent::JoinPlayer { player }).unwrap();
        assert_eq!(state.inventories.len(), count_before);
    }

    #[test]
    fn collect_unknown_node_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        let err = state
            .apply(Intent::Collect {
                player,
                resource_node: 999,
            })
            .unwrap_err();
        assert!(matches!(err, GameError::UnknownResourceNode(999)));
    }

    #[test]
    fn collect_unknown_player_fails() {
        let mut state = GameState::new(42, test_config());
        let err = state
            .apply(Intent::Collect {
                player: PlayerId::new(99),
                resource_node: 0,
            })
            .unwrap_err();
        assert!(matches!(err, GameError::UnknownPlayer(PlayerId(99))));
    }

    #[test]
    fn feed_unknown_creature_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        let err = state
            .apply(Intent::Feed {
                player,
                creature_id: CreatureId::new(999),
                inventory_slot: 0,
            })
            .unwrap_err();
        assert!(matches!(err, GameError::UnknownCreature(CreatureId(999))));
    }

    #[test]
    fn claim_reward_for_unknown_quest_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        let err = state
            .apply(Intent::ClaimQuestReward {
                player,
                quest_id: QuestId::new(999),
            })
            .unwrap_err();
        assert!(matches!(err, GameError::UnknownQuest(QuestId(999))));
    }

    #[test]
    fn group_operations_on_unknown_group_fail() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        let err = state
            .apply(Intent::JoinGroup {
                player,
                group_id: GroupId(99),
            })
            .unwrap_err();
        assert!(matches!(err, GameError::UnknownGroup(GroupId(99))));

        let err = state
            .apply(Intent::LeaveGroup {
                player,
                group_id: GroupId(99),
            })
            .unwrap_err();
        assert!(matches!(err, GameError::UnknownGroup(GroupId(99))));
    }

    #[test]
    fn double_join_group_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        let group_id = GroupId(1);
        state
            .apply(Intent::CreateGroup {
                group_id,
                members: vec![player],
            })
            .unwrap();

        let err = state
            .apply(Intent::JoinGroup { player, group_id })
            .unwrap_err();
        assert!(matches!(err, GameError::AlreadyInGroup(_, _)));
    }

    #[test]
    fn leave_group_not_member_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        let group_id = GroupId(1);
        state
            .apply(Intent::CreateGroup {
                group_id,
                members: vec![],
            })
            .unwrap();

        let err = state
            .apply(Intent::LeaveGroup { player, group_id })
            .unwrap_err();
        assert!(matches!(err, GameError::NotInGroup(_, _)));
    }

    #[test]
    fn place_on_occupied_plot_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();
        let plot_id = PlotId(1);

        // Place first item
        {
            let (_, inv) = state
                .inventories
                .iter_mut()
                .find(|(p, _)| *p == player)
                .unwrap();
            inv.slots[0] = Some(crate::inventory::ItemStack::new(berry_item(), 1));
            inv.slots[1] = Some(crate::inventory::ItemStack::new(
                ItemKind::Resource(ResourceKind::Wood),
                1,
            ));
        }
        state
            .apply(Intent::PlaceOnPlot {
                player,
                plot_id,
                item: berry_item(),
                inventory_slot: 0,
            })
            .unwrap();

        // Place second item — should fail (occupied)
        let err = state
            .apply(Intent::PlaceOnPlot {
                player,
                plot_id,
                item: ItemKind::Resource(ResourceKind::Wood),
                inventory_slot: 1,
            })
            .unwrap_err();
        assert!(matches!(err, GameError::PlotOccupied(PlotId(1))));
    }

    #[test]
    fn remove_from_empty_plot_fails() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();
        let plot_id = PlotId(1);

        // Place then remove
        {
            let (_, inv) = state
                .inventories
                .iter_mut()
                .find(|(p, _)| *p == player)
                .unwrap();
            inv.slots[0] = Some(crate::inventory::ItemStack::new(berry_item(), 1));
        }
        state
            .apply(Intent::PlaceOnPlot {
                player,
                plot_id,
                item: berry_item(),
                inventory_slot: 0,
            })
            .unwrap();
        state
            .apply(Intent::RemoveFromPlot { player, plot_id })
            .unwrap();

        // Remove again — should fail (empty)
        let err = state
            .apply(Intent::RemoveFromPlot { player, plot_id })
            .unwrap_err();
        // It should still be the owner's plot, but content is empty
        assert!(
            err == GameError::PlotOccupied(PlotId(1)) || matches!(err, GameError::PlotOccupied(_))
        );
    }
}
