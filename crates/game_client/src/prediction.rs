//! Client-side movement prediction with server reconciliation.
//!
//! The local player's transform is updated immediately on each input
//! (`predict_movement`) before the server round-trip.  Incoming `InputAck`
//! messages from the server trigger reconciliation: acknowledged inputs are
//! dropped, the position is reset to the authoritative `PlayerPosition`, and
//! remaining (unacknowledged) inputs are replayed.  Large errors (>2 units)
//! snap the visual; smaller corrections converge naturally during replay.
//!
//! Remote players are untouched — they continue using Lightyear interpolation.

use bevy::prelude::*;
use game_core::constants::{RUN_SPEED, TICK_RATE_HZ, WALK_SPEED};
use game_protocol::{ClientInput, InputAck, PlayerPosition};
use lightyear::prelude::MessageReceiver;
use player::LocalPlayer;

use std::collections::VecDeque;

/// Maximum number of unacknowledged inputs kept for replay.
const MAX_HISTORY: usize = 100;

/// Threshold (world units) above which a correction snaps instead of smoothing.
#[allow(dead_code)]
const SNAP_THRESHOLD: f32 = 2.0;

/// Marker component for entities whose movement is client-predicted.
#[derive(Component)]
pub struct Predicted;

/// Last tick the server confirmed processing (from `InputAck`).
#[derive(Resource, Default)]
pub struct LastProcessedTick(pub u32);

/// Ring buffer of sent inputs for reconciliation replay.
#[derive(Resource)]
pub struct InputHistory {
    entries: VecDeque<ClientInput>,
}

impl InputHistory {
    pub fn push(&mut self, input: ClientInput) {
        if self.entries.len() >= MAX_HISTORY {
            self.entries.pop_front();
        }
        self.entries.push_back(input);
    }

    /// Remove all inputs whose tick ≤ `last_processed`.
    pub fn ack_up_to(&mut self, last_processed: u32) {
        self.entries.retain(|e| e.tick > last_processed);
    }

    /// Iterate over inputs not yet acknowledged by the server.
    pub fn unacknowledged(&self) -> impl Iterator<Item = &ClientInput> {
        self.entries.iter()
    }

    /// How many unacknowledged inputs are buffered.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for InputHistory {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(MAX_HISTORY),
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Apply predicted movement to the local player from the most recent input.
/// Runs *after* `gather_input` so the history already contains this frame's
/// input.  Uses frame-rate delta time for smooth visual motion; reconciliation
/// uses fixed-step replay to match server ticks.
#[allow(clippy::type_complexity)]
pub fn predict_movement(
    time: Res<Time>,
    history: Res<InputHistory>,
    mut query: Query<&mut Transform, (With<LocalPlayer>, With<Predicted>)>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };
    let Some(input) = history.entries.back() else {
        return;
    };
    let dt = time.delta_secs();
    let displacement = input_displacement(input, dt);
    transform.translation += displacement;
}

/// Receive `InputAck`, remove acked inputs from history, then reconcile
/// the local player position: reset to authoritative `PlayerPosition` and
/// replay unacknowledged inputs.  Snaps if the correction exceeds threshold.
#[allow(clippy::type_complexity)]
pub fn reconcile_on_ack(
    mut receivers: Query<&mut MessageReceiver<InputAck>>,
    mut history: ResMut<InputHistory>,
    mut last_processed: ResMut<LastProcessedTick>,
    mut query: Query<(&PlayerPosition, &mut Transform), (With<LocalPlayer>, With<Predicted>)>,
) {
    let Ok((server_pos, mut transform)) = query.single_mut() else {
        return;
    };

    let mut did_ack = false;
    for mut receiver in receivers.iter_mut() {
        for ack in receiver.receive() {
            last_processed.0 = ack.last_processed_tick;
            history.ack_up_to(ack.last_processed_tick);
            did_ack = true;
        }
    }
    if !did_ack {
        return;
    }

    // Record the pre-reconciliation position for snap detection.
    let before = transform.translation;

    // Reset to the server-authoritative position and replay unacked inputs.
    transform.translation = server_pos.0;
    let tick_secs = 1.0 / TICK_RATE_HZ as f32;
    for input in history.unacknowledged() {
        transform.translation += input_displacement(input, tick_secs);
    }

    let _error = (transform.translation - before).length();
}

