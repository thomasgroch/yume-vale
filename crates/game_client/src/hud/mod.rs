mod inventory;
pub(crate) mod snapshot;
mod status;
pub(crate) mod version;

pub use inventory::{
    FeedbackText, InteractionPrompt, InventoryPanel, ResourceQuantity, spawn_inventory_panel,
    update_inventory_panel,
};
pub use snapshot::{
    ClientActionFeedback, ClientCooldown, ClientInventory, clear_action_feedback,
    receive_action_rejected, receive_inventory_snapshot, resource_label, resource_quantity,
    tick_cooldown,
};
pub(crate) use status::{reconnect_button, spawn_hud, update_hud_status};
pub use version::{VersionText, update_version_text};

use crate::flow::AppFlow;
use bevy::prelude::*;

/// Marker for gameplay-only panels (hidden outside InGame state).
#[derive(Component)]
pub struct GameplayPanel;

/// Toggle visibility of all `GameplayPanel`-marked entities based on
/// the current `AppFlow` state — shown only during `InGame`.
pub fn toggle_gameplay_panels(
    state: Res<State<AppFlow>>,
    mut panels: Query<&mut Visibility, With<GameplayPanel>>,
) {
    let visible = state.get() == &AppFlow::InGame;
    for mut vis in &mut panels {
        *vis = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::AppFlow;

    fn panels_hidden_when_not_ingame(state: AppFlow) -> bool {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        app.init_resource::<ClientInventory>();
        app.init_resource::<ClientCooldown>();
        app.init_resource::<ClientActionFeedback>();
        app.add_systems(Startup, spawn_inventory_panel);
        app.add_systems(Update, toggle_gameplay_panels);

        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(state);
        app.update();
        app.update(); // state transition

        app.world_mut()
            .query_filtered::<&Visibility, With<GameplayPanel>>()
            .iter(app.world())
            .all(|v| *v == Visibility::Hidden)
    }

    #[test]
    fn panels_hidden_in_loading() {
        assert!(panels_hidden_when_not_ingame(AppFlow::Loading));
    }

    #[test]
    fn panels_hidden_in_menu() {
        assert!(panels_hidden_when_not_ingame(AppFlow::Menu));
    }

    #[test]
    fn panels_visible_in_ingame() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        app.init_resource::<ClientInventory>();
        app.init_resource::<ClientCooldown>();
        app.init_resource::<ClientActionFeedback>();
        app.add_systems(Startup, spawn_inventory_panel);
        app.add_systems(Update, toggle_gameplay_panels);

        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::InGame);
        app.update();
        app.update(); // state transition

        let all_visible = app
            .world_mut()
            .query_filtered::<&Visibility, With<GameplayPanel>>()
            .iter(app.world())
            .all(|v| *v == Visibility::Visible);
        assert!(
            all_visible,
            "all GameplayPanel panels should be Visible in InGame"
        );
    }
}
