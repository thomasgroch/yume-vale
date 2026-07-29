use crate::id::*;
use crate::inventory::{Inventory, ItemKind};
use crate::world_config::{CreatureConfig, ResourceConfig, WorldConfig};
use serde::{Deserialize, Serialize};

pub(crate) mod error;
pub(crate) mod reducers;
pub(crate) mod types;
pub use error::*;
pub use types::*;

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
}

// ---------------------------------------------------------------------------
// Pure GameState
// ---------------------------------------------------------------------------

/// The entire deterministic simulation state.
///
/// No Bevy entities, clocks, floating-point physics, Tnua, or Avian.
/// All collections are `Vec` (no HashMap) for stable serialization.
/// Players, creatures, and plots are tracked as linear lists.
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
    /// Bond level per (player, creature)
    pub creature_bonds: Vec<(PlayerId, CreatureId, BondLevel)>,
    /// Plot registry.
    pub plots: Vec<(PlotId, PlotState)>,
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
            creature_bonds: Vec::new(),
            plots: Vec::new(),
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
        }
    }

    /// Helper: find the inventory index for a player (linear scan, small N).
    fn find_inventory(&self, player: PlayerId) -> Option<usize> {
        self.inventories.iter().position(|(p, _)| *p == player)
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

    fn find_creature_bond(&self, player: PlayerId, creature_id: CreatureId) -> Option<usize> {
        self.creature_bonds
            .iter()
            .position(|(p, c, _)| *p == player && *c == creature_id)
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
    use crate::inventory::InventoryError;
    use crate::resources::ResourceKind;
    use crate::world_config::CreatureKind;
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

        // Fill inventory manually
        let wood = ItemKind::Resource(ResourceKind::Wood);
        {
            let (_, inv) = state
                .inventories
                .iter_mut()
                .find(|(p, _)| *p == player)
                .unwrap();
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
        assert!(
            err == GameError::PlotOccupied(PlotId(1)) || matches!(err, GameError::PlotOccupied(_))
        );
    }
}
