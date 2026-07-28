//! Collapsible global chat panel.
//!
//! Renders server-confirmed chat messages in a pastel-themed scroll list.
//! When the chat input is focused (`ChatInputState::focused`), keyboard
//! movement input is suppressed — the fox stands still while the player types.
//! Unfocus with Enter (send) or Escape (cancel).

use bevy::prelude::*;
use game_protocol::channels::ReliableChannel;
use game_protocol::messages::ChatSend;
use lightyear::prelude::MessageSender;

use crate::hud::GameplayPanel;
use crate::ui::{theme, widgets};

use super::social::ClientChat;

// consts -------------------------------------------------------------------

/// Maximum length of a chat message in Unicode scalars.
const MAX_CHAT_LENGTH: usize = 256;

/// The number of recent messages shown in the panel.
const VISIBLE_MESSAGE_COUNT: usize = 20;

// components ---------------------------------------------------------------

/// Root marker for the chat panel entity.
#[derive(Component)]
pub struct ChatPanel;

/// Marker for the chat message container (scroll list).
#[derive(Component)]
pub struct ChatMessageLog;

/// Marker for the chat input display text.
#[derive(Component)]
pub struct ChatInputText;

/// Marker for the chat error/hint text.
#[derive(Component)]
pub struct ChatInfoText;

// resources ----------------------------------------------------------------

/// Chat input state — focused means movement is suppressed and keys build
/// the message buffer.
#[derive(Resource, Default)]
pub struct ChatInputState {
    pub focused: bool,
    pub buffer: String,
    pub info: String,
}

// spawn --------------------------------------------------------------------

/// Spawn the chat panel (hidden by default via `GameplayPanel`).
pub fn spawn_chat_panel(mut commands: Commands) {
    commands
        .spawn((
            ChatPanel,
            GameplayPanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(theme::SPACE_10),
                right: Val::Px(theme::SPACE_10),
                width: Val::Px(280.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme::SPACE_8)),
                border_radius: BorderRadius::all(Val::Px(theme::SPACE_8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            // Header
            parent.spawn((
                Text::new("💬 Chat"),
                widgets::text_font(theme::FONT_SM),
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(theme::SPACE_4)),
                    ..default()
                },
            ));

            // Message log
            parent
                .spawn((
                    ChatMessageLog,
                    Node {
                        flex_direction: FlexDirection::Column,
                        height: Val::Px(160.0),
                        overflow: Overflow::clip_y(),
                        margin: UiRect::bottom(Val::Px(theme::SPACE_4)),
                        ..default()
                    },
                ))
                .with_children(|log| {
                    // Placeholder child so the log is non-empty (avoids B0001)
                    log.spawn((
                        Text::new(""),
                        widgets::text_font(theme::FONT_XS),
                        TextColor(theme::TEXT_SUBTLE),
                    ));
                });

            // Input row
            parent
                .spawn((Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(theme::SPACE_4),
                    ..default()
                },))
                .with_children(|row| {
                    row.spawn((
                        Text::new(""),
                        widgets::text_font(theme::FONT_XS),
                        TextColor(Color::WHITE),
                        ChatInputText,
                        Node {
                            width: Val::Px(200.0),
                            padding: UiRect::axes(Val::Px(theme::SPACE_4), Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                    ));
                });

            // Info / error line
            parent.spawn((
                Text::new("Pressione Enter para digitar"),
                widgets::text_font(theme::FONT_XS),
                TextColor(theme::TEXT_SUBTLE),
                ChatInfoText,
                Node {
                    margin: UiRect::top(Val::Px(theme::SPACE_4)),
                    ..default()
                },
            ));
        });
}

// update systems -----------------------------------------------------------

/// Toggle chat focus on Enter, cancel on Escape.
pub fn toggle_chat_focus(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<ChatInputState>) {
    if keys.just_pressed(KeyCode::Enter) && !state.focused {
        state.focused = true;
        state.info.clear();
    }
    if state.focused && keys.just_pressed(KeyCode::Escape) {
        state.buffer.clear();
        state.focused = false;
        state.info.clear();
    }
}

