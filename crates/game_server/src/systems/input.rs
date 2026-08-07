use bevy::prelude::*;
use game_core::math::Direction;
use game_core::player_state::PlayerInput;
use game_protocol::{ClientInput, ReplicatedPlayerInput};
use lightyear::prelude::MessageReceiver;
use player::PlayerMovement;
use std::time::Duration;

use super::connection::ClientPlayer;

const INPUT_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Component, Default)]
pub struct InputFreshness {
    elapsed: Duration,
}

pub fn advance_input_silence(
    freshness: &mut InputFreshness,
    delta: Duration,
    movement: &mut PlayerMovement,
    rep_input: &mut ReplicatedPlayerInput,
) {
    freshness.elapsed += delta;
    if freshness.elapsed < INPUT_TIMEOUT {
        return;
    }
    movement.direction = Direction::zero();
    movement.running = false;
    movement.jump = false;
    rep_input.0 = PlayerInput::default();
}

/// Apply a single `ClientInput` to a player's movement and replicated input
/// components. Extracted for testability.
pub fn apply_input_to_player(
    input: &ClientInput,
    movement: &mut PlayerMovement,
    rep_input: &mut ReplicatedPlayerInput,
) {
    let dx = input.move_x as f32 / 127.0;
    let dz = input.move_z as f32 / 127.0;
    let direction = Direction::from_xz(dx, dz).unwrap_or(Direction::zero());
    movement.direction = direction;
    movement.running = input.run;
    movement.jump = input.jump;
    rep_input.0 = PlayerInput {
        movement: direction,
        run: input.run,
        interact: None,
        action: None,
    };
}

/// Reads `ClientInput` messages from connected clients and applies them
/// to the corresponding player's movement and replicated input.
pub fn apply_client_input(
    mut receivers: Query<(&mut MessageReceiver<ClientInput>, &ClientPlayer)>,
    mut players: Query<(
        &mut PlayerMovement,
        &mut ReplicatedPlayerInput,
        &mut InputFreshness,
    )>,
) {
    for (mut receiver, info) in receivers.iter_mut() {
        for ci in receiver.receive() {
            if let Ok((mut movement, mut rep_input, mut freshness)) =
                players.get_mut(info.player_entity)
            {
                apply_input_to_player(&ci, &mut movement, &mut rep_input);
                freshness.elapsed = Duration::ZERO;
            }
        }
    }
}

pub fn stop_stale_player_input(
    time: Res<Time>,
    mut players: Query<(
        &mut InputFreshness,
        &mut PlayerMovement,
        &mut ReplicatedPlayerInput,
    )>,
) {
    for (mut freshness, mut movement, mut rep_input) in &mut players {
        advance_input_silence(&mut freshness, time.delta(), &mut movement, &mut rep_input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::math::Direction;

    #[test]
    fn apply_input_to_player_writes_movement() {
        let input = ClientInput {
            tick: 1,
            move_x: 1,
            move_z: 0,
            run: true,
            jump: false,
        };
        let mut movement = PlayerMovement::default();
        let mut rep_input = ReplicatedPlayerInput(PlayerInput::default());

        apply_input_to_player(&input, &mut movement, &mut rep_input);

        assert_eq!(movement.direction, Direction::from_xz(1.0, 0.0).unwrap());
        assert!(movement.running);
        assert_eq!(rep_input.0.movement, Direction::from_xz(1.0, 0.0).unwrap());
        assert!(rep_input.0.run);
        assert!(rep_input.0.interact.is_none());
        assert!(rep_input.0.action.is_none());
    }

    #[test]
    fn apply_input_to_player_decodes_diagonal() {
        let diagonal_encoded = (std::f32::consts::FRAC_1_SQRT_2 * 127.0).round() as i8;
        let input = ClientInput {
            tick: 1,
            move_x: diagonal_encoded,
            move_z: diagonal_encoded,
            run: false,
            jump: false,
        };
        let mut movement = PlayerMovement::default();
        let mut rep_input = ReplicatedPlayerInput(PlayerInput::default());

        apply_input_to_player(&input, &mut movement, &mut rep_input);

        let dir = movement.direction.0;
        assert!((dir.x - dir.z).abs() < 1e-5, "diagonal must be symmetric");
        assert!(
            (dir.length() - 1.0).abs() < 1e-2,
            "direction must be normalized"
        );
    }

    #[test]
    fn apply_input_to_player_writes_action_and_interact() {
        let input = ClientInput {
            tick: 2,
            move_x: 0,
            move_z: 0,
            run: false,
            jump: false,
        };
        let mut movement = PlayerMovement::default();
        let mut rep_input = ReplicatedPlayerInput(PlayerInput::default());

        apply_input_to_player(&input, &mut movement, &mut rep_input);

        assert_eq!(movement.direction, Direction::zero());
        assert!(!movement.running);
        assert_eq!(rep_input.0.action, None);
        assert_eq!(rep_input.0.interact, None);
    }

    #[test]
    fn apply_input_without_movement_has_no_action() {
        let input = ClientInput {
            tick: 3,
            move_x: 0,
            move_z: 0,
            run: false,
            jump: false,
        };
        let mut movement = PlayerMovement::default();
        let mut rep_input = ReplicatedPlayerInput(PlayerInput::default());

        apply_input_to_player(&input, &mut movement, &mut rep_input);

        assert_eq!(rep_input.0.action, None);
    }

    #[test]
    fn fresh_input_is_preserved_before_one_second() {
        let mut freshness = InputFreshness::default();
        let mut movement = PlayerMovement {
            direction: Direction::from_xz(1.0, 0.0).unwrap(),
            running: true,
            jump: true,
        };
        let mut replicated = ReplicatedPlayerInput(PlayerInput {
            movement: movement.direction,
            run: true,
            interact: None,
            action: None,
        });

        advance_input_silence(
            &mut freshness,
            Duration::from_millis(999),
            &mut movement,
            &mut replicated,
        );

        assert!(!movement.direction.is_zero());
        assert!(movement.running);
        assert!(movement.jump);
    }

    #[test]
    fn stale_input_is_zeroed_after_one_second() {
        let mut freshness = InputFreshness::default();
        let mut movement = PlayerMovement {
            direction: Direction::from_xz(1.0, 0.0).unwrap(),
            running: true,
            jump: true,
        };
        let mut replicated = ReplicatedPlayerInput(PlayerInput {
            movement: movement.direction,
            run: true,
            interact: None,
            action: None,
        });

        advance_input_silence(
            &mut freshness,
            Duration::from_secs(1),
            &mut movement,
            &mut replicated,
        );

        assert!(movement.direction.is_zero());
        assert!(!movement.running);
        assert!(!movement.jump);
        assert_eq!(replicated.0, PlayerInput::default());
    }
}
