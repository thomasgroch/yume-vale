use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContext, PrimaryEguiContext};
use bevy_inspector_egui::egui;

#[derive(Resource, Default)]
pub struct DebugMode(pub bool);

pub fn toggle_debug_mode(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<DebugMode>) {
    if keys.just_pressed(KeyCode::F3) {
        mode.0 = !mode.0;
    }
}

pub fn inspector_ui(world: &mut World) {
    if !world.resource::<DebugMode>().0 {
        return;
    }
    let Ok(context) = world
        .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
        .single(world)
    else {
        return;
    };
    let mut context = context.clone();
    egui::Window::new("Debug").show(context.get_mut(), |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            bevy_inspector_egui::bevy_inspector::ui_for_world(world, ui);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f3_toggles_debug_mode() {
        let mut app = App::new();
        app.init_resource::<DebugMode>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, toggle_debug_mode);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F3);
        app.update();
        assert!(app.world().resource::<DebugMode>().0);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::F3);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F3);
        app.update();
        assert!(!app.world().resource::<DebugMode>().0);
    }
}
