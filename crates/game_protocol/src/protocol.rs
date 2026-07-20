use crate::PlayerPosition;
use crate::channels;
use crate::messages;
use bevy::prelude::*;
use core::time::Duration;
use lightyear::prelude::*;

pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
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

        app.register_message::<messages::ClientInput>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<messages::Welcome>()
            .add_direction(NetworkDirection::ServerToClient);

        app.component::<PlayerPosition>()
            .replicate()
            .add_linear_interpolation();

        app.component::<crate::PlayerColor>().replicate();
    }
}
