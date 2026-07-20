use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    Wood,
    Stone,
    Berry,
    Crystal,
    Flower,
    Fiber,
    Mushroom,
    Sap,
}

impl ResourceKind {
    pub fn all() -> &'static [ResourceKind] {
        &[
            ResourceKind::Wood,
            ResourceKind::Stone,
            ResourceKind::Berry,
            ResourceKind::Crystal,
            ResourceKind::Flower,
            ResourceKind::Fiber,
            ResourceKind::Mushroom,
            ResourceKind::Sap,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_kind_variants_exist() {
        assert_eq!(ResourceKind::all().len(), 8);
    }

    #[test]
    fn resource_kind_serde_roundtrip() {
        let kinds = ResourceKind::all();
        for kind in kinds {
            let json = serde_json::to_string(kind).unwrap();
            let deserialized: ResourceKind = serde_json::from_str(&json).unwrap();
            assert_eq!(*kind, deserialized);
        }
    }

    #[test]
    fn resource_kind_clone_copy() {
        let a = ResourceKind::Wood;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn resource_kind_debug() {
        assert_eq!(format!("{:?}", ResourceKind::Wood), "Wood");
    }
}
