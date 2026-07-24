use game_core::id::PlayerId;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClientInput {
    pub tick: u32,
    pub move_x: i8,
    pub move_z: i8,
    pub run: bool,
    pub jump: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Welcome {
    pub player_id: PlayerId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_input_serde_roundtrip() {
        let orig = ClientInput {
            tick: 42,
            move_x: 1,
            move_z: 0,
            run: true,
            jump: false,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ClientInput = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn welcome_serde_roundtrip() {
        let orig = Welcome {
            player_id: PlayerId::new(1),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: Welcome = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }
}
