//! Integration tests: replication / interpolation markers.

mod support;
use support::*;

use bevy::prelude::*;
use lightyear::prelude::*;
use player::Player;

#[test]
fn replicated_player_has_prediction_marker() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20005);
    send_identity_hello(&mut server, &mut client, "");

    // The single test client is the owning client (server uses its PeerAddr as the
    // PredictionTarget), so it receives the player entity with the Predicted marker.
    let ok = wait_until(&mut server, &mut client, |_s, c| {
        c.world_mut()
            .query_filtered::<Entity, (With<Player>, With<Predicted>)>()
            .iter(c.world())
            .count()
            >= 1
    });
    assert!(ok, "client player entity should have the Predicted marker");
}
