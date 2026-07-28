use bevy::prelude::*;
use game_core::resources::ResourceKind;

use crate::ui::{theme, widgets};

use super::GameplayPanel;
use super::snapshot::{
    ClientActionFeedback, ClientCooldown, ClientInventory, resource_label, resource_quantity,
};

// ─── Components ────────────────────────────────────────────────────────

/// Root marker for the inventory panel.
#[derive(Component)]
pub struct InventoryPanel;

/// Marks a text entity that shows a specific resource quantity.
#[derive(Component)]
pub struct ResourceQuantity(pub ResourceKind);

/// Marks the interaction-prompt text entity.
#[derive(Component)]
pub struct InteractionPrompt;

/// Marks the feedback text (cooldown / error).
#[derive(Component)]
pub struct FeedbackText;

// ─── Spawn ─────────────────────────────────────────────────────────────

/// Create the inventory panel UI (hidden by default, shown via
/// `GameplayPanel` visibility gating).
pub fn spawn_inventory_panel(mut commands: Commands) {
    commands
        .spawn((
            InventoryPanel,
            GameplayPanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(theme::SPACE_10),
                left: Val::Px(theme::SPACE_10),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme::SPACE_8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Inventário"),
                widgets::text_font(theme::FONT_SM),
                TextColor(Color::WHITE),
            ));

            for kind in RESOURCE_ORDER {
                parent.spawn((
                    Text::new(format!("{}: 0", resource_label(*kind))),
                    widgets::text_font(theme::FONT_XS),
                    TextColor(Color::WHITE),
                    ResourceQuantity(*kind),
                    Node {
                        margin: UiRect::left(Val::Px(theme::SPACE_4)),
                        ..default()
                    },
                ));
            }

            parent.spawn((
                Text::new(""),
                widgets::text_font(theme::FONT_SM),
                TextColor(Color::WHITE),
                InteractionPrompt,
                Node {
                    margin: UiRect::top(Val::Px(theme::SPACE_4)),
                    ..default()
                },
            ));

            parent.spawn((
                Text::new(""),
                widgets::text_font(theme::FONT_SM),
                TextColor(theme::STATUS_BUSY),
                FeedbackText,
            ));
        });
}

const RESOURCE_ORDER: &[ResourceKind] = &[
    ResourceKind::Wood,
    ResourceKind::Crystal,
    ResourceKind::Berry,
    ResourceKind::Fiber,
];

// ─── Update ────────────────────────────────────────────────────────────

