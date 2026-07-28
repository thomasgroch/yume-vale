use bevy::prelude::*;

use crate::ui::{theme, widgets};

use super::GameplayPanel;
use super::snapshot::ClientQuests;

// ─── Components ────────────────────────────────────────────────────────

/// Root marker for the quest tracker panel.
#[derive(Component)]
pub struct QuestPanel;

/// Text entity showing the current quest's first objective progress.
#[derive(Component)]
pub struct ObjectiveText;

/// Text entity showing completion state + reward indicator.
#[derive(Component)]
pub struct CompletionText;

// ─── Spawn ─────────────────────────────────────────────────────────────

/// Create the quest tracker panel (hidden outside InGame via `GameplayPanel`).
pub fn spawn_quest_panel(mut commands: Commands) {
    commands
        .spawn((
            QuestPanel,
            GameplayPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(theme::SPACE_10),
                right: Val::Px(theme::SPACE_10),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme::SPACE_8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Missões"),
                widgets::text_font(theme::FONT_SM),
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new("Nenhuma missão ativa"),
                widgets::text_font(theme::FONT_XS),
                TextColor(Color::WHITE),
                ObjectiveText,
                Node {
                    margin: UiRect::left(Val::Px(theme::SPACE_4)),
                    ..default()
                },
            ));

            parent.spawn((
                Text::new(""),
                widgets::text_font(theme::FONT_XS),
                TextColor(theme::STATUS_OK),
                CompletionText,
                Node {
                    margin: UiRect::left(Val::Px(theme::SPACE_4)),
                    ..default()
                },
            ));
        });
}

// ─── Update ────────────────────────────────────────────────────────────

/// Update quest display from `ClientQuests`.
pub fn update_quest_panel(
    client_quests: Res<ClientQuests>,
    mut objective: Query<&mut Text, (With<ObjectiveText>, Without<CompletionText>)>,
    mut completion: Query<&mut Text, (With<CompletionText>, Without<ObjectiveText>)>,
) {
    let Ok(mut obj_text) = objective.single_mut() else {
        return;
    };
    let Ok(mut comp_text) = completion.single_mut() else {
        return;
    };

    let Some(quest) = client_quests.quests.first() else {
        obj_text.0 = "Nenhuma missão ativa".to_string();
        comp_text.0.clear();
        return;
    };

    if quest.completed {
        obj_text.0 = "Missão completa!".to_string();
        comp_text.0 = "\u{2713} Recompensa disponível".to_string();
        return;
    }

    if let Some(obj) = quest.progress.first() {
        obj_text.0 = format!("Progresso: {}/{}", obj.current, obj.target);
    } else {
        obj_text.0 = "Missão: sem objetivos".to_string();
    }
    comp_text.0.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_protocol::messages::{ObjectiveProgress, QuestStateData};

    fn quest_app() -> App {
        let mut app = App::new();
        app.init_resource::<ClientQuests>();
        app.add_systems(Startup, spawn_quest_panel);
        app.add_systems(Update, update_quest_panel);
        app.update();
        app
    }

    fn objective_text(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<ObjectiveText>>()
            .single(app.world())
            .map(|t| t.0.clone())
            .unwrap_or_default()
    }

    fn completion_text(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<CompletionText>>()
            .single(app.world())
            .map(|t| t.0.clone())
            .unwrap_or_default()
    }

    #[test]
    fn empty_quests_shows_no_active_mission() {
        let mut app = quest_app();
        assert_eq!(objective_text(&mut app), "Nenhuma missão ativa");
        assert!(completion_text(&mut app).is_empty());
    }

    #[test]
    fn partial_quest_shows_progress() {
        let mut app = quest_app();
        app.world_mut().resource_mut::<ClientQuests>().quests = vec![QuestStateData {
            quest_id: 1,
            completed: false,
            progress: vec![ObjectiveProgress {
                objective_index: 0,
                current: 3,
                target: 5,
            }],
        }];
        app.update();
        assert_eq!(objective_text(&mut app), "Progresso: 3/5");
        assert!(completion_text(&mut app).is_empty());
    }

    #[test]
    fn completed_quest_shows_completion_and_reward_indicator() {
        let mut app = quest_app();
        app.world_mut().resource_mut::<ClientQuests>().quests = vec![QuestStateData {
            quest_id: 1,
            completed: true,
            progress: vec![ObjectiveProgress {
                objective_index: 0,
                current: 5,
                target: 5,
            }],
        }];
        app.update();
        assert_eq!(objective_text(&mut app), "Missão completa!");
        assert!(completion_text(&mut app).contains("Recompensa"));
    }

    #[test]
    fn snapshot_update_changes_labels_exactly_once() {
        let mut app = quest_app();

        // First update: partial
        app.world_mut().resource_mut::<ClientQuests>().quests = vec![QuestStateData {
            quest_id: 1,
            completed: false,
            progress: vec![ObjectiveProgress {
                objective_index: 0,
                current: 2,
                target: 5,
            }],
        }];
        app.update();
        assert_eq!(objective_text(&mut app), "Progresso: 2/5");

        // Second update: completed
        app.world_mut().resource_mut::<ClientQuests>().quests = vec![QuestStateData {
            quest_id: 1,
            completed: true,
            progress: vec![ObjectiveProgress {
                objective_index: 0,
                current: 5,
                target: 5,
            }],
        }];
        app.update();
        assert_eq!(objective_text(&mut app), "Missão completa!");
    }
}
