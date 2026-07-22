use bevy::prelude::*;
use lightyear::connection::client::Connect;
use lightyear::prelude::*;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct ReconnectButton;

pub fn spawn_hud(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        })
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
                        margin: UiRect::top(Val::Px(6.0)),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.25, 0.25, 0.3)),
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
}

type ClientConnectionState<'w, 's> =
    Query<'w, 's, (Has<Connected>, Has<Connecting>, Has<Disconnected>), With<Client>>;

pub fn update_hud_status(
    client: ClientConnectionState,
    mut texts: Query<(&mut Text, &mut TextColor), With<StatusText>>,
) {
    let Ok((mut text, mut color)) = texts.single_mut() else {
        return;
    };
    let (label, rgb) = match client.single() {
        Ok((true, _, _)) => ("Conectado", (0.4, 0.9, 0.4)),
        Ok((_, true, _)) => ("Conectando...", (0.9, 0.8, 0.3)),
        Ok((_, _, true)) => ("Desconectado - reconectando...", (0.9, 0.4, 0.4)),
        _ => ("Sem conexao (config invalida?)", (0.9, 0.4, 0.4)),
    };
    text.0 = label.into();
    *color = TextColor(Color::srgb(rgb.0, rgb.1, rgb.2));
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

    fn hud_app() -> App {
        let mut app = App::new();
        app.init_resource::<lightyear::connection::client::PeerMetadata>();
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
    fn hud_shows_config_error_without_client_entity() {
        let mut app = hud_app();
        assert_eq!(status_text(&mut app), "Sem conexao (config invalida?)");
    }

    #[test]
    fn hud_reflects_connection_markers() {
        let mut app = hud_app();
        let client = app
            .world_mut()
            .spawn((Client::default(), Connected, RemoteId(PeerId::Netcode(1))))
            .id();
        app.update();
        assert_eq!(status_text(&mut app), "Conectado");

        app.world_mut()
            .entity_mut(client)
            .remove::<Connected>()
            .insert(Disconnected::default());
        app.update();
        assert_eq!(status_text(&mut app), "Desconectado - reconectando...");
    }
}
