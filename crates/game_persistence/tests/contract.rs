//! RED repository-contract tests for `game_persistence`.
//!
//! Each test uses an isolated temporary SQLite file so there is no cross-test
//! interference. Tests are ordered by dependency — earlier tests establish
//! that the basic machinery works before we test edge cases.

use std::time::{SystemTime, UNIX_EPOCH};

use game_persistence::worker::PersistenceWorker;
use game_persistence::{CreatureBondRow, InventoryRow, PersistenceError, PlotDecorationRow};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a temporary SQLite URL and a spawned worker.
fn with_worker<F>(f: F)
where
    F: FnOnce(&PersistenceWorker),
{
    let dir = tempfile::tempdir().expect("create temp dir");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.display());

    let worker = PersistenceWorker::spawn(&url, 256).expect("spawn worker");
    worker.handle().migrate().expect("migrate");
    f(&worker);
}

/// Current unix epoch seconds.
fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ---------------------------------------------------------------------------
// 1.  Migrate creates all tables
// ---------------------------------------------------------------------------

#[test]
fn migrate_creates_tables() {
    with_worker(|w| {
        // Migration succeeded in `with_worker`. Verify by inserting into every table.
        let h = w.handle();

        // identities
        h.create_identity("token_a", 100).unwrap();

        // inventory
        h.save_inventory(
            100,
            &[InventoryRow {
                resource_kind: "Wood".into(),
                quantity: 5,
            }],
        )
        .unwrap();

        // creature_bond
        h.save_creature_bond(100, "Fluffball", 3).unwrap();

        // plot_decoration
        h.save_plot_decoration(
            100,
            &PlotDecorationRow {
                decoration_id: 1,
                position_x: 10.0,
                position_z: 20.0,
                rotation_y: 1.57,
            },
        )
        .unwrap();

        // Quick read-back: identity exists.
        let row = h.resolve_identity("token_a").unwrap();
        assert_eq!(row.player_id, 100);
    });
}

// ---------------------------------------------------------------------------
// 2.  Resolve identity — hash lookup + creation
// ---------------------------------------------------------------------------

#[test]
fn resolve_identity_creates_on_first_lookup() {
    with_worker(|w| {
        let h = w.handle();

        // First lookup: creates a new identity.
        let row = h.resolve_identity("new_hash").unwrap();
        assert!(row.player_id > 0, "player_id must be positive");
        assert!(
            row.created_at <= now_epoch() + 2,
            "created_at must be recent"
        );

        // Second lookup: returns the same identity.
        let row2 = h.resolve_identity("new_hash").unwrap();
        assert_eq!(row.player_id, row2.player_id);
        assert_eq!(row.created_at, row2.created_at);
    });
}

#[test]
fn resolve_identity_returns_existing() {
    with_worker(|w| {
        let h = w.handle();
        h.create_identity("existing", 42).unwrap();

        let row = h.resolve_identity("existing").unwrap();
        assert_eq!(row.player_id, 42);
    });
}

// ---------------------------------------------------------------------------
// 3.  Save + restore every persisted slice
// ---------------------------------------------------------------------------

#[test]
fn save_and_load_inventory() {
    with_worker(|w| {
        let h = w.handle();
        let items = vec![
            InventoryRow {
                resource_kind: "Wood".into(),
                quantity: 10,
            },
            InventoryRow {
                resource_kind: "Berry".into(),
                quantity: 25,
            },
            InventoryRow {
                resource_kind: "Crystal".into(),
                quantity: 3,
            },
        ];

        h.save_inventory(1, &items).unwrap();
        let loaded = h.load_inventory(1).unwrap();

        assert_eq!(loaded.len(), 3);
        assert!(loaded.contains(&items[0]));
        assert!(loaded.contains(&items[1]));
        assert!(loaded.contains(&items[2]));
    });
}

#[test]
fn save_and_load_creature_bond() {
    with_worker(|w| {
        let h = w.handle();

        h.save_creature_bond(3, "Glimmerwing", 5).unwrap();
        let loaded = h.load_creature_bond(3, "Glimmerwing").unwrap();
        assert_eq!(
            loaded,
            Some(CreatureBondRow {
                creature_kind: "Glimmerwing".into(),
                bond_level: 5,
            })
        );
    });
}

#[test]
fn save_and_load_plot_decorations() {
    with_worker(|w| {
        let h = w.handle();

        let deco = PlotDecorationRow {
            decoration_id: 1,
            position_x: 12.5,
            position_z: -8.3,
            rotation_y: 2.1,
        };
        h.save_plot_decoration(4, &deco).unwrap();

        let all = h.load_plot_decorations(4).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].decoration_id, 1);
        assert!((all[0].position_x - 12.5).abs() < 1e-9);
        assert!((all[0].position_z + 8.3).abs() < 1e-9);
    });
}

// ---------------------------------------------------------------------------
// 4.  Idempotent upsert
// ---------------------------------------------------------------------------

