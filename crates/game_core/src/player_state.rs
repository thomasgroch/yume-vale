use crate::actions::ActionKind;
pub use crate::id::PlayerId;
use crate::math::{Direction, Position, Velocity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub movement: Direction,
    pub run: bool,
    pub interact: Option<u64>,
    pub action: Option<ActionKind>,
}

impl Default for PlayerInput {
    fn default() -> Self {
        Self {
            movement: Direction::zero(),
            run: false,
            interact: None,
            action: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: PlayerId,
    pub position: Position,
    pub velocity: Velocity,
    pub current_action: Option<ActionKind>,
}

impl PlayerState {
    pub fn new(id: PlayerId, position: Position) -> Self {
        Self {
            id,
            position,
            velocity: Velocity::new(0.0, 0.0, 0.0),
            current_action: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_input_default() {
        let input = PlayerInput::default();
        assert!(input.movement.is_zero());
        assert!(!input.run);
        assert!(input.interact.is_none());
        assert!(input.action.is_none());
    }

    #[test]
    fn player_input_with_movement() {
        let dir = Direction::from_xz(1.0, 0.0).unwrap();
        let input = PlayerInput {
            movement: dir,
            run: true,
            interact: Some(5),
            action: Some(ActionKind::Collect),
        };
        assert!(!input.movement.is_zero());
        assert!(input.run);
        assert_eq!(input.interact, Some(5));
        assert_eq!(input.action, Some(ActionKind::Collect));
    }

    #[test]
    fn player_state_new() {
        let id = PlayerId::new(1);
        let pos = Position::new(10.0, 0.0, 20.0);
        let state = PlayerState::new(id, pos);
        assert_eq!(state.id, id);
        assert_eq!(state.position, pos);
        assert_eq!(state.velocity, Velocity::new(0.0, 0.0, 0.0));
        assert!(state.current_action.is_none());
    }

    #[test]
    fn player_state_serde_roundtrip() {
        let id = PlayerId::new(42);
        let pos = Position::new(100.0, 0.0, 200.0);
        let state = PlayerState::new(id, pos);
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: PlayerState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }
}
