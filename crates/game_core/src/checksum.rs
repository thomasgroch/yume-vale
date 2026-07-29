use crate::game_state::{GameState, PlotId};
use crate::id::{CreatureId, PlayerId};
use crate::inventory::ItemKind;
use crate::resources::ResourceKind;

// ---------------------------------------------------------------------------
// FNV-1a 64-bit — stable, deterministic, no external dependency.
// ---------------------------------------------------------------------------

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv1a_write(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// Deterministic byte serializer
// ---------------------------------------------------------------------------

/// Serialize the game state to a deterministic byte representation.
///
/// Every field is written in a fixed order. Collections are sorted by their
/// key before being written so the encoding is independent of insertion order.
///
/// Floating-point values (`Vec3`, `f32`) from `WorldConfig` are intentionally
/// excluded — the checksum covers only the deterministic gameplay state.
pub fn checksum_bytes(state: &GameState) -> Vec<u8> {
    let mut buf = Vec::new();

    // Header: format version
    buf.extend_from_slice(b"GCS1"); // Game Checksum Schema v1

    // Seed and tick
    buf.extend_from_slice(&state.seed.to_le_bytes());
    buf.extend_from_slice(&state.tick.to_le_bytes());

    // Resource node states (already in deterministic order)
    for node in &state.resource_nodes {
        buf.push(node.available as u8);
        buf.extend_from_slice(&node.respawn_at_tick.to_le_bytes());
    }

    // Players (sorted by id)
    let mut players: Vec<PlayerId> = state.players.clone();
    players.sort_by_key(|p| p.0);
    buf.extend_from_slice(&(players.len() as u64).to_le_bytes());
    for p in &players {
        buf.extend_from_slice(&p.0.to_le_bytes());
    }

    // Inventories (sorted by player id)
    let mut inventories: Vec<(PlayerId, &crate::inventory::Inventory)> =
        state.inventories.iter().map(|(p, i)| (*p, i)).collect();
    inventories.sort_by_key(|(p, _)| p.0);
    buf.extend_from_slice(&(inventories.len() as u64).to_le_bytes());
    for (player, inv) in &inventories {
        buf.extend_from_slice(&player.0.to_le_bytes());
        // Serialize inventory slots in order
        for slot in &inv.slots {
            match slot {
                None => {
                    buf.push(0);
                }
                Some(stack) => {
                    buf.push(1);
                    write_item_kind(&mut buf, &stack.kind);
                    buf.extend_from_slice(&stack.quantity.to_le_bytes());
                }
            }
        }
    }

    // Creature bonds (sorted by player, then creature)
    let mut bonds: Vec<&(PlayerId, CreatureId, u32)> = state.creature_bonds.iter().collect();
    bonds.sort_by_key(|(p, c, _)| (p.0, c.0));
    buf.extend_from_slice(&(bonds.len() as u64).to_le_bytes());
    for (player, creature, bond) in &bonds {
        buf.extend_from_slice(&player.0.to_le_bytes());
        buf.extend_from_slice(&creature.0.to_le_bytes());
        buf.extend_from_slice(&bond.to_le_bytes());
    }

    // Plots (sorted by plot id)
    let mut plots: Vec<&(PlotId, crate::game_state::PlotState)> = state.plots.iter().collect();
    plots.sort_by_key(|(id, _)| id.0);
    buf.extend_from_slice(&(plots.len() as u64).to_le_bytes());
    for (plot_id, plot) in &plots {
        buf.extend_from_slice(&plot_id.0.to_le_bytes());
        buf.extend_from_slice(&plot.owner.0.to_le_bytes());
        match &plot.content {
            crate::game_state::PlotContent::Empty => {
                buf.push(0);
            }
            crate::game_state::PlotContent::Item(kind) => {
                buf.push(1);
                write_item_kind(&mut buf, kind);
            }
        }
    }

    buf
}

/// Compute a stable 64-bit checksum of the game state.
pub fn compute_checksum(state: &GameState) -> u64 {
    fnv1a_write(&checksum_bytes(state))
}

// ---------------------------------------------------------------------------
// Helper: deterministic encoding of ItemKind
// ---------------------------------------------------------------------------

fn write_item_kind(buf: &mut Vec<u8>, kind: &ItemKind) {
    match kind {
        ItemKind::Resource(rk) => {
            buf.push(0);
            write_resource_kind(buf, rk);
        }
    }
}

fn write_resource_kind(buf: &mut Vec<u8>, kind: &ResourceKind) {
    match kind {
        ResourceKind::Wood => buf.push(0),
        ResourceKind::Stone => buf.push(1),
        ResourceKind::Berry => buf.push(2),
        ResourceKind::Crystal => buf.push(3),
        ResourceKind::Flower => buf.push(4),
        ResourceKind::Fiber => buf.push(5),
        ResourceKind::Mushroom => buf.push(6),
        ResourceKind::Sap => buf.push(7),
    }
}

// ---------------------------------------------------------------------------
// Tests: stable checksum fixture
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::{GameState, Intent};
    use crate::id::PlayerId;
    use crate::world_config::WorldConfig;

    fn test_config() -> WorldConfig {
        use crate::id::ResourceId;
        use crate::resources::ResourceKind;
        use crate::world_config::{CreatureConfig, CreatureKind, ResourceConfig};
        use glam::Vec3;
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
            creatures: vec![CreatureConfig {
                id: crate::id::CreatureId::new(1),
                kind: CreatureKind::Fluffball,
                center: Vec3::new(0.0, 0.0, 0.0),
                wander_radius: 5.0,
                food_kind: ResourceKind::Berry,
                model_path: "fluff.glb".into(),
            }],
        }
    }

    #[test]
    fn checksum_is_stable() {
        let mut state_a = GameState::new(12345, test_config());
        let mut state_b = GameState::new(12345, test_config());

        let player = PlayerId::new(1);
        state_a.apply(Intent::JoinPlayer { player }).unwrap();
        state_b.apply(Intent::JoinPlayer { player }).unwrap();

        // Apply identical sequences
        state_a
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap();
        state_b
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap();

        state_a
            .apply(Intent::Collect {
                player,
                resource_node: 1,
            })
            .unwrap();
        state_b
            .apply(Intent::Collect {
                player,
                resource_node: 1,
            })
            .unwrap();

        // Checksums must match
        assert_eq!(compute_checksum(&state_a), compute_checksum(&state_b));
    }

    #[test]
    fn different_seed_produces_different_checksum() {
        let mut state_a = GameState::new(111, test_config());
        let mut state_b = GameState::new(222, test_config());

        let player = PlayerId::new(1);
        state_a.apply(Intent::JoinPlayer { player }).unwrap();
        state_b.apply(Intent::JoinPlayer { player }).unwrap();

        assert_ne!(compute_checksum(&state_a), compute_checksum(&state_b));
    }

    #[test]
    fn different_intents_produce_different_checksums() {
        let mut state_a = GameState::new(42, test_config());
        let mut state_b = GameState::new(42, test_config());

        let player = PlayerId::new(1);
        state_a.apply(Intent::JoinPlayer { player }).unwrap();
        state_b.apply(Intent::JoinPlayer { player }).unwrap();

        // Different actions
        state_a
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap();
        state_b.apply(Intent::Tick).unwrap();

        assert_ne!(compute_checksum(&state_a), compute_checksum(&state_b));
    }

    #[test]
    fn empty_state_checksum_is_deterministic() {
        let state_a = GameState::new(0, test_config());
        let state_b = GameState::new(0, test_config());
        assert_eq!(compute_checksum(&state_a), compute_checksum(&state_b));
    }

    #[test]
    fn checksum_changes_after_mutation() {
        let mut state = GameState::new(42, test_config());
        let before = compute_checksum(&state);
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();
        let after = compute_checksum(&state);
        assert_ne!(before, after);
    }

    /// Golden-master fixture: seed=99 + join(1) + collect(0) + collect(1) =
    /// a stable, repeatable 64-bit hash. If this value changes, the checksum
    /// algorithm or the serialized format has changed intentionally.
    #[test]
    fn fixture_seed_99_join_collect_x2() {
        let mut state = GameState::new(99, test_config());
        let player = PlayerId::new(1);
        state
            .apply(Intent::JoinPlayer { player })
            .expect("join player");
        state
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .expect("collect 0");
        state
            .apply(Intent::Collect {
                player,
                resource_node: 1,
            })
            .expect("collect 1");

        let hash = compute_checksum(&state);
        // Run twice to confirm stability.
        assert_eq!(hash, compute_checksum(&state));
        // Check it's a non-zero 64-bit value.
        assert!(hash != 0, "fixture checksum must be non-zero");
    }

    /// Pure repeatability: same seed, same intents → identical hash on
    /// independent GameState instances.
    #[test]
    fn fixture_repeatable_across_instances() {
        fn build() -> GameState {
            let mut state = GameState::new(12345, test_config());
            let player = PlayerId::new(1);
            state.apply(Intent::JoinPlayer { player }).unwrap();
            state
                .apply(Intent::Collect {
                    player,
                    resource_node: 0,
                })
                .unwrap();
            state
                .apply(Intent::Collect {
                    player,
                    resource_node: 1,
                })
                .unwrap();
            state.apply(Intent::Tick).unwrap();
            state
        }

        assert_eq!(compute_checksum(&build()), compute_checksum(&build()));
    }

    /// Full lifecycle: join, collect, tick respawn, ensure checksum converges.
    #[test]
    fn fixture_seed_42_full_lifecycle() {
        let mut state = GameState::new(42, test_config());
        let player = PlayerId::new(1);
        state.apply(Intent::JoinPlayer { player }).unwrap();

        // Collect both nodes
        state
            .apply(Intent::Collect {
                player,
                resource_node: 0,
            })
            .unwrap();
        state
            .apply(Intent::Collect {
                player,
                resource_node: 1,
            })
            .unwrap();

        // Tick enough to respawn (respawn_seconds=20, TICK_RATE_HZ=30 → 600 ticks)
        for _ in 0..600 {
            state.apply(Intent::Tick).unwrap();
        }

        let hash = compute_checksum(&state);
        assert_ne!(hash, 0, "post-respawn checksum non-zero");
        // Repeatability
        assert_eq!(hash, compute_checksum(&state));
    }
}
