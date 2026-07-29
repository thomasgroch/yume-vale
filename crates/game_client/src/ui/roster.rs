use bevy::prelude::*;
use player::Player;

use crate::hud::GameplayPanel;
use crate::ui::{theme, widgets};

#[derive(Component)]
pub struct RosterPanel;

#[derive(Component)]
pub struct RosterPlayersText;

pub fn spawn_roster_panel(mut commands: Commands) {
    commands
        .spawn((
            RosterPanel,
            GameplayPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(theme::SPACE_48),
                left: Val::Px(theme::SPACE_10),
                width: Val::Px(200.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme::SPACE_8)),
                border_radius: BorderRadius::all(Val::Px(theme::SPACE_8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Jogadores"),
                widgets::text_font(theme::FONT_SM),
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(theme::SPACE_4)),
                    ..default()
                },
            ));
            parent.spawn((
                Text::new("Conectados: 0"),
                widgets::text_font(theme::FONT_XS),
                TextColor(theme::TEXT_SUBTLE),
                RosterPlayersText,
            ));
        });
}

pub fn update_roster_panel(
    players: Query<(), With<Player>>,
    mut text: Query<&mut Text, With<RosterPlayersText>>,
) {
    if let Ok(mut text) = text.single_mut() {
        text.0 = format!("Conectados: {}", players.iter().count());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::id::PlayerId;

    #[test]
    fn roster_counts_replicated_players() {
        let mut app = App::new();
        app.add_systems(Update, update_roster_panel);
        app.world_mut().spawn((Text::default(), RosterPlayersText));
        app.world_mut().spawn(Player {
            id: PlayerId::new(1),
        });
        app.world_mut().spawn(Player {
            id: PlayerId::new(2),
        });

        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&Text, With<RosterPlayersText>>();
        assert_eq!(query.single(app.world()).unwrap().0, "Conectados: 2");
    }
}
