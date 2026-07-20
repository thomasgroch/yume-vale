use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    Walk,
    Run,
    Interact,
    Collect,
    UseItem,
    Build,
    Remove,
    Emote,
    Sit,
    Stand,
}

impl ActionKind {
    pub fn all() -> &'static [ActionKind] {
        &[
            ActionKind::Walk,
            ActionKind::Run,
            ActionKind::Interact,
            ActionKind::Collect,
            ActionKind::UseItem,
            ActionKind::Build,
            ActionKind::Remove,
            ActionKind::Emote,
            ActionKind::Sit,
            ActionKind::Stand,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_kind_variants_exist() {
        assert_eq!(ActionKind::all().len(), 10);
    }

    #[test]
    fn action_kind_serde_roundtrip() {
        let actions = ActionKind::all();
        for action in actions {
            let json = serde_json::to_string(action).unwrap();
            let deserialized: ActionKind = serde_json::from_str(&json).unwrap();
            assert_eq!(*action, deserialized);
        }
    }

    #[test]
    fn action_kind_debug() {
        assert_eq!(format!("{:?}", ActionKind::Walk), "Walk");
    }
}
