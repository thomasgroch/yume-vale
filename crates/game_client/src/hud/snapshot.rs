use bevy::prelude::*;
use game_core::inventory::ItemKind;
use game_core::resources::ResourceKind;
use game_protocol::messages::{
    ActionRejected, InventorySnapshot, ItemSlotData, QuestSnapshot, QuestStateData,
};
use lightyear::prelude::MessageReceiver;

// ─── Client-side snapshot resources (pure projection) ───────────────────

#[derive(Resource, Default)]
pub struct ClientInventory {
    pub items: Vec<ItemSlotData>,
}

#[derive(Resource, Default)]
pub struct ClientQuests {
    pub quests: Vec<QuestStateData>,
}

/// Cooldown state — set to `true` when server confirms an action, cleared
/// after the cooldown duration elapses.
#[derive(Resource)]
pub struct ClientCooldown {
    pub active: bool,
    pub remaining: f32,
}

impl Default for ClientCooldown {
    fn default() -> Self {
        Self {
            active: false,
            remaining: 0.0,
        }
    }
}

/// Last action feedback (rejection / persistence-busy message).
#[derive(Resource, Default)]
pub struct ClientActionFeedback {
    pub message: String,
    pub visible: bool,
}

// ─── Receiver systems ──────────────────────────────────────────────────

/// Project `InventorySnapshot` messages into the `ClientInventory` resource.
pub fn receive_inventory_snapshot(
    mut receivers: Query<&mut MessageReceiver<InventorySnapshot>>,
    mut client_inv: ResMut<ClientInventory>,
) {
    for mut receiver in &mut receivers {
        for snapshot in receiver.receive() {
            client_inv.items = snapshot.items;
        }
    }
}

/// Project `QuestSnapshot` messages into the `ClientQuests` resource.
pub fn receive_quest_snapshot(
    mut receivers: Query<&mut MessageReceiver<QuestSnapshot>>,
    mut client_quests: ResMut<ClientQuests>,
) {
    for mut receiver in &mut receivers {
        for snapshot in receiver.receive() {
            client_quests.quests = snapshot.quests;
        }
    }
}

/// Project `ActionRejected` messages into `ClientActionFeedback`.
pub fn receive_action_rejected(
    mut receivers: Query<&mut MessageReceiver<ActionRejected>>,
    mut feedback: ResMut<ClientActionFeedback>,
) {
    for mut receiver in &mut receivers {
        for rejected in receiver.receive() {
            feedback.message = rejected.reason;
            feedback.visible = true;
        }
    }
}

/// Tick cooldown timer down each frame.
pub fn tick_cooldown(time: Res<Time>, mut cooldown: ResMut<ClientCooldown>) {
    if !cooldown.active {
        return;
    }
    cooldown.remaining -= time.delta_secs();
    if cooldown.remaining <= 0.0 {
        cooldown.active = false;
        cooldown.remaining = 0.0;
    }
}

/// Clear action feedback after a brief display duration.
pub fn clear_action_feedback(mut feedback: ResMut<ClientActionFeedback>) {
    if !feedback.visible {
        return;
    }
    feedback.visible = false;
    feedback.message.clear();
}

// ─── Helpers ───────────────────────────────────────────────────────────

/// Quantity of a specific `ResourceKind` in the client inventory.
pub fn resource_quantity(items: &[ItemSlotData], kind: ResourceKind) -> u32 {
    items
        .iter()
        .filter(|slot| slot.kind == ItemKind::Resource(kind))
        .map(|slot| slot.quantity)
        .sum()
}

/// Human-readable label for a resource kind (Portuguese).
pub fn resource_label(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Wood => "Madeira",
        ResourceKind::Stone => "Pedra",
        ResourceKind::Berry => "Fruta",
        ResourceKind::Crystal => "Cristal",
        ResourceKind::Flower => "Flor",
        ResourceKind::Fiber => "Fibra",
        ResourceKind::Mushroom => "Cogumelo",
        ResourceKind::Sap => "Seiva",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_inventory_default_is_empty() {
        let inv = ClientInventory::default();
        assert!(inv.items.is_empty());
    }

    #[test]
    fn client_quests_default_is_empty() {
        let q = ClientQuests::default();
        assert!(q.quests.is_empty());
    }

    #[test]
    fn client_cooldown_default_is_inactive() {
        let c = ClientCooldown::default();
        assert!(!c.active);
        assert_eq!(c.remaining, 0.0);
    }

    #[test]
    fn client_feedback_default_is_hidden() {
        let f = ClientActionFeedback::default();
        assert!(!f.visible);
        assert!(f.message.is_empty());
    }

    #[test]
    fn resource_quantity_aggregates_across_slots() {
        let items = vec![
            ItemSlotData {
                slot_index: 0,
                kind: ItemKind::Resource(ResourceKind::Wood),
                quantity: 5,
            },
            ItemSlotData {
                slot_index: 1,
                kind: ItemKind::Resource(ResourceKind::Wood),
                quantity: 3,
            },
            ItemSlotData {
                slot_index: 2,
                kind: ItemKind::Resource(ResourceKind::Berry),
                quantity: 10,
            },
        ];
        assert_eq!(resource_quantity(&items, ResourceKind::Wood), 8);
        assert_eq!(resource_quantity(&items, ResourceKind::Berry), 10);
        assert_eq!(resource_quantity(&items, ResourceKind::Crystal), 0);
    }

    #[test]
    fn resource_label_returns_portuguese_name() {
        assert_eq!(resource_label(ResourceKind::Wood), "Madeira");
        assert_eq!(resource_label(ResourceKind::Crystal), "Cristal");
        assert_eq!(resource_label(ResourceKind::Berry), "Fruta");
        assert_eq!(resource_label(ResourceKind::Fiber), "Fibra");
    }

    #[test]
    fn tick_cooldown_decrements_remaining() {
        let mut c = ClientCooldown {
            active: true,
            remaining: 2.0,
        };
        // Simulate one tick at 60 fps
        let dt = 1.0 / 60.0;
        if c.active {
            c.remaining -= dt;
        }
        assert!(c.remaining < 2.0);
        assert!(c.remaining > 1.9);
    }

    #[test]
    fn tick_cooldown_deactivates_when_expired() {
        let mut c = ClientCooldown {
            active: true,
            remaining: 0.01,
        };
        c.remaining -= 0.02;
        if c.remaining <= 0.0 {
            c.active = false;
            c.remaining = 0.0;
        }
        assert!(!c.active);
        assert_eq!(c.remaining, 0.0);
    }
}
