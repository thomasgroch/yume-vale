use bevy::prelude::*;
use game_protocol::RejectionKind;
use lightyear::prelude::Client;

use crate::connection::{IdentityToken, TransportState, clear_identity_token};
use crate::flow::AppFlow;
use crate::ui::{theme, widgets};

#[derive(Component)]
pub(crate) struct RejectionModal;

#[derive(Component)]
pub(crate) struct RejectionDismiss;

fn rejection_copy(reason: RejectionKind) -> (&'static str, &'static str) {
    match reason {
        RejectionKind::ServerFull => (
            "Servidor cheio",
            "Não há vagas agora. Tente novamente em instantes.",
        ),
        RejectionKind::ProtocolMismatch => (
            "Atualização necessária",
            "Recarregue a página para usar a versão mais recente.",
        ),
        RejectionKind::InvalidIdentity => (
            "Sessão inválida",
            "Sua identidade local será renovada na próxima conexão.",
        ),
    }
}

pub(crate) fn sync_rejection_modal(
    mut commands: Commands,
    transport: Res<TransportState>,
    existing: Query<Entity, With<RejectionModal>>,
) {
    let Some(reason) = transport.rejection_reason else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    let (title, body) = rejection_copy(reason);
    commands
        .spawn((
            RejectionModal,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.06, 0.09, 0.72)),
            GlobalZIndex(100),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Percent(86.0),
                        max_width: Val::Px(420.0),
                        padding: UiRect::all(Val::Px(theme::SPACE_24)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(theme::SPACE_16),
                        border_radius: BorderRadius::all(Val::Px(theme::SPACE_16)),
                        ..default()
                    },
                    BackgroundColor(theme::SURFACE_MENU),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new(title),
                        widgets::text_font(theme::FONT_XL),
                        TextColor(theme::TEXT_TITLE),
                    ));
                    card.spawn((
                        Text::new(body),
                        widgets::text_font(theme::FONT_MD),
                        TextColor(theme::TEXT_SUBTLE),
                        TextLayout::justify(Justify::Center),
                    ));
                    card.spawn((
                        RejectionDismiss,
                        Button,
                        Node {
                            min_height: Val::Px(theme::MIN_TOUCH_TARGET),
                            ..widgets::button_frame(theme::SPACE_24, theme::SPACE_10)
                        },
                        BackgroundColor(theme::BUTTON_PRIMARY),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Voltar"),
                            widgets::text_font(theme::FONT_MD),
                            TextColor(Color::WHITE),
                        ));
                    });
                });
        });
}

pub(crate) fn dismiss_rejection_modal(
    mut commands: Commands,
    interactions: Query<&Interaction, (Changed<Interaction>, With<RejectionDismiss>)>,
    modals: Query<Entity, With<RejectionModal>>,
    clients: Query<Entity, With<Client>>,
    mut transport: ResMut<TransportState>,
    mut identity: ResMut<IdentityToken>,
    mut next_state: ResMut<NextState<AppFlow>>,
) {
    if !interactions
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        return;
    }
    if transport.rejection_reason == Some(RejectionKind::InvalidIdentity) {
        identity.0.clear();
        clear_identity_token();
    }
    for entity in &clients {
        commands.entity(entity).despawn();
    }
    for entity in &modals {
        commands.entity(entity).despawn();
    }
    transport.reset_rejection();
    next_state.set(AppFlow::Menu);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    #[test]
    fn rejection_copy_is_localized_for_every_reason() {
        for reason in [
            RejectionKind::ServerFull,
            RejectionKind::ProtocolMismatch,
            RejectionKind::InvalidIdentity,
        ] {
            let (title, body) = rejection_copy(reason);
            assert!(!title.is_empty());
            assert!(!body.is_empty());
        }
    }

    #[test]
    fn modal_spawns_once_for_rejection() {
        let mut app = App::new();
        let mut transport = TransportState::default();
        transport.reject(RejectionKind::ServerFull);
        app.insert_resource(transport);
        app.add_systems(Update, sync_rejection_modal);

        app.update();
        app.update();

        let count = app
            .world_mut()
            .query_filtered::<Entity, With<RejectionModal>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn dismiss_resets_rejection_and_removes_modal_and_client() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<AppFlow>();
        app.insert_resource(IdentityToken("keep-token".into()));
        let mut transport = TransportState::default();
        transport.reject(RejectionKind::ServerFull);
        app.insert_resource(transport);
        app.add_systems(Update, dismiss_rejection_modal);

        app.world_mut().spawn(RejectionModal);
        app.world_mut()
            .spawn((RejectionDismiss, Interaction::Pressed));
        app.world_mut().spawn(Client::default());
        app.update();

        {
            let transport = app.world().resource::<TransportState>();
            assert!(!transport.rejection_received);
            assert!(transport.rejection_reason.is_none());
        }
        assert_eq!(app.world().resource::<IdentityToken>().0, "keep-token");
        let world = app.world_mut();
        let modal_count = world.query::<&RejectionModal>().iter(world).count();
        let client_count = world.query::<&Client>().iter(world).count();
        assert_eq!(modal_count, 0);
        assert_eq!(client_count, 0);
    }
}
