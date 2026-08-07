use crate::channels;
use crate::components::*;
use crate::messages;
use avian3d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::prelude::*;
use core::time::Duration;
use lightyear::avian3d::types;
use lightyear::input::prelude::InputConfig;
use lightyear::prelude::input::native;
use lightyear::prelude::*;

fn lerp_position(start: Position, end: Position, t: f32) -> Position {
    types::position::lerp(&start, &end, t)
}

fn lerp_rotation(start: Rotation, end: Rotation, t: f32) -> Rotation {
    types::rotation::lerp(&start, &end, t)
}

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

        app.add_plugins(native::InputPlugin::<MovementInput> {
            config: InputConfig {
                packet_redundancy: 5,
                send_interval: Duration::default(),
                ..default()
            },
        });

        // ── Input channel messages (ClientToServer only) ──────────────────
        app.register_message::<messages::ClientInput>()
            .add_direction(NetworkDirection::ClientToServer);

        // ── Reliable messages: ClientToServer ─────────────────────────────
        app.register_message::<messages::IdentityHello>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::ActionIntent>()
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

        app.register_message::<messages::ActionRejected>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<messages::InventorySnapshot>()
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

        app.component::<Position>()
            .replicate()
            .predict()
            .add_correction_fn::<Position>(lerp_position)
            .into_component_registration()
            .add_interpolation_with(lerp_position);
        app.component::<Rotation>()
            .replicate()
            .predict()
            .add_correction_fn::<Rotation>(lerp_rotation)
            .into_component_registration()
            .add_interpolation_with(lerp_rotation);
        app.component::<LinearVelocity>().replicate().predict();
        app.component::<AngularVelocity>().replicate().predict();
    }
}
