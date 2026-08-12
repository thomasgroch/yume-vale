use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::touch::TouchJump;

const NO_EVENT: u8 = 0;
const HIDDEN: u8 = 1;
const VISIBLE: u8 = 2;

static PAGE_EVENT: AtomicU8 = AtomicU8::new(NO_EVENT);

/// Tracks whether the page is currently visible.
///
/// Hiding/showing the tab does **not** by itself disconnect or reconnect
/// anything — a healthy connection is left alone. Reconnect attempts are
/// just paused while hidden (`blocks_retry`), since there's no point
/// retrying a dead connection nobody is looking at. If the underlying
/// connection actually died while backgrounded (the browser killed it, or
/// the server's inactivity timeout fired), lightyear will already have
/// transitioned the client to `Disconnected` on its own, and
/// `retry_connect_when_disconnected` picks that up as soon as the page is
/// visible again.
///
/// An earlier version force-disconnected on every hide *and* every show,
/// even for a perfectly healthy connection — every tab switch or app swap
/// dropped the player and respawned them at the map origin. Don't
/// reintroduce that.
#[derive(Resource)]
pub(crate) struct PageLifecycle {
    visible: bool,
    /// Test-only injection point for a pending event, so unit tests don't
    /// have to race the process-global `PAGE_EVENT` static against other
    /// tests running concurrently in the same binary.
    pending_event: u8,
}

impl Default for PageLifecycle {
    fn default() -> Self {
        Self {
            visible: true,
            pending_event: NO_EVENT,
        }
    }
}

impl PageLifecycle {
    pub(crate) fn blocks_retry(&self) -> bool {
        !self.visible
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn install_visibility_listener() {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let observed = document.clone();
    let callback = Closure::<dyn FnMut()>::new(move || {
        PAGE_EVENT.store(
            if observed.hidden() { HIDDEN } else { VISIBLE },
            Ordering::Release,
        );
    });
    if document
        .add_event_listener_with_callback("visibilitychange", callback.as_ref().unchecked_ref())
        .is_ok()
    {
        callback.forget();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn install_visibility_listener() {}

#[derive(SystemParam)]
pub(crate) struct VisibilityInput<'w> {
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    touches: ResMut<'w, Touches>,
    touch_jump: ResMut<'w, TouchJump>,
}

pub(crate) fn handle_page_visibility(
    mut lifecycle: ResMut<PageLifecycle>,
    mut input: VisibilityInput,
) {
    let event = if lifecycle.pending_event == NO_EVENT {
        PAGE_EVENT.swap(NO_EVENT, Ordering::AcqRel)
    } else {
        std::mem::replace(&mut lifecycle.pending_event, NO_EVENT)
    };
    if event == NO_EVENT {
        return;
    }

    // A keyup/touchend can be missed while the tab was hidden, which would
    // otherwise leave input stuck "pressed" forever.
    input.keys.reset_all();
    input.touches.reset_all();
    input.touch_jump.0 = false;

    lifecycle.visible = match event {
        HIDDEN => false,
        VISIBLE => true,
        _ => return,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifecycle_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<Touches>();
        app.init_resource::<TouchJump>();
        app.init_resource::<PageLifecycle>();
        app.add_systems(Update, handle_page_visibility);
        app
    }

    #[test]
    fn hidden_page_resets_pressed_input_and_blocks_retry() {
        let mut app = lifecycle_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.world_mut().resource_mut::<TouchJump>().0 = true;

        app.world_mut()
            .resource_mut::<PageLifecycle>()
            .pending_event = HIDDEN;
        app.update();

        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyW)
        );
        assert!(!app.world().resource::<TouchJump>().0);
        assert!(app.world().resource::<PageLifecycle>().blocks_retry());
    }

    #[test]
    fn visible_page_unblocks_retry_without_touching_input() {
        let mut app = lifecycle_app();
        app.world_mut().resource_mut::<PageLifecycle>().visible = false;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);

        app.world_mut()
            .resource_mut::<PageLifecycle>()
            .pending_event = VISIBLE;
        app.update();

        assert!(!app.world().resource::<PageLifecycle>().blocks_retry());
        // Visibility events reset input regardless of direction (defends
        // against a missed keyup while hidden), so this is intentionally
        // reset too.
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyW)
        );
    }

    #[test]
    fn no_event_leaves_lifecycle_untouched() {
        let mut app = lifecycle_app();
        app.world_mut().resource_mut::<PageLifecycle>().visible = false;
        app.update();
        assert!(app.world().resource::<PageLifecycle>().blocks_retry());
    }
}