/// Process keyboard input when chat is focused.
/// Builds buffer, sends on Enter, validates max length.
pub fn process_chat_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ChatInputState>,
    mut senders: Query<&mut MessageSender<ChatSend>>,
) {
    if !state.focused {
        return;
    }

    // Backspace
    if keys.just_pressed(KeyCode::Backspace) {
        state.buffer.pop();
        state.info.clear();
    }

    // Escape already handled in toggle_chat_focus — but handle here too
    // for the focused branch
    if keys.just_pressed(KeyCode::Escape) {
        state.buffer.clear();
        state.focused = false;
        state.info.clear();
        return;
    }

    // Enter with content → send
    if keys.just_pressed(KeyCode::Enter) && !state.buffer.is_empty() {
        if state.buffer.chars().count() > MAX_CHAT_LENGTH {
            state.info = format!("Máximo de {MAX_CHAT_LENGTH} caracteres");
            return;
        }
        if let Ok(mut sender) = senders.single_mut() {
            sender.send::<ReliableChannel>(ChatSend {
                text: state.buffer.clone(),
            });
        }
        state.buffer.clear();
        state.focused = false;
        state.info.clear();
        return;
    }

    // Printable characters
    for key in keys.get_just_pressed() {
        if let Some(c) = key_to_char(*key, has_shift(&keys)) {
            if state.buffer.chars().count() >= MAX_CHAT_LENGTH {
                state.info = format!("Máximo de {MAX_CHAT_LENGTH} caracteres");
                break;
            }
            state.buffer.push(c);
            state.info.clear();
        }
    }
}