/// Update resource quantities from `ClientInventory`.
#[allow(clippy::type_complexity)]
pub fn update_inventory_panel(
    client_inv: Res<ClientInventory>,
    cooldown: Res<ClientCooldown>,
    error: Res<ClientActionFeedback>,
    mut texts: ParamSet<(
        Query<(&mut Text, &ResourceQuantity)>,
        Query<&mut Text, (With<InteractionPrompt>, Without<ResourceQuantity>)>,
        Query<&mut Text, (With<FeedbackText>, Without<ResourceQuantity>)>,
    )>,
) {
    for (mut text, rq) in &mut texts.p0() {
        let qty = resource_quantity(&client_inv.items, rq.0);
        text.0 = format!("{}: {qty}", resource_label(rq.0));
    }

    if let Ok(mut prompt_text) = texts.p1().single_mut() {
        prompt_text.0 = "Pressione E para interagir".to_string();
    }

    if let Ok(mut fb_text) = texts.p2().single_mut() {
        if error.visible {
            fb_text.0 = format!("Erro: {}", error.message);
        } else if cooldown.active {
            fb_text.0 = format!("Aguarde... {:.0}s", cooldown.remaining.ceil());
        } else {
            fb_text.0.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::inventory::ItemKind;
    use game_protocol::messages::ItemSlotData;

    fn inventory_app() -> App {
        let mut app = App::new();
        app.init_resource::<ClientInventory>();
        app.init_resource::<ClientCooldown>();
        app.init_resource::<ClientActionFeedback>();
        app.add_systems(Startup, spawn_inventory_panel);
        app.add_systems(Update, update_inventory_panel);
        app.update();
        app
    }

    fn resource_text(app: &mut App, kind: ResourceKind) -> String {
        let mut query = app
            .world_mut()
            .query_filtered::<&Text, With<ResourceQuantity>>();
        let world = app.world();
        query
            .iter(world)
            .find(|t| t.0.starts_with(resource_label(kind)))
            .map(|t| t.0.clone())
            .unwrap_or_default()
    }

    fn feedback_text(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<FeedbackText>>()
            .single(app.world())
            .map(|t| t.0.clone())
            .unwrap_or_default()
    }

    fn prompt_text(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<InteractionPrompt>>()
            .single(app.world())
            .map(|t| t.0.clone())
            .unwrap_or_default()
    }

    #[test]
    fn empty_inventory_shows_all_zeros() {
        let mut app = inventory_app();
        assert_eq!(resource_text(&mut app, ResourceKind::Wood), "Madeira: 0");
        assert_eq!(resource_text(&mut app, ResourceKind::Crystal), "Cristal: 0");
        assert_eq!(resource_text(&mut app, ResourceKind::Berry), "Fruta: 0");
        assert_eq!(resource_text(&mut app, ResourceKind::Fiber), "Fibra: 0");
    }

    #[test]
    fn partial_inventory_shows_correct_quantities() {
        let mut app = inventory_app();
        app.world_mut().resource_mut::<ClientInventory>().items = vec![
            ItemSlotData {
                slot_index: 0,
                kind: ItemKind::Resource(ResourceKind::Wood),
                quantity: 3,
            },
            ItemSlotData {
                slot_index: 1,
                kind: ItemKind::Resource(ResourceKind::Berry),
                quantity: 7,
            },
        ];
        app.update();
        assert_eq!(resource_text(&mut app, ResourceKind::Wood), "Madeira: 3");
        assert_eq!(resource_text(&mut app, ResourceKind::Berry), "Fruta: 7");
        assert_eq!(resource_text(&mut app, ResourceKind::Crystal), "Cristal: 0");
        assert_eq!(resource_text(&mut app, ResourceKind::Fiber), "Fibra: 0");
    }

    #[test]
    fn inventory_updates_labels_on_snapshot_change() {
        let mut app = inventory_app();

        // First snapshot
        app.world_mut().resource_mut::<ClientInventory>().items = vec![ItemSlotData {
            slot_index: 0,
            kind: ItemKind::Resource(ResourceKind::Wood),
            quantity: 5,
        }];
        app.update();
        assert_eq!(resource_text(&mut app, ResourceKind::Wood), "Madeira: 5");

        // Second snapshot — labels update exactly once per change
        app.world_mut().resource_mut::<ClientInventory>().items = vec![ItemSlotData {
            slot_index: 0,
            kind: ItemKind::Resource(ResourceKind::Wood),
            quantity: 8,
        }];
        app.update();
        assert_eq!(resource_text(&mut app, ResourceKind::Wood), "Madeira: 8");
    }

    #[test]
    fn interaction_prompt_is_shown() {
        let mut app = inventory_app();
        let prompt = prompt_text(&mut app);
        assert!(!prompt.is_empty(), "interaction prompt should be visible");
        assert!(prompt.contains("Pressione E"));
    }

    #[test]
    fn cooldown_shows_visual_state() {
        let mut app = inventory_app();
        app.world_mut().resource_mut::<ClientCooldown>().active = true;
        app.world_mut().resource_mut::<ClientCooldown>().remaining = 3.5;
        app.update();
        let fb = feedback_text(&mut app);
        assert!(
            fb.contains("Aguarde"),
            "cooldown feedback shows wait message"
        );
        assert!(fb.contains("4"), "remaining rounded up appears in feedback");
    }

    #[test]
    fn error_shows_persistence_busy_state() {
        let mut app = inventory_app();
        app.world_mut()
            .resource_mut::<ClientActionFeedback>()
            .visible = true;
        app.world_mut()
            .resource_mut::<ClientActionFeedback>()
            .message = "persistence busy: database locked".to_string();
        app.update();
        let fb = feedback_text(&mut app);
        assert!(fb.contains("Erro"));
        assert!(fb.contains("database locked"));
    }

    #[test]
    fn no_feedback_when_idle() {
        let mut app = inventory_app();
        let fb = feedback_text(&mut app);
        assert!(fb.is_empty(), "feedback text is empty when idle");
    }
}
