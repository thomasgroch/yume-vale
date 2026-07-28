use bevy::prelude::*;
use game_core::math::Direction;
use game_core::player_state::PlayerInput;
use game_protocol::{ClientInput, InputAck, ReplicatedPlayerInput};
use lightyear::prelude::{MessageReceiver, MessageSender};
use player::PlayerMovement;

use super::connection::ClientPlayer;

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

/// Reads `ClientInput` messages from connected clients, applies them
/// to the corresponding player's movement and replicated input, and sends
/// an `InputAck` back to the client with the last processed tick.
pub fn apply_client_input(
    mut receivers: Query<(Entity, &mut MessageReceiver<ClientInput>, &ClientPlayer)>,
    mut players: Query<(&mut PlayerMovement, &mut ReplicatedPlayerInput)>,
    mut ack_senders: Query<&mut MessageSender<InputAck>>,
) {
    for (link_entity, mut receiver, info) in receivers.iter_mut() {
        for ci in receiver.receive() {
            if let Ok((mut movement, mut rep_input)) = players.get_mut(info.player_entity) {
                apply_input_to_player(&ci, &mut movement, &mut rep_input);
                // Send InputAck for the processed tick
                if let Ok(mut sender) = ack_senders.get_mut(link_entity) {
                    sender.send::<game_protocol::channels::ReliableChannel>(InputAck {
                        last_processed_tick: ci.tick,
                    });
                }
            }
        }
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
    fn apply_input_to_player_chat_message_ignored() {
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
}
