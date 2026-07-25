use bevy::prelude::*;

use crate::config::ClientConfig;
use crate::connection::start_connection;

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppFlow {
    #[default]
    Menu,
    Playing,
}

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct PlayButton;

const MENU_BG: Color = Color::srgb(1.0, 0.90, 0.94);
const TITLE: Color = Color::srgb(0.88, 0.42, 0.58);
const SUBTLE: Color = Color::srgb(0.43, 0.50, 0.58);
const BUTTON: Color = Color::srgb(1.0, 0.56, 0.67);
const BUTTON_HOVER: Color = Color::srgb(1.0, 0.48, 0.60);

pub fn spawn_menu(mut commands: Commands) {
    let bubble = |size: f32, top: f32, left: f32, color: Color| {
        (
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(size),
                height: Val::Px(size),
                top: Val::Px(top),
                left: Val::Px(left),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(color),
        )
    };

    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(MENU_BG),
        ))
        .with_children(|root| {
            root.spawn(bubble(
                180.0,
                60.0,
                100.0,
                Color::srgba(1.0, 0.78, 0.85, 0.5),
            ));
            root.spawn(bubble(
                120.0,
                420.0,
                900.0,
                Color::srgba(0.78, 0.90, 1.0, 0.5),
            ));
            root.spawn(bubble(
                90.0,
                130.0,
                950.0,
                Color::srgba(0.80, 0.96, 0.85, 0.5),
            ));

            root.spawn((
                Text::new("Yume Vale"),
                TextFont {
                    font_size: FontSize::Px(80.0),
                    ..default()
                },
                TextColor(TITLE),
                TextShadow::default(),
            ));
            root.spawn((
                Text::new("um vale fofo para passear com amigos"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(SUBTLE),
                Node {
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
            ));
            root.spawn((
                PlayButton,
                Button,
                Node {
                    margin: UiRect::top(Val::Px(48.0)),
                    padding: UiRect::axes(Val::Px(64.0), Val::Px(16.0)),
                    border_radius: BorderRadius::all(Val::Px(999.0)),
                    ..default()
                },
                BackgroundColor(BUTTON),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Jogar"),
                    TextFont {
                        font_size: FontSize::Px(28.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((
                Text::new("WASD ou setas: mover  ·  Shift: correr  ·  Q/E: girar a câmera  ·  Espaço: pular"),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(SUBTLE),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(24.0),
                    justify_self: JustifySelf::Center,
                    ..default()
                },
            ));
        });
}

pub fn play_button(
    mut interactions: Query<&Interaction, (Changed<Interaction>, With<PlayButton>)>,
    mut commands: Commands,
    config: Res<ClientConfig>,
    mut flow: ResMut<AppFlow>,
    menus: Query<Entity, With<MenuRoot>>,
) {
    for interaction in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        *flow = AppFlow::Playing;
        for entity in &menus {
            commands.entity(entity).despawn();
        }
        start_connection(&mut commands, &config);
    }
}

type PlayButtonHover<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<PlayButton>),
>;

pub fn play_button_hover(mut buttons: PlayButtonHover) {
    for (interaction, mut color) in &mut buttons {
        *color = match interaction {
            Interaction::Pressed | Interaction::Hovered => BUTTON_HOVER.into(),
            Interaction::None => BUTTON.into(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightyear::prelude::Client;

    use crate::config::build_client_config;

    fn menu_app() -> App {
        let mut app = App::new();
        app.init_resource::<AppFlow>();
        app.insert_resource(build_client_config("127.0.0.1:5000", "player"));
        app.add_systems(Startup, spawn_menu);
        app.add_systems(Update, play_button);
        app
    }

    #[test]
    fn menu_spawns_title_and_button() {
        let mut app = menu_app();
        app.update();
        let mut texts = app.world_mut().query::<&Text>();
        let all: Vec<String> = texts.iter(app.world()).map(|t| t.0.clone()).collect();
        assert!(all.iter().any(|t| t == "Yume Vale"));
        assert!(all.iter().any(|t| t == "Jogar"));
    }

    #[test]
    fn play_starts_game_and_connection() {
        let mut app = menu_app();
        app.update();
        let mut buttons = app.world_mut().query_filtered::<Entity, With<PlayButton>>();
        let button = buttons.single(app.world()).unwrap();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();

        assert_eq!(*app.world().resource::<AppFlow>(), AppFlow::Playing);
        let mut menus = app.world_mut().query_filtered::<Entity, With<MenuRoot>>();
        assert_eq!(menus.iter(app.world()).count(), 0, "menu should be gone");
        let mut clients = app.world_mut().query_filtered::<Entity, With<Client>>();
        assert_eq!(
            clients.iter(app.world()).count(),
            1,
            "client entity should exist"
        );
    }

    #[test]
    fn menu_input_gate_blocks_gather() {
        use crate::input::{InputState, gather_input};

        let mut app = App::new();
        app.init_resource::<AppFlow>();
        app.init_resource::<InputState>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<Touches>();
        app.init_resource::<crate::camera::CameraOrbit>();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.add_systems(Update, gather_input);
        app.update();
        assert_eq!(app.world().resource::<InputState>().tick, 0);
    }
}
