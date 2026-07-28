//! Systems for quest lifecycle: activation, progress tracking, group credit,
//! reward grant, and snapshot delivery.

use bevy::ecs::observer::On;
use bevy::prelude::*;
use game_core::id::PlayerId;
use game_core::inventory::ItemKind;
use game_core::resources::ResourceKind;
use game_core::world_config::{ObjectiveKind, QuestConfig};
use game_protocol::channels::ReliableChannel;
use game_protocol::messages::{
    InventorySnapshot, ObjectiveProgress, QuestSnapshot, QuestStateData,
    inventory_to_snapshot_items,
};
use lightyear::prelude::MessageSender;
use player::Player;
use resources::PlayerInventory;
use social::systems::{PlayerClientMap, PlayerGroup};
use tracing::info;

use crate::ResourceCollectedEvent;
use crate::components::*;

// ---------------------------------------------------------------------------
// Quest activation
// ---------------------------------------------------------------------------

/// Activate quests for any player entity that lacks a `PlayerQuests` component.
///
/// This runs once per new player spawn (or reconnect): it initialises progress
/// to zero (or loads persisted state when available) so that future collection
/// events can advance the quest.
pub fn initialize_player_quests(
    mut commands: Commands,
    quest_defs: Res<QuestDefs>,
    persistence: Option<Res<QuestPersistence>>,
    query: Query<(Entity, &Player), Without<PlayerQuests>>,
) {
    for (entity, player) in query.iter() {
        let mut quests: Vec<QuestProgressData> = Vec::new();

        for quest_cfg in &quest_defs.configs {
            // Try to load persisted progress
            let persisted = persistence
                .as_ref()
                .and_then(|p| p.0.as_ref())
                .and_then(|handle| {
                    handle
                        .load_quest_progress(player.id.get() as i64, quest_cfg.id.get() as i64)
                        .ok()
                        .flatten()
                });

            match persisted {
                Some(row) => {
                    // The DB stores progress as f64 (0.0 … 1.0).
                    // Reconstruct absolute progress from the fraction × target.
                    let target = quest_cfg
                        .objectives
                        .first()
                        .map(|o| o.target_quantity)
                        .unwrap_or(1);
                    let current = (row.progress * target as f64).round() as u32;
                    quests.push(QuestProgressData {
                        quest_id: quest_cfg.id,
                        objective_index: 0,
                        current: current.min(target),
                        target,
                        completed: row.completed,
                        reward_granted: row.completed, // completed ⇒ reward already granted
                    });
                }
                None => {
                    // Fresh progress — start at 0.
                    let target = quest_cfg
                        .objectives
                        .first()
                        .map(|o| o.target_quantity)
                        .unwrap_or(1);
                    quests.push(QuestProgressData {
                        quest_id: quest_cfg.id,
                        objective_index: 0,
                        current: 0,
                        target,
                        completed: false,
                        reward_granted: false,
                    });
                }
            }
        }

        if !quests.is_empty() {
            info!(
                "initialised quests for player {} ({} quest(s))",
                player.id,
                quests.len()
            );
            commands.entity(entity).insert(PlayerQuests { quests });
        }
    }
}

// ---------------------------------------------------------------------------
// Core: increment quest progress (pure function)
// ---------------------------------------------------------------------------

/// Find the index of an objective in `quest_cfg` that matches the collected
/// resource kind.
pub fn find_matching_objective(
    quest_cfg: &QuestConfig,
    resource_kind: ResourceKind,
) -> Option<usize> {
    quest_cfg
        .objectives
        .iter()
        .position(|obj| matches!(obj.kind, ObjectiveKind::Collect(k) if k == resource_kind))
}

/// Increment progress for a single quest entry.
///
/// Returns `true` if the quest just became completed by this increment.
/// Returns `false` if already completed or not yet completing.
pub fn increment_quest_progress(progress: &mut QuestProgressData, amount: u32) -> bool {
    if progress.completed {
        return false; // Already done — no-op
    }

    progress.current = (progress.current + amount).min(progress.target);

    let just_completed = !progress.completed && progress.current >= progress.target;
    if just_completed {
        progress.completed = true;
    }

    just_completed
}

