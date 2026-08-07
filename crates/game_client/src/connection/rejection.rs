use bevy::prelude::*;
use game_protocol::ConnectionRejected;
use lightyear::prelude::MessageReceiver;

use super::transport_fallback::TransportState;

pub(crate) fn handle_connection_rejected(
    mut receivers: Query<&mut MessageReceiver<ConnectionRejected>>,
    mut transport: ResMut<TransportState>,
) {
    for mut receiver in &mut receivers {
        for rejection in receiver.receive() {
            transport.reject(rejection.reason);
            warn!("connection rejected: {:?}", rejection.reason);
        }
    }
}
