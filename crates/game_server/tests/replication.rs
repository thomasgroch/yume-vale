//! Integration tests: replication / interpolation markers.

mod support;
use support::*;

use bevy::prelude::*;
use lightyear::prelude::*;
use player::Player;

#[test]
fn replicated_player_has_interpolated_marker() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20004);
    send_identity_hello(&mut server, &mut client, "");

    let ok = wait_until(&mut server, &mut client, |_s, c| {
        c.world_mut()
            .query_filtered::<Entity, (With<Player>, With<Interpolated>)>()
            .iter(c.world())
            .count()
            >= 1
    });
    assert!(
        ok,
        "client player entity should have the Interpolated marker"
    );
}
