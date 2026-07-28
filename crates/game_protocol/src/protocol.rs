use crate::channels;
use crate::components::*;
use crate::messages;
use bevy::prelude::*;
use core::time::Duration;
use lightyear::prelude::*;

pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        // ── Channels ──────────────────────────────────────────────────────
        app.add_channel::<channels::InputChannel>(ChannelSettings {
            mode: ChannelMode::SequencedUnreliable,
            send_frequency: Duration::from_secs_f64(1.0 / 30.0),
            priority: 2.0,
        })
        .add_direction(NetworkDirection::ClientToServer);

        app.add_channel::<channels::ReliableChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            send_frequency: Duration::default(),
            priority: 2.0,
        })
        .add_direction(NetworkDirection::Bidirectional);

        // ── Input channel messages (ClientToServer only) ──────────────────
        app.register_message::<messages::ClientInput>()
            .add_direction(NetworkDirection::ClientToServer);

        // ── Reliable messages: ClientToServer ─────────────────────────────
        app.register_message::<messages::IdentityHello>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::ActionIntent>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::ChatSend>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::GroupInvite>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::GroupAccept>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::GroupDecline>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::GroupLeave>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::EmoteIntent>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::PlotBuildIntent>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::PlotRemoveIntent>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::EmoteBroadcast>()
            .add_direction(NetworkDirection::ServerToClient);

        // ── Reliable messages: ServerToClient ─────────────────────────────
        app.register_message::<messages::Welcome>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::ConnectionRejected>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::ChatReceived>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::GroupUpdate>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::InputAck>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::ActionRejected>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::InventorySnapshot>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::QuestSnapshot>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::BondSnapshot>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::PlotSnapshot>()
            .add_direction(NetworkDirection::ServerToClient);

        // ── Replicated components ─────────────────────────────────────────
        app.component::<PlayerPosition>()
            .replicate()
            .add_linear_interpolation();

        app.component::<PlayerColor>().replicate();

        app.component::<ResourceNodeState>()
            .replicate()
            .add_linear_interpolation();

        app.component::<CreatureState>()
            .replicate()
            .add_linear_interpolation();

        app.component::<DecorationState>().replicate();
    }
}