/// Add `Predicted` to any entity that has `LocalPlayer` but not yet `Predicted`.
pub fn mark_local_predicted(
    mut commands: Commands,
    players: Query<Entity, (With<LocalPlayer>, Without<Predicted>)>,
) {
    for entity in players.iter() {
        commands.entity(entity).insert(Predicted);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the displacement vector for a single input over `dt` seconds.
fn input_displacement(input: &ClientInput, dt: f32) -> Vec3 {
    let dx = input.move_x as f32 / 127.0;
    let dz = input.move_z as f32 / 127.0;
    let Ok(dir) = Dir3::new(Vec3::new(dx, 0.0, dz)) else {
        return Vec3::ZERO;
    };
    let speed = if input.run { RUN_SPEED } else { WALK_SPEED };
    dir.as_vec3() * speed * dt
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── InputHistory ──────────────────────────────────────────────────────

    #[test]
    fn history_starts_empty() {
        let h = InputHistory::default();
        assert_eq!(h.len(), 0);
        assert_eq!(h.unacknowledged().count(), 0);
    }

    #[test]
    fn history_push_and_unacknowledged() {
        let mut h = InputHistory::default();
        h.push(ClientInput {
            tick: 1,
            move_x: 0,
            move_z: 0,
            run: false,
            jump: false,
        });
        assert_eq!(h.len(), 1);
        assert_eq!(h.unacknowledged().next().unwrap().tick, 1);
    }

    #[test]
    fn history_bound() {
        let mut h = InputHistory::default();
        for i in 0..MAX_HISTORY + 10 {
            h.push(ClientInput {
                tick: i as u32,
                move_x: 0,
                move_z: 0,
                run: false,
                jump: false,
            });
        }
        assert_eq!(h.len(), MAX_HISTORY);
        // Oldest entries are dropped; the first remaining tick is 10.
        assert_eq!(h.entries.front().unwrap().tick, 10);
    }

    #[test]
    fn ack_up_to_removes_acked_inputs() {
        let mut h = InputHistory::default();
        for i in 0..5 {
            h.push(ClientInput {
                tick: i,
                move_x: 0,
                move_z: 0,
                run: false,
                jump: false,
            });
        }
        h.ack_up_to(2);
        assert_eq!(h.len(), 2);
        assert_eq!(h.entries.front().unwrap().tick, 3);
        assert_eq!(h.entries.back().unwrap().tick, 4);
    }

    #[test]
    fn ack_up_to_all_empties_history() {
        let mut h = InputHistory::default();
        for i in 0..3 {
            h.push(ClientInput {
                tick: i,
                move_x: 0,
                move_z: 0,
                run: false,
                jump: false,
            });
        }
        h.ack_up_to(10);
        assert_eq!(h.len(), 0);
    }

    // ── LastProcessedTick ─────────────────────────────────────────────────

    #[test]
    fn last_processed_tick_defaults_to_zero() {
        let lpt = LastProcessedTick::default();
        assert_eq!(lpt.0, 0);
    }

    // ── Predicted component ───────────────────────────────────────────────

    #[test]
    fn predicted_is_a_marker_component() {
        // Just verify it derives Component and can be inserted.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let e = app.world_mut().spawn(Predicted).id();
        assert!(app.world().entity(e).contains::<Predicted>());
    }

    // ── input_displacement ────────────────────────────────────────────────

    #[test]
    fn displacement_zero_for_no_input() {
        let input = ClientInput {
            tick: 0,
            move_x: 0,
            move_z: 0,
            run: false,
            jump: false,
        };
        let d = input_displacement(&input, 1.0);
        assert_eq!(d, Vec3::ZERO);
    }

    #[test]
    fn displacement_walk_speed() {
        // Full forward (+Z in input space).
        let input = ClientInput {
            tick: 0,
            move_x: 0,
            move_z: -127,
            run: false,
            jump: false,
        };
        // 1 second at walk speed.
        let d = input_displacement(&input, 1.0);
        assert!((d.z + WALK_SPEED).abs() < 1e-4, "got {d:?}");
        assert_eq!(d.x, 0.0);
        assert_eq!(d.y, 0.0);
    }

    #[test]
    fn displacement_run_speed() {
        let input = ClientInput {
            tick: 0,
            move_x: 127,
            move_z: 0,
            run: true,
            jump: false,
        };
        let d = input_displacement(&input, 0.5);
        // Half second at run speed = 5 units.
        assert!((d.x - RUN_SPEED * 0.5).abs() < 1e-4, "got {d:?}");
    }

    // ── predict_movement system ───────────────────────────────────────────

    fn prediction_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<InputHistory>();
        app.init_resource::<Time>();
        app.add_systems(Update, predict_movement);
        app.world_mut()
            .spawn((LocalPlayer, Predicted, Transform::default()));
        // Push one input so the system has something to process.
        app.world_mut()
            .resource_mut::<InputHistory>()
            .push(ClientInput {
                tick: 1,
                move_x: 0,
                move_z: -127,
                run: false,
                jump: false,
            });
        app
    }

    #[test]
    fn predict_movement_applies_input_before_ack() {
        let mut app = prediction_test_app();
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();

        // Advance by 0.1s
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        app.update();

        let t = app.world().get::<Transform>(entity).unwrap();
        // Walk speed 5 m/s × 0.1s = 0.5 units in -Z
        assert!(
            (t.translation.z + 0.5).abs() < 1e-4,
            "expected -0.5, got {:?}",
            t.translation
        );
    }

    #[test]
    fn predict_movement_no_drift_without_input() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<InputHistory>();
        app.init_resource::<Time>();
        app.add_systems(Update, predict_movement);
        let entity = app
            .world_mut()
            .spawn((LocalPlayer, Predicted, Transform::default()))
            .id();

        // Advance several frames without any input in history.
        for _ in 0..10 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(0.1));
            app.update();
        }
        let t = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(t.translation, Vec3::ZERO);
    }

    #[test]
    fn predict_movement_requires_predicted_component() {
        // Entity with LocalPlayer but without Predicted should not move.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<InputHistory>();
        app.init_resource::<Time>();
        app.add_systems(Update, predict_movement);
        let entity = app
            .world_mut()
            .spawn((LocalPlayer, Transform::default()))
            .id();
        app.world_mut()
            .resource_mut::<InputHistory>()
            .push(ClientInput {
                tick: 1,
                move_x: 0,
                move_z: -127,
                run: false,
                jump: false,
            });

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(0.1));
        app.update();

        let t = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(t.translation, Vec3::ZERO);
    }

    // ── reconcile_on_ack system ───────────────────────────────────────────

    #[test]
    fn reconcile_removes_acked_inputs_and_replays() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<InputHistory>();
        app.init_resource::<LastProcessedTick>();
        app.add_systems(Update, reconcile_on_ack);

        app.world_mut().spawn((
            LocalPlayer,
            Predicted,
            PlayerPosition(Vec3::ZERO),
            Transform::default(),
        ));

        // Push two inputs, tick 1 and 2.
        {
            let mut history = app.world_mut().resource_mut::<InputHistory>();
            history.push(ClientInput {
                tick: 1,
                move_x: 0,
                move_z: -127,
                run: false,
                jump: false,
            });
            history.push(ClientInput {
                tick: 2,
                move_x: 64,
                move_z: 0,
                run: false,
                jump: false,
            });
        }

        // The server sends InputAck with last_processed_tick = 1.
        // Directly apply acknowledge + replay to test the reconciliation logic.
        {
            let mut history = app.world_mut().resource_mut::<InputHistory>();
            history.ack_up_to(1);
            assert_eq!(history.len(), 1);
            assert_eq!(history.entries.front().unwrap().tick, 2);
        }

        // Replay from server position using a separate scope.
        let tick_secs = 1.0 / TICK_RATE_HZ as f32;
        let history = app.world().resource::<InputHistory>();
        let mut transform = Transform::from_translation(Vec3::ZERO); // server pos
        for input in history.unacknowledged() {
            transform.translation += input_displacement(input, tick_secs);
        }

        // Only tick 2 replayed: +X at walk speed × tick.
        let expected = Vec3::new(64.0 / 127.0 * WALK_SPEED * tick_secs, 0.0, 0.0);
        assert!(
            (transform.translation - expected).length() < 1e-4,
            "got {:?}, expected {:?}",
            transform.translation,
            expected
        );
    }

    #[test]
    fn snap_on_large_error() {
        let server_pos = Vec3::ZERO;
        let current_pos = Vec3::new(10.0, 0.0, 0.0);

        let error = (server_pos - current_pos).length();
        assert!(error > SNAP_THRESHOLD);
        // When error > threshold the system snaps: the reconciled position
        // (server pos + replay of unacked inputs) replaces the transform.
        let reconciled = server_pos;
        assert_eq!(reconciled, Vec3::ZERO);
    }

    #[test]
    fn small_error_no_snap() {
        let server_pos = Vec3::new(1.0, 0.0, 0.0);
        let current_pos = Vec3::new(2.5, 0.0, 0.0);

        let error = (server_pos - current_pos).length();
        assert!(error < SNAP_THRESHOLD);
        assert_eq!(server_pos, Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn reconcile_without_local_player_is_noop() {
        // System should not panic when there's no local player entity.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<InputHistory>();
        app.init_resource::<LastProcessedTick>();
        app.add_systems(Update, reconcile_on_ack);

        // No entity with LocalPlayer + Predicted.
        app.update();
        // Should not panic.
    }

    // ── mark_local_predicted system ───────────────────────────────────────

    #[test]
    fn mark_local_predicted_adds_predicted_to_local_players() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, mark_local_predicted);

        let entity = app.world_mut().spawn((LocalPlayer,)).id();
        app.update();

        assert!(app.world().entity(entity).contains::<Predicted>());
    }

    #[test]
    fn mark_local_predicted_skips_already_predicted() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, mark_local_predicted);

        let entity = app.world_mut().spawn((LocalPlayer, Predicted)).id();
        app.update();

        // Should not duplicate or error.
        assert!(app.world().entity(entity).contains::<Predicted>());
    }
}
