use bevy::prelude::*;

use crate::config::ClientConfig;
use crate::connection::{TransportState, start_connection};
use crate::flow::AppFlow;
use crate::ui::{theme, widgets};

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct PlayButton;

pub fn spawn_menu(mut commands: Commands) {
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
            BackgroundColor(theme::SURFACE_MENU),
        ))
        .with_children(|root| {
            root.spawn(widgets::bubble(180.0, 60.0, 100.0, theme::BUBBLE_PINK));
            root.spawn(widgets::bubble(120.0, 420.0, 900.0, theme::BUBBLE_BLUE));
            root.spawn(widgets::bubble(90.0, 130.0, 950.0, theme::BUBBLE_GREEN));

            root.spawn(widgets::text_style(theme::FONT_TITLE, theme::TEXT_TITLE))
                .insert(Text::new("Yume Vale"));
            root.spawn((
                Text::new("um vale fofo para passear com amigos"),
                widgets::text_font(theme::FONT_LG),
                TextColor(theme::TEXT_SUBTLE),
                Node {
                    margin: UiRect::top(Val::Px(theme::SPACE_8)),
                    ..default()
                },
            ));
            root.spawn((
                PlayButton,
                Button,
                Node {
                    margin: UiRect::top(Val::Px(theme::SPACE_48)),
                    ..widgets::button_frame(theme::SPACE_64, theme::SPACE_16)
                },
                BackgroundColor(theme::BUTTON_PRIMARY),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Jogar"),
                    widgets::text_font(theme::FONT_XL),
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((
                Text::new("WASD ou setas: mover  |  Shift: correr  |  Q/E: girar a câmera  |  Espaço: pular"),
                widgets::text_font(theme::FONT_SM),
                TextColor(theme::TEXT_SUBTLE),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(theme::SPACE_24),
                    justify_self: JustifySelf::Center,
                    ..default()
                },
            ));
        });
}

pub(crate) fn play_button(
    mut interactions: Query<&Interaction, (Changed<Interaction>, With<PlayButton>)>,
    mut commands: Commands,
    config: Res<ClientConfig>,
    mut transport: ResMut<TransportState>,
    time: Res<Time>,
    mut next_state: ResMut<NextState<AppFlow>>,
    menus: Query<Entity, With<MenuRoot>>,
) {
    for interaction in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        next_state.set(AppFlow::InGame);
        for entity in &menus {
            commands.entity(entity).despawn();
        }
        start_connection(
            &mut commands,
            &config,
            &mut transport,
            time.elapsed_secs_f64(),
        );
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
        *color = theme::button_interaction_color(interaction).into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightyear::prelude::Client;

    use crate::config::build_client_config;

    fn menu_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        app.insert_resource(build_client_config("127.0.0.1:5000", "player"));
        app.init_resource::<Time>();
        app.insert_resource(TransportState::detect());
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
        // Start in Menu state so play_button processes presses.
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::Menu);
        app.update(); // apply Menu state (OnEnter systems)
        app.update(); // actually get into Menu via StateTransition
        let mut buttons = app.world_mut().query_filtered::<Entity, With<PlayButton>>();
        let button = buttons.single(app.world()).unwrap();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update(); // play_button runs, sets NextState(InGame), spawns client
        app.update(); // StateTransition to InGame

        assert_eq!(
            app.world().resource::<State<AppFlow>>().get(),
            &AppFlow::InGame
        );
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
        use crate::prediction::InputHistory;

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppFlow>();
        // Start in Menu state (input should be blocked there).
        app.world_mut()
            .resource_mut::<NextState<AppFlow>>()
            .set(AppFlow::Menu);
        app.update(); // start state transition
        app.update(); // actually enter Menu
        app.init_resource::<InputState>();
        app.init_resource::<InputHistory>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<Touches>();
        app.init_resource::<crate::touch::TouchJump>();
        app.init_resource::<crate::camera::CameraOrbit>();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.add_systems(Update, gather_input);
        app.update();
        assert_eq!(app.world().resource::<InputState>().tick, 0);
    }
}
