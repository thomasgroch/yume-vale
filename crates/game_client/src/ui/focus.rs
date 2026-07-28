use bevy::prelude::*;
use bevy::ui::Outline;

use crate::ui::theme;

/// Stores the currently keyboard-focused entity, if any.
#[derive(Resource, Default)]
pub struct FocusState {
    pub focused: Option<Entity>,
}

/// Tab/Shift+Tab keyboard focus cycling for all [`Button`] entities.
///
/// Applies a visible [`Outline`] to the focused button and removes it from
/// the previously focused one.
pub fn manage_focus(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<FocusState>,
    mut commands: Commands,
    buttons: Query<Entity, With<Button>>,
) {
    let tab = keys.just_pressed(KeyCode::Tab);
    if !tab {
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    let mut ids: Vec<Entity> = buttons.iter().collect();
    ids.sort_by_key(|e| e.to_bits());
    if ids.is_empty() {
        return;
    }

    let current = state.focused.and_then(|e| ids.contains(&e).then_some(e));
    let next_idx = match current {
        None => 0,
        Some(cur) => {
            let pos = ids.iter().position(|&e| e == cur).unwrap_or(0);
            if shift {
                (pos + ids.len() - 1) % ids.len()
            } else {
                (pos + 1) % ids.len()
            }
        }
    };

    if let Some(prev) = current {
        commands.entity(prev).remove::<Outline>();
    }
    let next = ids[next_idx];
    commands
        .entity(next)
        .insert(Outline::new(Val::Px(2.0), Val::Px(1.0), theme::TEXT_TITLE));
    state.focused = Some(next);
}

/// Clean up stale focus reference when the focused entity is despawned.
pub fn clear_stale_focus(mut state: ResMut<FocusState>, buttons: Query<Entity, With<Button>>) {
    let Some(focused) = state.focused else {
        return;
    };
    if buttons.get(focused).is_err() {
        state.focused = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_state_default_is_none() {
        let s = FocusState::default();
        assert!(s.focused.is_none());
    }

    #[test]
    fn tab_focuses_some_button_and_applies_outline() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<FocusState>();
        app.add_systems(Update, manage_focus);

        let btn_ids: Vec<Entity> = (0..2)
            .map(|_| app.world_mut().spawn((Button, Node::default())).id())
            .collect();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();

        let state = app.world().resource::<FocusState>();
        assert!(state.focused.is_some(), "Tab focuses a button");
        let focused = state.focused.unwrap();
        assert!(
            app.world().get::<Outline>(focused).is_some(),
            "focused button has Outline"
        );
        assert!(
            btn_ids.contains(&focused),
            "focused entity is one of the spawned buttons"
        );
    }

    #[test]
    fn second_tab_moves_to_different_button() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<FocusState>();
        app.add_systems(Update, manage_focus);

        let btn_ids: Vec<Entity> = (0..2)
            .map(|_| app.world_mut().spawn((Button, Node::default())).id())
            .collect();

        // First Tab
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
        let first = app.world().resource::<FocusState>().focused;
        assert!(first.is_some(), "first Tab sets focus");
        let first_entity = first.unwrap();
        assert!(btn_ids.contains(&first_entity));

        // Second Tab
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
        let second = app.world().resource::<FocusState>().focused;
        assert!(second.is_some(), "second Tab sets focus");
        let second_entity = second.unwrap();
        assert_ne!(first_entity, second_entity, "Tab moves to different button");

        assert!(
            app.world().get::<Outline>(first_entity).is_none(),
            "previous button Outline removed"
        );
    }
}
