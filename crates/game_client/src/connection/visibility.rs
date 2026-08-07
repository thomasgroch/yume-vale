use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use lightyear::connection::client::{Connect, Disconnect};
use lightyear::prelude::{Client, Disconnected};
use std::sync::atomic::{AtomicU8, Ordering};

use super::transport_fallback::TransportState;
use super::welcome::LocalPlayerId;
use crate::touch::TouchJump;

const NO_EVENT: u8 = 0;
const HIDDEN: u8 = 1;
const VISIBLE: u8 = 2;

static PAGE_EVENT: AtomicU8 = AtomicU8::new(NO_EVENT);

#[derive(Resource)]
pub(crate) struct PageLifecycle {
    visible: bool,
    reconnect_pending: bool,
    connect_triggered: bool,
    pending_event: u8,
}

impl Default for PageLifecycle {
    fn default() -> Self {
        Self {
            visible: true,
            reconnect_pending: false,
            connect_triggered: false,
            pending_event: NO_EVENT,
        }
    }
}

impl PageLifecycle {
    pub(crate) fn blocks_retry(&self) -> bool {
        !self.visible || self.reconnect_pending
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

type Clients<'w, 's> = Query<'w, 's, (Entity, Has<Disconnected>), With<Client>>;

#[derive(SystemParam)]
pub(crate) struct VisibilityInput<'w> {
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    touches: ResMut<'w, Touches>,
    touch_jump: ResMut<'w, TouchJump>,
    local_player: ResMut<'w, LocalPlayerId>,
}

pub(crate) fn handle_page_visibility(
    mut commands: Commands,
    mut lifecycle: ResMut<PageLifecycle>,
    transport: Res<TransportState>,
    mut input: VisibilityInput,
    clients: Clients,
) {
    let event = if lifecycle.pending_event == NO_EVENT {
        PAGE_EVENT.swap(NO_EVENT, Ordering::AcqRel)
    } else {
        std::mem::replace(&mut lifecycle.pending_event, NO_EVENT)
    };
    if event != NO_EVENT {
        input.keys.reset_all();
        input.touches.reset_all();
        input.touch_jump.0 = false;
        input.local_player.id = None;
    }

    match event {
        HIDDEN => {
            lifecycle.visible = false;
            lifecycle.reconnect_pending = true;
            lifecycle.connect_triggered = false;
            for (entity, disconnected) in &clients {
                if !disconnected {
                    commands
                        .entity(entity)
                        .trigger(|entity| Disconnect { entity });
                }
            }
        }
        VISIBLE => {
            lifecycle.visible = true;
            lifecycle.reconnect_pending = true;
            lifecycle.connect_triggered = false;
            for (entity, disconnected) in &clients {
                if !disconnected {
                    commands
                        .entity(entity)
                        .trigger(|entity| Disconnect { entity });
                }
            }
        }
        _ => {}
    }

    if transport.rejection_received {
        lifecycle.reconnect_pending = false;
        lifecycle.connect_triggered = false;
        return;
    }
    if !lifecycle.visible || !lifecycle.reconnect_pending {
        return;
    }

    if lifecycle.connect_triggered {
        if clients.iter().all(|(_, disconnected)| !disconnected) {
            lifecycle.reconnect_pending = false;
            lifecycle.connect_triggered = false;
        }
        return;
    }

    let mut waiting = false;
    for (entity, disconnected) in &clients {
        if disconnected {
            commands.entity(entity).trigger(|entity| Connect { entity });
        } else {
            waiting = true;
        }
    }
    if !waiting {
        lifecycle.connect_triggered = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightyear::connection::client::PeerMetadata;
    use lightyear::prelude::{Connected, PeerId, RemoteId};
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    fn lifecycle_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<Touches>();
        app.init_resource::<TouchJump>();
        app.init_resource::<LocalPlayerId>();
        app.init_resource::<PageLifecycle>();
        app.init_resource::<PeerMetadata>();
        app.insert_resource(TransportState::default());
        app.add_systems(Update, handle_page_visibility);
        app
    }

    #[test]
    fn hidden_page_resets_pressed_input_and_requests_disconnect() {
        let mut app = lifecycle_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.world_mut().resource_mut::<TouchJump>().0 = true;
        app.world_mut().resource_mut::<LocalPlayerId>().id = Some(game_core::id::PlayerId::new(1));
        app.world_mut()
            .spawn((Client::default(), Connected, RemoteId(PeerId::Server)));

        let disconnects = Arc::new(AtomicUsize::new(0));
        let observed = disconnects.clone();
        app.add_observer(move |_: On<Disconnect>| {
            observed.fetch_add(1, Ordering::SeqCst);
        });
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
        assert!(app.world().resource::<LocalPlayerId>().id.is_none());
        assert_eq!(disconnects.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn visible_page_connects_only_after_disconnected() {
        let mut app = lifecycle_app();
        let client = app
            .world_mut()
            .spawn((Client::default(), Connected, RemoteId(PeerId::Server)))
            .id();
        let connects = Arc::new(AtomicUsize::new(0));
        let observed = connects.clone();
        app.add_observer(move |_: On<Connect>| {
            observed.fetch_add(1, Ordering::SeqCst);
        });

        app.world_mut()
            .resource_mut::<PageLifecycle>()
            .pending_event = VISIBLE;
        app.update();
        assert_eq!(connects.load(Ordering::SeqCst), 0);

        app.world_mut().entity_mut(client).remove::<Connected>();
        app.world_mut()
            .entity_mut(client)
            .insert(Disconnected::default());
        app.update();
        assert_eq!(connects.load(Ordering::SeqCst), 1);
        app.update();
        assert_eq!(connects.load(Ordering::SeqCst), 1);
    }
}