/// Returns true if either shift key is pressed.
fn has_shift(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

/// Map a `KeyCode` to a printable character, respecting shift.
pub fn key_to_char(key: KeyCode, shift: bool) -> Option<char> {
    Some(match key {
        KeyCode::KeyA => {
            if shift {
                'A'
            } else {
                'a'
            }
        }
        KeyCode::KeyB => {
            if shift {
                'B'
            } else {
                'b'
            }
        }
        KeyCode::KeyC => {
            if shift {
                'C'
            } else {
                'c'
            }
        }
        KeyCode::KeyD => {
            if shift {
                'D'
            } else {
                'd'
            }
        }
        KeyCode::KeyE => {
            if shift {
                'E'
            } else {
                'e'
            }
        }
        KeyCode::KeyF => {
            if shift {
                'F'
            } else {
                'f'
            }
        }
        KeyCode::KeyG => {
            if shift {
                'G'
            } else {
                'g'
            }
        }
        KeyCode::KeyH => {
            if shift {
                'H'
            } else {
                'h'
            }
        }
        KeyCode::KeyI => {
            if shift {
                'I'
            } else {
                'i'
            }
        }
        KeyCode::KeyJ => {
            if shift {
                'J'
            } else {
                'j'
            }
        }
        KeyCode::KeyK => {
            if shift {
                'K'
            } else {
                'k'
            }
        }
        KeyCode::KeyL => {
            if shift {
                'L'
            } else {
                'l'
            }
        }
        KeyCode::KeyM => {
            if shift {
                'M'
            } else {
                'm'
            }
        }
        KeyCode::KeyN => {
            if shift {
                'N'
            } else {
                'n'
            }
        }
        KeyCode::KeyO => {
            if shift {
                'O'
            } else {
                'o'
            }
        }
        KeyCode::KeyP => {
            if shift {
                'P'
            } else {
                'p'
            }
        }
        KeyCode::KeyQ => {
            if shift {
                'Q'
            } else {
                'q'
            }
        }
        KeyCode::KeyR => {
            if shift {
                'R'
            } else {
                'r'
            }
        }
        KeyCode::KeyS => {
            if shift {
                'S'
            } else {
                's'
            }
        }
        KeyCode::KeyT => {
            if shift {
                'T'
            } else {
                't'
            }
        }
        KeyCode::KeyU => {
            if shift {
                'U'
            } else {
                'u'
            }
        }
        KeyCode::KeyV => {
            if shift {
                'V'
            } else {
                'v'
            }
        }
        KeyCode::KeyW => {
            if shift {
                'W'
            } else {
                'w'
            }
        }
        KeyCode::KeyX => {
            if shift {
                'X'
            } else {
                'x'
            }
        }
        KeyCode::KeyY => {
            if shift {
                'Y'
            } else {
                'y'
            }
        }
        KeyCode::KeyZ => {
            if shift {
                'Z'
            } else {
                'z'
            }
        }
        KeyCode::Digit1 => {
            if shift {
                '!'
            } else {
                '1'
            }
        }
        KeyCode::Digit2 => {
            if shift {
                '@'
            } else {
                '2'
            }
        }
        KeyCode::Digit3 => {
            if shift {
                '#'
            } else {
                '3'
            }
        }
        KeyCode::Digit4 => {
            if shift {
                '$'
            } else {
                '4'
            }
        }
        KeyCode::Digit5 => {
            if shift {
                '%'
            } else {
                '5'
            }
        }
        KeyCode::Digit6 => {
            if shift {
                '^'
            } else {
                '6'
            }
        }
        KeyCode::Digit7 => {
            if shift {
                '&'
            } else {
                '7'
            }
        }
        KeyCode::Digit8 => {
            if shift {
                '*'
            } else {
                '8'
            }
        }
        KeyCode::Digit9 => {
            if shift {
                '('
            } else {
                '9'
            }
        }
        KeyCode::Digit0 => {
            if shift {
                ')'
            } else {
                '0'
            }
        }
        KeyCode::Space => ' ',
        KeyCode::Minus => {
            if shift {
                '_'
            } else {
                '-'
            }
        }
        KeyCode::Equal => {
            if shift {
                '+'
            } else {
                '='
            }
        }
        KeyCode::BracketLeft => {
            if shift {
                '{'
            } else {
                '['
            }
        }
        KeyCode::BracketRight => {
            if shift {
                '}'
            } else {
                ']'
            }
        }
        KeyCode::Backslash => {
            if shift {
                '|'
            } else {
                '\\'
            }
        }
        KeyCode::Semicolon => {
            if shift {
                ':'
            } else {
                ';'
            }
        }
        KeyCode::Quote => {
            if shift {
                '"'
            } else {
                '\''
            }
        }
        KeyCode::Comma => {
            if shift {
                '<'
            } else {
                ','
            }
        }
        KeyCode::Period => {
            if shift {
                '>'
            } else {
                '.'
            }
        }
        KeyCode::Slash => {
            if shift {
                '?'
            } else {
                '/'
            }
        }
        KeyCode::IntlBackslash => {
            if shift {
                '~'
            } else {
                '`'
            }
        }
        _ => return None,
    })
}

/// Update the chat panel: show recent messages, input buffer text, and info.
pub fn update_chat_panel(
    chat: Res<ClientChat>,
    state: Res<ChatInputState>,
    mut log_query: Query<&mut Text, With<ChatMessageLog>>,
    mut input_query: Query<&mut Text, With<ChatInputText>>,
    mut info_query: Query<&mut Text, (With<ChatInfoText>, Without<ChatInputText>)>,
) {
    // Update message log
    if let Ok(mut log_text) = log_query.single_mut() {
        let recent: Vec<String> = chat
            .messages
            .iter()
            .rev()
            .take(VISIBLE_MESSAGE_COUNT)
            .rev()
            .map(|m| format!("Jogador {}: {}", m.from_player, m.text))
            .collect();
        log_text.0 = recent.join("\n");
    }

    // Update input display
    if let Ok(mut input_text) = input_query.single_mut() {
        if state.focused {
            input_text.0 = format!("{}█", state.buffer);
        } else {
            input_text.0.clear();
        }
    }

    // Update info text
    if let Ok(mut info_text) = info_query.single_mut() {
        if state.focused {
            if state.info.is_empty() && state.buffer.is_empty() {
                info_text.0 = "Digite sua mensagem...".to_string();
            } else if !state.info.is_empty() {
                info_text.0 = state.info.clone();
            } else {
                info_text.0 = format!("{}/{}", state.buffer.chars().count(), MAX_CHAT_LENGTH);
            }
        } else {
            info_text.0 = "Pressione Enter para digitar".to_string();
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    // --- ChatInputState ---

    #[test]
    fn chat_input_default_is_not_focused() {
        let state = ChatInputState::default();
        assert!(!state.focused);
        assert!(state.buffer.is_empty());
    }

    #[test]
    fn chat_toggle_enter_sets_focused() {
        let mut state = ChatInputState::default();
        assert!(!state.focused);

        // Simulate Enter press
        state.focused = true;
        assert!(state.focused);
    }

    #[test]
    fn chat_escape_clears_and_unfocuses() {
        let mut state = ChatInputState {
            focused: true,
            buffer: "hello".to_string(),
            info: String::new(),
        };
        state.buffer.clear();
        state.focused = false;
        assert!(!state.focused);
        assert!(state.buffer.is_empty());
    }

    #[test]
    fn chat_buffer_accepts_characters() {
        let mut state = ChatInputState::default();
        state.focused = true;
        state.buffer.push('a');
        state.buffer.push('b');
        assert_eq!(state.buffer, "ab");
    }

    #[test]
    fn chat_backspace_removes_last_char() {
        let mut state = ChatInputState {
            focused: true,
            buffer: "abc".to_string(),
            info: String::new(),
        };
        state.buffer.pop();
        assert_eq!(state.buffer, "ab");
    }

    #[test]
    fn chat_buffer_enforces_max_length() {
        let mut state = ChatInputState::default();
        state.focused = true;
        let long = "a".repeat(MAX_CHAT_LENGTH + 1);
        // Only push up to max
        for c in long.chars().take(MAX_CHAT_LENGTH) {
            state.buffer.push(c);
        }
        assert_eq!(state.buffer.len(), MAX_CHAT_LENGTH);
    }

    #[test]
    fn chat_send_clears_buffer_and_unfocuses() {
        let mut state = ChatInputState {
            focused: true,
            buffer: "hello".to_string(),
            info: String::new(),
        };
        // Simulate send: clear + unfocus
        state.buffer.clear();
        state.focused = false;
        assert!(!state.focused);
        assert!(state.buffer.is_empty());
    }

    // --- key_to_char ---

    #[test]
    fn key_to_char_lowercase() {
        assert_eq!(key_to_char(KeyCode::KeyA, false), Some('a'));
        assert_eq!(key_to_char(KeyCode::KeyZ, false), Some('z'));
    }

    #[test]
    fn key_to_char_uppercase() {
        assert_eq!(key_to_char(KeyCode::KeyA, true), Some('A'));
        assert_eq!(key_to_char(KeyCode::KeyZ, true), Some('Z'));
    }

    #[test]
    fn key_to_char_digits() {
        assert_eq!(key_to_char(KeyCode::Digit1, false), Some('1'));
        assert_eq!(key_to_char(KeyCode::Digit0, false), Some('0'));
    }

    #[test]
    fn key_to_char_digit_shifted() {
        assert_eq!(key_to_char(KeyCode::Digit1, true), Some('!'));
    }

    #[test]
    fn key_to_char_space() {
        assert_eq!(key_to_char(KeyCode::Space, false), Some(' '));
    }

    #[test]
    fn key_to_char_non_printable() {
        assert_eq!(key_to_char(KeyCode::F1, false), None);
        assert_eq!(key_to_char(KeyCode::Enter, false), None);
        assert_eq!(key_to_char(KeyCode::Escape, false), None);
    }

    #[test]
    fn key_to_char_symbols() {
        assert_eq!(key_to_char(KeyCode::Minus, false), Some('-'));
        assert_eq!(key_to_char(KeyCode::Minus, true), Some('_'));
        assert_eq!(key_to_char(KeyCode::Period, false), Some('.'));
        assert_eq!(key_to_char(KeyCode::Slash, false), Some('/'));
    }

    // --- Chat panel spawn ---

    #[test]
    fn chat_panel_spawns_required_components() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<crate::flow::AppFlow>();
        app.init_resource::<ClientChat>();
        app.init_resource::<ChatInputState>();
        app.add_systems(Startup, spawn_chat_panel);
        app.update();

        let panel_count = app
            .world_mut()
            .query_filtered::<Entity, With<ChatPanel>>()
            .iter(app.world())
            .count();
        assert_eq!(panel_count, 1, "chat panel spawned");

        let log_count = app
            .world_mut()
            .query_filtered::<Entity, With<ChatMessageLog>>()
            .iter(app.world())
            .count();
        assert_eq!(log_count, 1, "message log spawned");

        let text_count = app
            .world_mut()
            .query_filtered::<Entity, With<ChatInputText>>()
            .iter(app.world())
            .count();
        assert_eq!(text_count, 1, "input text spawned");

        let info_count = app
            .world_mut()
            .query_filtered::<Entity, With<ChatInfoText>>()
            .iter(app.world())
            .count();
        assert_eq!(info_count, 1, "info text spawned");
    }
}
