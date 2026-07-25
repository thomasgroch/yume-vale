use bevy::prelude::*;
use lightyear::connection::client::Connect;
use lightyear::prelude::*;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct ReconnectButton;

#[derive(Component)]
pub struct VersionText;

pub fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
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
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            right: Val::Px(10.0),
            ..default()
        },
        ZIndex(-1),
        VersionText,
    ));
}

type ClientConnectionState<'w, 's> =
    Query<'w, 's, (Has<Connected>, Has<Connecting>, Has<Disconnected>), With<Client>>;

pub fn update_hud_status(
    client: ClientConnectionState,
    links: Query<&Link, With<Client>>,
    names: Query<&player::PlayerName, With<player::LocalPlayer>>,
    mut texts: Query<(&mut Text, &mut TextColor), With<StatusText>>,
) {
    let Ok((mut text, mut color)) = texts.single_mut() else {
        return;
    };
    let (label, rgb) = match client.single() {
        Ok((true, _, _)) => {
            let base = match names.single() {
                Ok(name) => format!("Conectado - {}", name.0),
                Err(_) => "Conectado".to_string(),
            };
            let label = match links.single() {
                Ok(link) if !link.stats.rtt.is_zero() => {
                    format!("{base} | {}ms", link.stats.rtt.as_millis())
                }
                _ => base,
            };
            (label, (0.4, 0.9, 0.4))
        }
        Ok((_, true, _)) => ("Conectando...".to_string(), (0.9, 0.8, 0.3)),
        Ok((_, _, true)) => (
            "Desconectado - reconectando...".to_string(),
            (0.9, 0.4, 0.4),
        ),
        _ => (
            "Sem conexao (config invalida?)".to_string(),
            (0.9, 0.4, 0.4),
        ),
    };
    text.0 = label;
    *color = TextColor(Color::srgb(rgb.0, rgb.1, rgb.2));
}

pub fn update_version_text(
    time: Res<Time>,
    mut state: Local<Option<Timer>>,
    mut texts: Query<&mut Text, With<VersionText>>,
) {
    let should_update = match state.as_mut() {
        None => {
            *state = Some(Timer::from_seconds(30.0, TimerMode::Repeating));
            true
        }
        Some(timer) => {
            timer.tick(time.delta());
            timer.just_finished()
        }
    };
    if !should_update {
        return;
    }
    for mut text in &mut texts {
        text.0 = version_label();
    }
}

fn version_label() -> String {
    let ts: u64 = env!("YUME_GIT_TS").parse().unwrap_or(0);
    if ts == 0 {
        return env!("YUME_GIT_HASH").to_string();
    }
    let age = format_age(now_unix().saturating_sub(ts));
    format!("{} | {age}", env!("YUME_GIT_HASH"))
}

fn format_age(secs: u64) -> String {
    if secs < 90 {
        "agora".to_string()
    } else if secs < 3600 {
        format!("ha {}min", secs / 60)
    } else if secs < 86400 {
        format!("ha {}h", secs / 3600)
    } else {
        format!("ha {}d", secs / 86400)
    }
}

fn now_unix() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as u64
    }
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

    #[test]
    fn hud_shows_local_player_name() {
        let mut app = hud_app();
        app.world_mut()
            .spawn((Client::default(), Connected, RemoteId(PeerId::Netcode(1))));
        app.world_mut().spawn((
            player::PlayerName("Player 1".to_string()),
            player::LocalPlayer,
        ));
        app.update();
        assert_eq!(status_text(&mut app), "Conectado - Player 1");
    }

    #[test]
    fn hud_shows_ping_when_link_has_rtt() {
        let mut app = hud_app();
        let mut link = Link::new(None);
        link.stats.rtt = core::time::Duration::from_millis(34);
        app.world_mut().spawn((
            Client::default(),
            Connected,
            RemoteId(PeerId::Netcode(1)),
            link,
        ));
        app.update();
        assert_eq!(status_text(&mut app), "Conectado | 34ms");
    }

    #[test]
    fn format_age_humanizes() {
        assert_eq!(format_age(10), "agora");
        assert_eq!(format_age(600), "ha 10min");
        assert_eq!(format_age(7200), "ha 2h");
        assert_eq!(format_age(172800), "ha 2d");
    }
}
