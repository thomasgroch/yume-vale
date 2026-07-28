use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::housing_layout;
use game_protocol::messages::{PlotBuildIntent, PlotRemoveIntent};
use game_protocol::{DecorationState, PlotEntry, PlotSnapshot};
use lightyear::connection::network_target::NetworkTarget;
use lightyear::prelude::*;

use crate::components::{HousingPlayer, PlotDecorationMarker};
use crate::plugin::PlotStateResource;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Handles `PlotBuildIntent` from clients: validates ownership, cooldown,
/// one-decoration limit, spawns collider + replicated decoration entity.
#[allow(clippy::too_many_arguments)]
pub fn handle_plot_build_intent(
    mut receivers: Query<(&mut MessageReceiver<PlotBuildIntent>, &HousingPlayer)>,
    mut plot_state: ResMut<PlotStateResource>,
    mut commands: Commands,
    mut snapshot_senders: Query<&mut MessageSender<PlotSnapshot>>,
    time: Res<Time>,
) {
    for (mut receiver, hp) in receivers.iter_mut() {
        for _intent in receiver.receive() {
            let player_id = hp.player_id.get();
            let slot = housing_layout::slot_for_player(player_id);

            // Cooldown
            let now = time.elapsed_secs_f64();
            if now - plot_state.last_build_time[slot] < 0.5 {
                continue;
            }
            plot_state.last_build_time[slot] = now;

            // One-decoration limit per plot
            if plot_state.decorations[slot].is_some() {
                continue;
            }

            // Mark occupied
            plot_state.decorations[slot] = Some(player_id);

            // Spawn decoration entity with collider + replication
            let center = housing_layout::slot_center(slot);
            let entity = spawn_plot_decoration(&mut commands, player_id, slot, center);
            plot_state.decoration_entities[slot] = Some(entity);

            // Broadcast snapshot
            broadcast_plot_snapshot(&plot_state, &mut snapshot_senders);
        }
    }
}

/// Handles `PlotRemoveIntent` from clients: validates ownership, cooldown,
/// despawns decoration entity.
#[allow(clippy::too_many_arguments)]
pub fn handle_plot_remove_intent(
    mut receivers: Query<(&mut MessageReceiver<PlotRemoveIntent>, &HousingPlayer)>,
    mut plot_state: ResMut<PlotStateResource>,
    mut commands: Commands,
    mut snapshot_senders: Query<&mut MessageSender<PlotSnapshot>>,
    time: Res<Time>,
) {
    for (mut receiver, hp) in receivers.iter_mut() {
        for _intent in receiver.receive() {
            let player_id = hp.player_id.get();
            let slot = housing_layout::slot_for_player(player_id);

            // Cooldown
            let now = time.elapsed_secs_f64();
            if now - plot_state.last_build_time[slot] < 0.5 {
                continue;
            }
            plot_state.last_build_time[slot] = now;

            // Must have a decoration owned by this player
            if plot_state.decorations[slot] != Some(player_id) {
                continue;
            }

            plot_state.decorations[slot] = None;

            // Despawn decoration entity
            if let Some(entity) = plot_state.decoration_entities[slot].take() {
                commands.entity(entity).despawn();
            }

            // Broadcast snapshot
            broadcast_plot_snapshot(&plot_state, &mut snapshot_senders);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Spawn a decoration entity with a physics collider and replicated state.
fn spawn_plot_decoration(
    commands: &mut Commands,
    _player_id: u64,
    _slot_index: usize,
    center: Vec3,
) -> Entity {
    let decoration_state = DecorationState {
        kind: game_core::decorations::DecorationKind::Rock(0.4),
        position_x: center.x,
        position_y: 0.44,
        position_z: center.z,
        rotation: 0.0,
    };

    commands
        .spawn((
            decoration_state,
            RigidBody::Static,
            Collider::cylinder(0.4, 0.88),
            Transform::from_translation(Vec3::new(center.x, 0.44, center.z)),
            Replicate::to_clients(NetworkTarget::All),
            PlotDecorationMarker {
                player_id: _player_id,
                slot_index: _slot_index,
            },
        ))
        .id()
}

/// Send a `PlotSnapshot` to all connected clients.
fn broadcast_plot_snapshot(
    plot_state: &PlotStateResource,
    senders: &mut Query<&mut MessageSender<PlotSnapshot>>,
) {
    let plots: Vec<PlotEntry> = plot_state
        .decorations
        .iter()
        .enumerate()
        .filter_map(|(slot, &opt)| {
            opt.map(|pid| {
                let center = housing_layout::slot_center(slot);
                PlotEntry {
                    plot_id: pid,
                    position_x: center.x,
                    position_z: center.z,
                    building_kind: 1,
                }
            })
        })
        .collect();
    let snapshot = PlotSnapshot { plots };

    for mut sender in senders.iter_mut() {
        sender.send::<game_protocol::channels::ReliableChannel>(snapshot.clone());
    }
}

#[cfg(test)]
mod tests {
    use game_core::housing_layout;

    #[test]
    fn slot_for_player_deterministic_and_stable() {
        let slot_a = housing_layout::slot_for_player(42u64);
        let slot_b = housing_layout::slot_for_player(42u64);
        assert_eq!(slot_a, slot_b);
        assert!(slot_a < housing_layout::HOUSING_PLOT_COUNT);
    }

    #[test]
    fn different_players_spread_across_slots() {
        let slots: Vec<usize> = (0..100u64).map(housing_layout::slot_for_player).collect();
        let unique = slots
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        assert!(unique.len() > 1, "expected multiple different slots");
    }

    #[test]
    fn slot_center_returns_valid_position() {
        let center = housing_layout::slot_center(0);
        assert!(center.x < 0.0, "plots should be on negative-x side");
        assert_eq!(center.y, 0.0);
    }

    #[test]
    fn point_in_plot_works_at_center() {
        let layout = housing_layout::plot_layout();
        for slot in &layout {
            assert!(housing_layout::point_in_plot(
                slot.index,
                slot.center.x,
                slot.center.z
            ));
        }
    }

    #[test]
    fn point_in_plot_rejects_far_point() {
        assert!(!housing_layout::point_in_plot(0, 999.0, 999.0));
    }
}