// ---------------------------------------------------------------------------
// Reward grant (pure function)
// ---------------------------------------------------------------------------

/// Grant all rewards for a completed quest into the player's inventory.
///
/// Returns the list of item kinds granted (used for snapshot + logging).
#[allow(clippy::type_complexity)]
pub fn grant_quest_rewards(
    inventory: &mut PlayerInventory,
    quest_cfg: &QuestConfig,
) -> Vec<ItemKind> {
    let mut granted = Vec::new();
    for reward in &quest_cfg.rewards {
        match reward {
            game_core::world_config::QuestReward::Item(kind) => {
                // Silently ignore full inventory — the item is lost.
                let _ = inventory.inventory.add(*kind, 1);
                granted.push(*kind);
            }
        }
    }
    granted
}

// ---------------------------------------------------------------------------
// Resource-collected event handler
// ---------------------------------------------------------------------------

/// Observer for `ResourceCollectedEvent`: increment matching quest progress
/// for the collecting player and all current group members, grant rewards on
/// completion, and push snapshots to affected clients.
///
/// Registered via `app.observe(on_resource_collected)` in the plugin.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn on_resource_collected(
    trigger: On<ResourceCollectedEvent>,
    quest_defs: Res<QuestDefs>,
    client_map: Res<PlayerClientMap>,
    mut player_query: Query<(
        Entity,
        &Player,
        Option<&PlayerGroup>,
        &mut PlayerQuests,
        Option<&mut PlayerInventory>,
    )>,
    mut quest_senders: Query<&mut MessageSender<QuestSnapshot>>,
    mut inventory_senders: Query<&mut MessageSender<InventorySnapshot>>,
) {
    let event = trigger.event();
    let resource_kind = event.resource_kind;

    // ── Build the set of player IDs who should receive progress ──
    let mut affected_players: Vec<PlayerId> = Vec::new();

    // Always include the collecting player.
    if !affected_players.contains(&event.player_id) {
        affected_players.push(event.player_id);
    }

    // If the collector belongs to a group, include every member.
    for (_, _player, group_opt, _, _) in player_query.iter() {
        if _player.id == event.player_id {
            if let Some(Some(group_id)) = group_opt.map(|g| &g.0) {
                for (_, p2, g2, _, _) in player_query.iter() {
                    if let Some(Some(gid)) = g2.map(|g| &g.0)
                        && gid == group_id
                        && !affected_players.contains(&p2.id)
                    {
                        affected_players.push(p2.id);
                    }
                }
            }
            break;
        }
    }

    // ── Apply progress to each affected player ──────────────────
    for &pid in &affected_players {
        let mut entity = None;

        for (e, p, _pg, mut quests, mut inv_opt) in player_query.iter_mut() {
            if p.id != pid {
                continue;
            }
            entity = Some(e);

            for quest_cfg in &quest_defs.configs {
                if find_matching_objective(quest_cfg, resource_kind).is_none() {
                    continue;
                }
                let Some(prog) = quests.find_quest_mut(quest_cfg.id) else {
                    continue;
                };

                let just_completed = increment_quest_progress(prog, 1);

                if just_completed
                    && !prog.reward_granted
                    && let Some(ref mut inv) = inv_opt
                {
                    let granted = grant_quest_rewards(inv, quest_cfg);
                    prog.reward_granted = true;
                    info!(
                        "quest {} completed by player {} — rewards: {:?}",
                        quest_cfg.id, pid, granted
                    );

                    if let Some(client_entity) = client_map.get(pid)
                        && let Ok(mut sender) = inventory_senders.get_mut(client_entity)
                    {
                        sender.send::<ReliableChannel>(InventorySnapshot {
                            items: inventory_to_snapshot_items(&inv.inventory),
                        });
                    }
                }
            }
        }

        // ── Send QuestSnapshot for this player ─────────────────
        if let Some(player_entity) = entity
            && let Some(client_entity) = client_map.get(pid)
            && let Ok(mut sender) = quest_senders.get_mut(client_entity)
        {
            let snapshot = build_quest_snapshot(player_entity, &quest_defs, &player_query);
            sender.send::<ReliableChannel>(snapshot);
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot builder
// ---------------------------------------------------------------------------

/// Build a `QuestSnapshot` for the player identified by `player_entity`.
#[allow(clippy::type_complexity)]
fn build_quest_snapshot(
    player_entity: Entity,
    quest_defs: &QuestDefs,
    player_query: &Query<(
        Entity,
        &Player,
        Option<&PlayerGroup>,
        &mut PlayerQuests,
        Option<&mut PlayerInventory>,
    )>,
) -> QuestSnapshot {
    let Ok((_, _, _, quests, _)) = player_query.get(player_entity) else {
        return QuestSnapshot { quests: Vec::new() };
    };

    let quests_data: Vec<QuestStateData> = quest_defs
        .configs
        .iter()
        .map(|cfg| {
            let prog = quests.find_quest(cfg.id);
            let progress = cfg
                .objectives
                .iter()
                .enumerate()
                .map(|(i, obj)| {
                    let current = prog
                        .filter(|p| p.objective_index == i)
                        .map(|p| p.current)
                        .unwrap_or(0);
                    ObjectiveProgress {
                        objective_index: i as u8,
                        current,
                        target: obj.target_quantity,
                    }
                })
                .collect();
            QuestStateData {
                quest_id: cfg.id.get(),
                completed: prog.map(|p| p.completed).unwrap_or(false),
                progress,
            }
        })
        .collect();

    QuestSnapshot {
        quests: quests_data,
    }
}

// ---------------------------------------------------------------------------
// Send snapshot to all players (e.g., on reconnect)
// ---------------------------------------------------------------------------

/// Send the current `QuestSnapshot` to a specific client.
#[allow(clippy::type_complexity)]
pub fn send_quest_snapshot(
    client_entity: Entity,
    player_entity: Entity,
    quest_defs: &QuestDefs,
    player_query: &Query<(
        Entity,
        &Player,
        Option<&PlayerGroup>,
        &mut PlayerQuests,
        Option<&mut PlayerInventory>,
    )>,
    senders: &mut Query<&mut MessageSender<QuestSnapshot>>,
) {
    if let Ok(mut sender) = senders.get_mut(client_entity) {
        let snapshot = build_quest_snapshot(player_entity, quest_defs, player_query);
        sender.send::<ReliableChannel>(snapshot);
    }
}

// ---------------------------------------------------------------------------
// Persistence sync
// ---------------------------------------------------------------------------

/// Persist quest progress for all players to the database (best-effort).
///
/// Runs periodically so reconnecting players see their progress.
pub fn persist_quest_progress(
    _quest_defs: Res<QuestDefs>,
    persistence: Option<Res<QuestPersistence>>,
    player_query: Query<(&Player, &PlayerQuests)>,
) {
    let Some(p_ref) = persistence.as_ref().and_then(|p| p.0.as_ref()) else {
        return;
    };

    for (player, quests) in player_query.iter() {
        for prog in &quests.quests {
            let target = prog.target.max(1);
            let progress_f64 = (prog.current as f64) / (target as f64);
            let _ = p_ref.save_quest_progress(
                player.id.get() as i64,
                prog.quest_id.get() as i64,
                progress_f64.clamp(0.0, 1.0),
                prog.completed,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::id::QuestId;
    use game_core::inventory::Inventory;
    use game_core::resources::ResourceKind;
    use game_core::world_config::{QuestConfig, QuestObjective, QuestReward};

    // -----------------------------------------------------------------------
    // Pure-function tests: find_matching_objective
    // -----------------------------------------------------------------------

    fn berry_quest() -> QuestConfig {
        QuestConfig {
            id: QuestId::new(1),
            title: "Berry Good Start".into(),
            description: "".into(),
            objectives: vec![QuestObjective {
                kind: ObjectiveKind::Collect(ResourceKind::Berry),
                target_quantity: 5,
            }],
            rewards: vec![QuestReward::Item(ItemKind::Resource(ResourceKind::Fiber))],
        }
    }

    #[allow(dead_code)]
    fn wood_quest() -> QuestConfig {
        QuestConfig {
            id: QuestId::new(2),
            title: "Wood Gatherer".into(),
            description: "".into(),
            objectives: vec![QuestObjective {
                kind: ObjectiveKind::Collect(ResourceKind::Wood),
                target_quantity: 3,
            }],
            rewards: vec![QuestReward::Item(ItemKind::Resource(ResourceKind::Fiber))],
        }
    }

    #[test]
    fn find_matching_objective_berry() {
        let q = berry_quest();
        assert_eq!(find_matching_objective(&q, ResourceKind::Berry), Some(0));
    }

    #[test]
    fn find_matching_objective_no_match() {
        let q = berry_quest();
        assert_eq!(find_matching_objective(&q, ResourceKind::Wood), None);
    }

    // -----------------------------------------------------------------------
    // Pure-function tests: increment_quest_progress
    // -----------------------------------------------------------------------

    #[test]
    fn increment_progress_0_to_5() {
        let mut p = QuestProgressData {
            quest_id: QuestId::new(1),
            objective_index: 0,
            current: 0,
            target: 5,
            completed: false,
            reward_granted: false,
        };
        for i in 0..5 {
            let completed = increment_quest_progress(&mut p, 1);
            assert_eq!(p.current, i + 1);
            if i < 4 {
                assert!(!completed);
                assert!(!p.completed);
            } else {
                assert!(completed);
                assert!(p.completed);
            }
        }
    }

    #[test]
    fn increment_progress_target_overflow_clamped() {
        let mut p = QuestProgressData {
            quest_id: QuestId::new(1),
            objective_index: 0,
            current: 3,
            target: 5,
            completed: false,
            reward_granted: false,
        };
        increment_quest_progress(&mut p, 10); // would overflow
        assert_eq!(p.current, 5);
        assert!(p.completed);
    }

    #[test]
    fn increment_progress_already_completed_noop() {
        let mut p = QuestProgressData {
            quest_id: QuestId::new(1),
            objective_index: 0,
            current: 5,
            target: 5,
            completed: true,
            reward_granted: true,
        };
        let completed = increment_quest_progress(&mut p, 1);
        assert!(!completed);
        assert_eq!(p.current, 5);
    }

    // -----------------------------------------------------------------------
    // Pure-function tests: grant_quest_rewards
    // -----------------------------------------------------------------------

    #[test]
    fn grant_quest_rewards_adds_fiber() {
        let q = berry_quest();
        let mut inv = PlayerInventory {
            inventory: Inventory::default(),
        };
        let granted = grant_quest_rewards(&mut inv, &q);
        assert_eq!(granted, vec![ItemKind::Resource(ResourceKind::Fiber)]);
        assert_eq!(
            inv.inventory
                .count_item_kind(&ItemKind::Resource(ResourceKind::Fiber)),
            1
        );
    }

    #[test]
    fn grant_quest_rewards_idempotent_manual_check() {
        // Test: granting twice adds two items (the caller prevents duplicate grants)
        let q = berry_quest();
        let mut inv = PlayerInventory {
            inventory: Inventory::default(),
        };
        let granted = grant_quest_rewards(&mut inv, &q);
        assert_eq!(granted.len(), 1);
        let granted2 = grant_quest_rewards(&mut inv, &q);
        assert_eq!(granted2.len(), 1);
        assert_eq!(
            inv.inventory
                .count_item_kind(&ItemKind::Resource(ResourceKind::Fiber)),
            2
        );
    }

    // -----------------------------------------------------------------------
    // PlayerQuests component helpers
    // -----------------------------------------------------------------------

    #[test]
    fn player_quests_find_quest() {
        let pq = PlayerQuests {
            quests: vec![QuestProgressData {
                quest_id: QuestId::new(1),
                objective_index: 0,
                current: 3,
                target: 5,
                completed: false,
                reward_granted: false,
            }],
        };
        assert!(pq.find_quest(QuestId::new(1)).is_some());
        assert!(pq.find_quest(QuestId::new(999)).is_none());
    }

    #[test]
    fn player_quests_find_quest_mut() {
        let mut pq = PlayerQuests {
            quests: vec![QuestProgressData {
                quest_id: QuestId::new(1),
                objective_index: 0,
                current: 3,
                target: 5,
                completed: false,
                reward_granted: false,
            }],
        };
        let prog = pq.find_quest_mut(QuestId::new(1)).unwrap();
        prog.current = 5;
        assert_eq!(pq.quests[0].current, 5);
    }
}