#[test]
fn upsert_inventory_overwrites() {
    with_worker(|w| {
        let h = w.handle();

        // Save initial inventory.
        h.save_inventory(
            5,
            &[InventoryRow {
                resource_kind: "Wood".into(),
                quantity: 5,
            }],
        )
        .unwrap();

        // Full replace with new data (no Wood).
        h.save_inventory(
            5,
            &[InventoryRow {
                resource_kind: "Berry".into(),
                quantity: 99,
            }],
        )
        .unwrap();

        let loaded = h.load_inventory(5).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].resource_kind, "Berry");
        assert_eq!(loaded[0].quantity, 99);
    });
}

#[test]
fn upsert_creature_bond_updates_in_place() {
    with_worker(|w| {
        let h = w.handle();

        h.save_creature_bond(7, "Fluffball", 1).unwrap();
        h.save_creature_bond(7, "Fluffball", 10).unwrap();

        let loaded = h.load_creature_bond(7, "Fluffball").unwrap().unwrap();
        assert_eq!(loaded.bond_level, 10);
    });
}

#[test]
fn upsert_plot_decoration_updates_in_place() {
    with_worker(|w| {
        let h = w.handle();

        h.save_plot_decoration(
            8,
            &PlotDecorationRow {
                decoration_id: 3,
                position_x: 0.0,
                position_z: 0.0,
                rotation_y: 0.0,
            },
        )
        .unwrap();

        // Move the decoration.
        h.save_plot_decoration(
            8,
            &PlotDecorationRow {
                decoration_id: 3,
                position_x: 15.0,
                position_z: 25.0,
                rotation_y: std::f64::consts::PI,
            },
        )
        .unwrap();

        let all = h.load_plot_decorations(8).unwrap();
        assert_eq!(all.len(), 1);
        assert!((all[0].position_x - 15.0).abs() < 1e-9);
        assert!((all[0].position_z - 25.0).abs() < 1e-9);
    });
}

// ---------------------------------------------------------------------------
// 5.  Constraint failure returns typed error
// ---------------------------------------------------------------------------

#[test]
fn duplicate_identity_returns_constraint_error() {
    with_worker(|w| {
        let h = w.handle();
        h.create_identity("dup_token", 1).unwrap();

        // Attempt to create the same token_hash with a different player_id.
        let err = h.create_identity("dup_token", 2).unwrap_err();

        assert!(
            matches!(&err, PersistenceError::Constraint(_)),
            "expected Constraint error, got: {err}"
        );
    });
}

// ---------------------------------------------------------------------------
// 6.  Bounded queue reports QueueFull (not blocking)
// ---------------------------------------------------------------------------

#[test]
fn bounded_queue_returns_full_error() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let db_path = dir.path().join("queue_test.db");
    let url = format!("sqlite://{}", db_path.display());

    // Capacity 1: the internal buffer holds exactly one pending command.
    let worker = PersistenceWorker::spawn(&url, 1).expect("spawn worker");
    worker.handle().migrate().expect("migrate");

    // Keep the worker sleeping for 500 ms so it won't drain the buffer between
    // our rapid sends. Two back-to-back sends from the same thread then saturate
    // the single buffer slot deterministically — no thread-spawning race needed.
    worker.handle()._test_stall(500).unwrap();

    let r1 = worker.handle()._test_stall(0); // fills or overflows the 1-slot buffer
    let r2 = worker.handle()._test_stall(0); // must hit QueueFull (capacity already saturated)

    assert!(
        matches!(r1, Err(PersistenceError::QueueFull { .. }))
            || matches!(r2, Err(PersistenceError::QueueFull { .. })),
        "expected QueueFull when buffer is at capacity, r1={r1:?}, r2={r2:?}"
    );
}

// ---------------------------------------------------------------------------
// 7.  Worker shutdown joins cleanly
// ---------------------------------------------------------------------------

#[test]
fn worker_shutdown_joins_cleanly() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let db_path = dir.path().join("shutdown.db");
    let url = format!("sqlite://{}", db_path.display());

    let mut worker = PersistenceWorker::spawn(&url, 16).expect("spawn worker");
    worker.handle().migrate().expect("migrate");
    worker.handle().resolve_identity("shutdown_test").unwrap();

    // Explicit shut-down.
    worker.shutdown().expect("clean shutdown");
}

#[test]
fn worker_drop_shuts_down_cleanly() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let db_path = dir.path().join("drop_shutdown.db");
    let url = format!("sqlite://{}", db_path.display());

    let worker = PersistenceWorker::spawn(&url, 16).expect("spawn worker");
    worker.handle().migrate().expect("migrate");
    // Drop is called when `worker` goes out of scope.
    drop(worker);
}

// ---------------------------------------------------------------------------
// 8.  Load non-existent rows returns None / empty
// ---------------------------------------------------------------------------

#[test]
fn load_missing_creature_bond_returns_none() {
    with_worker(|w| {
        let loaded = w.handle().load_creature_bond(999, "Nonexistent").unwrap();
        assert_eq!(loaded, None);
    });
}

#[test]
fn load_missing_inventory_returns_empty() {
    with_worker(|w| {
        let loaded = w.handle().load_inventory(999).unwrap();
        assert!(loaded.is_empty());
    });
}

#[test]
fn load_missing_decorations_returns_empty() {
    with_worker(|w| {
        let loaded = w.handle().load_plot_decorations(999).unwrap();
        assert!(loaded.is_empty());
    });
}
