use crate::loading::queue::{ActiveLoad, LabelKind, ManifestEntry, SeqLoader};
use bevy::asset::UntypedHandle;
use std::any::TypeId;

fn make_handle() -> UntypedHandle {
    UntypedHandle::default_for_type(TypeId::of::<bevy::world_serialization::WorldAsset>())
}

fn make_active(path: &str) -> ActiveLoad {
    ActiveLoad {
        path: path.into(),
        handle: make_handle(),
    }
}

fn entry(path: &str, label: LabelKind) -> ManifestEntry {
    ManifestEntry {
        path: path.into(),
        label,
    }
}

// ── can_start_next guards ──────────────────────────────────────────────────

#[test]
fn rejects_when_active_exists() {
    let loader = SeqLoader {
        queue: vec![entry("a.glb", LabelKind::Scene)],
        active: Some(make_active("a.glb")),
        completed: vec![],
        progress: 0,
        total: 1,
        failing_path: None,
    };
    assert!(!loader.can_start_next());
    // Also check queue unchanged
    assert_eq!(loader.queue.len(), 1);
}

#[test]
fn rejects_when_queue_empty() {
    let loader = SeqLoader {
        queue: vec![],
        active: Some(make_active("a.glb")),
        completed: vec![],
        progress: 1,
        total: 1,
        failing_path: None,
    };
    assert!(!loader.can_start_next());
}

#[test]
fn rejects_when_frozen_on_failure() {
    let loader = SeqLoader {
        queue: vec![entry("a.glb", LabelKind::Scene)],
        active: None,
        completed: vec![],
        progress: 5,
        total: 10,
        failing_path: Some("broken.glb".into()),
    };
    assert!(!loader.can_start_next());
}

// ── Progress transitions ───────────────────────────────────────────────────

#[test]
fn mark_completed_increments_progress() {
    let mut loader = SeqLoader {
        queue: vec![],
        active: Some(make_active("test.glb")),
        completed: vec![],
        progress: 5,
        total: 10,
        failing_path: None,
    };
    loader.mark_active_completed();
    assert_eq!(loader.progress, 6);
    assert_eq!(loader.completed.len(), 1);
}

#[test]
fn mark_failed_records_path() {
    let mut loader = SeqLoader {
        queue: vec![entry("queued.glb", LabelKind::Scene)],
        active: Some(make_active("broken.glb")),
        completed: vec![],
        progress: 5,
        total: 10,
        failing_path: None,
    };
    loader.mark_active_failed();
    assert_eq!(loader.failing_path.as_deref(), Some("broken.glb"));
    assert!(!loader.can_start_next());
    assert_eq!(loader.queue.len(), 1, "queue preserved after failure");
}

#[test]
fn zero_total_is_immediately_finished() {
    let loader = SeqLoader {
        queue: vec![],
        active: None,
        completed: vec![],
        progress: 0,
        total: 0,
        failing_path: None,
    };
    assert!(loader.is_finished());
    assert!(loader.all_loaded());
}

#[test]
fn full_cycle_0_to_16() {
    let mut loader = SeqLoader::from_config(&super::production_config());
    assert_eq!(loader.total, 16);
    assert_eq!(loader.progress, 0);

    for i in 0..16 {
        assert!(loader.can_start_next(), "can start at step {i}");
        let next = loader.queue.remove(0);
        loader.active = Some(ActiveLoad {
            path: next.path,
            handle: make_handle(),
        });
        loader.mark_active_completed();
        assert_eq!(loader.progress, i + 1, "progress after step {i}");
    }

    assert!(loader.is_finished());
    assert!(loader.all_loaded());
}
