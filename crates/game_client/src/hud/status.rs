use bevy::prelude::*;
use lightyear::connection::client::Connect;
use lightyear::prelude::*;

use crate::connection::TransportState;
use crate::ui::{theme, widgets};

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct ReconnectButton;

pub fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(theme::SPACE_10),
                left: Val::Px(theme::SPACE_10),
                ..default()
            },
            ZIndex(-1),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("..."),
                TextFont::default(),
                TextColor(Color::WHITE),
                TextShadow::default(),
                StatusText,
            ));
            parent
                .spawn((
                    Button,
                    Node {
                        margin: UiRect::top(Val::Px(theme::SPACE_6)),
                        min_width: Val::Px(theme::MIN_TOUCH_TARGET),
                        min_height: Val::Px(theme::MIN_TOUCH_TARGET),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(theme::SPACE_10), Val::Px(theme::SPACE_6)),
                        ..default()
                    },
                    BackgroundColor(theme::SURFACE_RECONNECT),
                    ReconnectButton,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Reconectar"),
                        TextFont::default(),
                        TextColor(Color::WHITE),
                    ));
                });
        });
    commands.spawn((
        Button,
        Text::new(""),
        widgets::text_font(theme::FONT_XS),
        TextColor(theme::TEXT_DIM),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(theme::SPACE_8),
            right: Val::Px(theme::SPACE_10),
            ..default()
        },
        // Above the menu's full-screen background (default z=0) so it stays
        // visible before connecting, not just once in-game.
        ZIndex(10),
        super::version::VersionText,
    ));
}

#[allow(clippy::type_complexity)]
pub(crate) fn update_hud_status(
    client: Query<(Has<Connected>, Has<Connecting>, Has<Disconnected>), With<Client>>,
    links: Query<&Link, With<Client>>,
    names: Query<&player::PlayerName, With<player::LocalPlayer>>,
    transport: Res<TransportState>,
    mut texts: Query<(&mut Text, &mut TextColor), With<StatusText>>,
) {
    let Ok((mut text, mut color)) = texts.single_mut() else {
        return;
    };
    let transport_label = transport.mode.short_name();
    let (label, status) = match client.single() {
        Ok((true, _, _)) => {
            let base = match names.single() {
                Ok(name) => format!("Conectado - {} [{}]", name.0, transport_label),
                Err(_) => format!("Conectado [{}]", transport_label),
            };
            let label = match links.single() {
                Ok(link) if !link.stats.rtt.is_zero() => {
                    format!("{base} | {}ms", link.stats.rtt.as_millis())
                }
                _ => base,
            };
            (label, theme::STATUS_OK)
        }
        Ok((_, true, _)) => ("Conectando...".to_string(), theme::STATUS_BUSY),
        Ok((_, _, true)) => (
            "Desconectado - reconectando...".to_string(),
            theme::STATUS_ERR,
        ),
        _ => (
            "Sem conexao (config invalida?)".to_string(),
            theme::STATUS_ERR,
        ),
    };
    text.0 = label;
    *color = TextColor(status);
}

pub fn reconnect_button(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ReconnectButton>)>,
    clients: Query<Entity, (With<Client>, With<Disconnected>)>,
    mut commands: Commands,
) {
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            for entity in &clients {
                commands.entity(entity).trigger(|e| Connect { entity: e });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightyear::connection::client::PeerMetadata;

    fn status_app() -> App {
        let mut app = App::new();
        app.init_resource::<PeerMetadata>();
        app.insert_resource(crate::connection::TransportState::detect());
        app.add_systems(Startup, spawn_hud);
        app.add_systems(Update, update_hud_status);
        app.update();
        app
    }

    fn status_text(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<StatusText>>()
            .single(app.world())
            .unwrap()
            .0
            .clone()
    }

    #[test]
    fn spawn_hud_creates_status_text_entity() {
        let mut app = status_app();
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<StatusText>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "spawn_hud must create exactly one StatusText");
    }

    #[test]
    fn spawn_hud_creates_reconnect_button_entity() {
        let mut app = status_app();
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<ReconnectButton>>()
            .iter(app.world())
            .count();
        assert_eq!(
            count, 1,
            "spawn_hud must create exactly one ReconnectButton"
        );
    }

    #[test]
    fn hud_shows_config_error_without_client_entity() {
        let mut app = status_app();
        assert_eq!(status_text(&mut app), "Sem conexao (config invalida?)");
    }

    #[test]
    fn hud_reflects_connection_markers() {
        let mut app = status_app();
        let client = app
            .world_mut()
            .spawn((Client::default(), Connected, RemoteId(PeerId::Netcode(1))))
            .id();
        app.update();
        let text = status_text(&mut app);
        assert!(
            text.starts_with("Conectado"),
            "expected connected status, got: {text}"
        );
        assert!(
            text.contains("[WT]") || text.contains("[WS]"),
            "expected transport label in status, got: {text}"
        );

        app.world_mut()
            .entity_mut(client)
            .remove::<Connected>()
            .insert(Disconnected::default());
        app.update();
        assert_eq!(status_text(&mut app), "Desconectado - reconectando...");
    }

    #[test]
    fn hud_shows_local_player_name() {
        let mut app = status_app();
        app.world_mut()
            .spawn((Client::default(), Connected, RemoteId(PeerId::Netcode(1))));
        app.world_mut().spawn((
            player::PlayerName("Player 1".to_string()),
            player::LocalPlayer,
        ));
        app.update();
        let text = status_text(&mut app);
        assert!(
            text.contains("Player 1"),
            "expected player name in status, got: {text}"
        );
        assert!(
            text.contains("[WT]") || text.contains("[WS]"),
            "expected transport label, got: {text}"
        );
    }

    #[test]
    fn hud_shows_ping_when_link_has_rtt() {
        let mut app = status_app();
        let mut link = Link::new(None);
        link.stats.rtt = core::time::Duration::from_millis(34);
        app.world_mut().spawn((
            Client::default(),
            Connected,
            RemoteId(PeerId::Netcode(1)),
            link,
        ));
        app.update();
        let text = status_text(&mut app);
        assert!(
            text.contains("34ms"),
            "expected ping in status, got: {text}"
        );
        assert!(
            text.contains("[WT]") || text.contains("[WS]"),
            "expected transport label, got: {text}"
        );
    }

    #[test]
    fn status_text_shows_connecting_state() {
        let mut app = status_app();
        app.world_mut().spawn((Client::default(), Connecting));
        app.update();
        let text = status_text(&mut app);
        assert_eq!(text, "Conectando...");
    }

    #[test]
    fn status_text_shows_disconnected_state() {
        let mut app = status_app();
        app.world_mut()
            .spawn((Client::default(), Disconnected::default()));
        app.update();
        let text = status_text(&mut app);
        assert_eq!(text, "Desconectado - reconectando...");
    }

    #[test]
    fn reconnect_button_reacts_to_press() {
        let mut app = status_app();
        let btn = app
            .world_mut()
            .query_filtered::<Entity, With<ReconnectButton>>()
            .single(app.world())
            .unwrap();
        app.world_mut().entity_mut(btn).insert(Interaction::Pressed);
        app.update();
    }
}
