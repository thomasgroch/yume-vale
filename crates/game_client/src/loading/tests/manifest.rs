use crate::loading::queue::paths;
use crate::loading::queue::{LabelKind, SeqLoader};
use game_core::arena::ArenaModel;

// Production manifest = 6 arena + 1 fox rig + 3 resources + 2 creatures + 4 animations = 16.

#[test]
fn manifest_has_exactly_16_entries() {
    let loader = SeqLoader::from_config(&super::production_config());
    assert_eq!(loader.total, 16);
}

#[test]
fn manifest_has_12_scene_entries() {
    let loader = SeqLoader::from_config(&super::production_config());
    let n = loader
        .queue
        .iter()
        .filter(|e| e.label == LabelKind::Scene)
        .count();
    assert_eq!(n, 12);
}

#[test]
fn manifest_has_4_animation_entries() {
    let loader = SeqLoader::from_config(&super::production_config());
    let n = loader
        .queue
        .iter()
        .filter(|e| e.label == LabelKind::Animation)
        .count();
    assert_eq!(n, 4);
}

#[test]
fn manifest_entries_are_unique_by_path_and_label() {
    let loader = SeqLoader::from_config(&super::production_config());
    let mut seen = std::collections::HashSet::new();
    for e in &loader.queue {
        assert!(
            seen.insert((&e.path, e.label)),
            "duplicate: ({:?}, {:?})",
            e.path,
            e.label
        );
    }
}

#[test]
fn fifo_order_arena_then_fox_then_resources_then_creatures_then_animations() {
    let loader = SeqLoader::from_config(&super::production_config());
    let arena_set: std::collections::HashSet<String> = [
        ArenaModel::Portal,
        ArenaModel::Wall,
        ArenaModel::Pillar,
        ArenaModel::CrystalBig,
        ArenaModel::CrystalSmall,
        ArenaModel::Rock,
    ]
    .iter()
    .map(|m| m.asset_path().to_string())
    .collect();
    for (i, e) in loader.queue.iter().enumerate().take(6) {
        assert!(
            arena_set.contains(&e.path),
            "entry {i}: expected arena, got {}",
            e.path
        );
        assert_eq!(e.label, LabelKind::Scene);
    }
    assert_eq!(loader.queue[6].path, paths::FOX_RIG);
    assert_eq!(loader.queue[6].label, LabelKind::Scene);
    for (i, e) in loader.queue.iter().enumerate().rev().take(4) {
        assert_eq!(
            e.label,
            LabelKind::Animation,
            "entry {i}: expected animation clip"
        );
    }
}
