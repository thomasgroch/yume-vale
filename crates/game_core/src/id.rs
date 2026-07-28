use serde::{Deserialize, Serialize};

macro_rules! define_id {
    ($vis:vis struct $name:ident(pub u64);) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        $vis struct $name(pub u64);

        impl $name {
            pub const INVALID: Self = Self(u64::MAX);

            pub fn new(value: u64) -> Self {
                Self(value)
            }

            pub fn get(&self) -> u64 {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::INVALID
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl From<$name> for u64 {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_id!(
    pub struct PlayerId(pub u64);
);
define_id!(
    pub struct EntityId(pub u64);
);
define_id!(
    pub struct ResourceId(pub u64);
);
define_id!(
    pub struct CreatureId(pub u64);
);
define_id!(
    pub struct ItemId(pub u64);
);
define_id!(
    pub struct QuestId(pub u64);
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct_types() {
        let pid = PlayerId::new(1);
        let eid = EntityId::new(1);
        assert_eq!(pid.get(), eid.get());
    }

    #[test]
    fn id_display_format() {
        let pid = PlayerId::new(42);
        assert_eq!(pid.to_string(), "PlayerId(42)");
    }

    #[test]
    fn id_debug_format() {
        let pid = PlayerId::new(7);
        assert_eq!(format!("{pid:?}"), "PlayerId(7)");
    }

    #[test]
    fn id_clone_copy() {
        let a = PlayerId::new(10);
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn id_equality_and_hash() {
        use std::collections::HashSet;
        let a = PlayerId::new(5);
        let b = PlayerId::new(5);
        let c = PlayerId::new(6);
        assert_eq!(a, b);
        assert_ne!(a, c);

        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }

    #[test]
    fn id_default_is_invalid() {
        let id = PlayerId::default();
        assert_eq!(id, PlayerId::INVALID);
    }

    #[test]
    fn id_from_u64_roundtrip() {
        let id: PlayerId = 42u64.into();
        let val: u64 = id.into();
        assert_eq!(val, 42);
    }

    #[test]
    fn id_serde_roundtrip_json() {
        let id = PlayerId::new(99);
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: PlayerId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }
}
