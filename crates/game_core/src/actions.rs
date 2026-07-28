use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    Walk,
    Run,
    Interact,
    Collect,
    Feed,
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
            ActionKind::Feed,
            ActionKind::UseItem,
            ActionKind::Build,
            ActionKind::Remove,
            ActionKind::Emote,
            ActionKind::Sit,
            ActionKind::Stand,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmoteKind {
    Wave,
    Dance,
    Sit,
    Point,
    Laugh,
    Cry,
    Joy,
    Bow,
    Cheer,
    Sleep,
}

impl EmoteKind {
    pub fn all() -> &'static [EmoteKind] {
        &[
            EmoteKind::Wave,
            EmoteKind::Dance,
            EmoteKind::Sit,
            EmoteKind::Point,
            EmoteKind::Laugh,
            EmoteKind::Cry,
            EmoteKind::Joy,
            EmoteKind::Bow,
            EmoteKind::Cheer,
            EmoteKind::Sleep,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_kind_variants_exist() {
        assert_eq!(ActionKind::all().len(), 11);
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

    #[test]
    fn emote_kind_variants_exist() {
        assert_eq!(EmoteKind::all().len(), 10);
    }

    #[test]
    fn emote_kind_serde_roundtrip() {
        for emote in EmoteKind::all() {
            let json = serde_json::to_string(emote).unwrap();
            let deserialized: EmoteKind = serde_json::from_str(&json).unwrap();
            assert_eq!(*emote, deserialized);
        }
    }

    #[test]
    fn emote_kind_clone_copy() {
        let a = EmoteKind::Wave;
        let b = a;
        assert_eq!(a, b);
    }
}
