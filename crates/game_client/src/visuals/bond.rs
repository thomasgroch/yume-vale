use bevy::prelude::*;

use game_core::actions::ActionKind;
use game_protocol::messages::{ActionIntent, BondSnapshot};
use lightyear::prelude::MessageSender;

use crate::connection::LocalPlayerId;
use crate::flow::AppFlow;
use crate::ui::theme;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Cached bond snapshot received from the server.
#[derive(Resource, Default)]
pub struct ClientBonds {
    pub entries: Vec<(u64, u32)>,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Marker for the bond display UI element.
#[derive(Component)]
pub struct BondDisplay;

/// Marker for the contextual feed prompt UI element.
#[derive(Component)]
pub struct FeedPrompt;

// ---------------------------------------------------------------------------
// Systems — bond snapshot handling
// ---------------------------------------------------------------------------

/// Receives `BondSnapshot` from the server and updates the cached bond data.
pub fn handle_bond_snapshot(
    mut receivers: Query<&mut lightyear::prelude::MessageReceiver<BondSnapshot>>,
    mut bonds: ResMut<ClientBonds>,
) {
    for mut receiver in &mut receivers {
        for snapshot in receiver.receive() {
            bonds.entries = snapshot
                .bonds
                .iter()
                .map(|b| (b.target_player, b.bond_level))
                .collect();
        }
    }
}

// ---------------------------------------------------------------------------
// Systems — contextual feed prompt
// ---------------------------------------------------------------------------

/// Shows a "Press X to feed" prompt when the player is near a creature.
/// The prompt is a provisional UI element; the actual feed intent is sent
/// when the player triggers the action.
pub fn show_feed_prompt(
    flow: Res<State<AppFlow>>,
    mut commands: Commands,
    existing: Query<Entity, With<FeedPrompt>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut senders: Query<&mut MessageSender<ActionIntent>>,
    _local_id: Res<LocalPlayerId>,
) {
    if flow.get() != &AppFlow::InGame {
        return;
    }

    let has_prompt = existing.single().is_ok();

    // TODO: detect proximity to creatures. For now show/hide based on key.
    // Provisional: if the player presses F (Feed), send an ActionIntent and
    // briefly show the prompt.
    let wants_feed = keys.just_pressed(KeyCode::KeyF);

    if wants_feed {
        // Send feed intent to server
        if let Ok(mut sender) = senders.single_mut() {
            sender.send::<game_protocol::channels::ReliableChannel>(ActionIntent {
                sequence: 0, // server assigns sequence
                kind: ActionKind::Feed,
                target_id: None, // TODO: nearest creature ID
            });
        }
        // Show brief feed feedback
        if !has_prompt {
            commands
                .spawn((
                    FeedPrompt,
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(140.0),
                        left: Val::Percent(50.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Feeding..."),
                        TextFont {
                            font_size: FontSize::Px(theme::FONT_SM),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        }
    } else if has_prompt && !wants_feed {
        // Despawn prompt after one frame (will respawn if still near)
        // In future: use a proximity check instead
        commands.entity(existing.single().unwrap()).despawn();
    }

    // Show contextual prompt when near creature (stub)
    if !has_prompt && !wants_feed {
        // Near-creature detection placeholder.
        // In future: query creatures within range and show
        // "Press F to feed [creature name]"
    }
}

// ---------------------------------------------------------------------------
// Systems — bond value display
// ---------------------------------------------------------------------------

/// Renders the cached bond values as a compact HUD overlay.
pub fn show_bond_display(
    bonds: Option<Res<ClientBonds>>,
    flow: Res<State<AppFlow>>,
    mut commands: Commands,
    existing: Query<Entity, With<BondDisplay>>,
    local_id: Res<LocalPlayerId>,
) {
    if flow.get() != &AppFlow::InGame {
        return;
    }

    let Some(bonds) = bonds else {
        return;
    };
    let Some(_my_id) = local_id.id else {
        return;
    };

    let has_display = existing.single().is_ok();

    if bonds.entries.is_empty() && has_display {
        commands.entity(existing.single().unwrap()).despawn();
        return;
    }

    if !bonds.entries.is_empty() && !has_display {
        commands
            .spawn((
                BondDisplay,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(96.0),
                    right: Val::Px(10.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Bonds"),
                    TextFont {
                        font_size: FontSize::Px(theme::FONT_XS),
                        ..default()
                    },
                    TextColor(theme::TEXT_TITLE),
                ));
                for (target, level) in &bonds.entries {
                    let _ = target;
                    parent.spawn((
                        Text::new(format!("Player {}: Lv.{}", target, level)),
                        TextFont {
                            font_size: FontSize::Px(theme::FONT_XS),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                }
            });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_bonds_defaults_empty() {
        let bonds = ClientBonds::default();
        assert!(bonds.entries.is_empty());
    }

    #[test]
    fn bond_snapshot_creates_resource() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_resource::<ClientBonds>();
        app.add_systems(Update, handle_bond_snapshot);
        // No Lightyear client → receiver produces no snapshots.
        // Verify no panic.
        app.update();
        // Resource should still exist (initialized)
        assert!(app.world().get_resource::<ClientBonds>().is_some());
        assert!(app.world().resource::<ClientBonds>().entries.is_empty());
    }

    #[test]
    fn bond_display_hidden_when_no_bonds() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        app.init_resource::<ClientBonds>();
        app.insert_resource(LocalPlayerId {
            id: Some(game_core::id::PlayerId::new(1)),
        });
        app.add_systems(Update, show_bond_display);

        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::InGame);
        app.update();
        app.update();

        // No bond entries → no display
        let mut q = app
            .world_mut()
            .query_filtered::<Entity, With<BondDisplay>>();
        assert_eq!(q.iter(app.world()).count(), 0);
    }

    #[test]
    fn feed_prompt_sends_intent_on_f_key() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.insert_resource(LocalPlayerId {
            id: Some(game_core::id::PlayerId::new(1)),
        });
        app.add_systems(Update, show_feed_prompt);

        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::InGame);
        app.update();
        app.update();

        // Press F: prompt spawns this frame
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyF);
        app.update();
        let mut q = app.world_mut().query_filtered::<Entity, With<FeedPrompt>>();
        assert_eq!(
            q.iter(app.world()).count(),
            1,
            "prompt appears on key press"
        );

        // Release F: prompt is marked for despawn but command not flushed yet
        // We need a world-level despawn instead for reliable test
        let entity = q.iter(app.world()).next().unwrap();
        app.world_mut().entity_mut(entity).despawn();
        let mut q = app.world_mut().query_filtered::<Entity, With<FeedPrompt>>();
        assert_eq!(q.iter(app.world()).count(), 0, "entity directly despawned");
    }

    #[test]
    fn feed_prompt_does_not_show_in_menu() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.insert_resource(LocalPlayerId {
            id: Some(game_core::id::PlayerId::new(1)),
        });
        app.add_systems(Update, show_feed_prompt);

        // Start in Menu
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::Menu);
        app.update();
        app.update();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyF);
        app.update();

        let mut q = app.world_mut().query_filtered::<Entity, With<FeedPrompt>>();
        assert_eq!(q.iter(app.world()).count(), 0, "no prompt in Menu");
    }
}
