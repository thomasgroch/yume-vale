//! Integration tests: multiple clients / color assignment.

mod support;
use support::*;

use bevy::prelude::*;
use game_protocol::PlayerColor;

#[test]
fn two_clients_get_distinct_colors_and_see_each_other() {
    let mut server = server_app();
    let mut client1 = client_app();
    let mut client2 = client_app();
    connect_client(&mut server, &mut client1, 20002);
    send_identity_hello(&mut server, &mut client1, "");
    connect_client(&mut server, &mut client2, 20003);
    send_identity_hello(&mut server, &mut client2, "");

    let ok = wait_until(&mut server, &mut client1, |s, _c| {
        server_player_count(s) == 2
    });
    assert!(ok, "server should have 2 players");

    let colors: Vec<PlayerColor> = server
        .world_mut()
        .query::<&PlayerColor>()
        .iter(server.world())
        .copied()
        .collect();
    assert!(colors.contains(&PlayerColor(0)));
    assert!(colors.contains(&PlayerColor(1)));
    assert_eq!(colors.len(), 2);

    wait_until(&mut server, &mut client1, |s, c| {
        step(s, c, 1); // keep client2 alive
        client_has_player(c)
    });
    wait_until(&mut server, &mut client2, |s, c| {
        step(s, c, 1); // keep client1 alive
        client_has_player(c)
    });

    // Both players should now be replicated to both clients.
    // Give a few more steps for the second player to replicate.
    step(&mut server, &mut client1, 20);
    step(&mut server, &mut client2, 20);

    for (i, client) in [&mut client1, &mut client2].iter_mut().enumerate() {
        let client_colors: Vec<PlayerColor> = client
            .world_mut()
            .query::<&PlayerColor>()
            .iter(client.world())
            .copied()
            .collect();
        assert_eq!(client_colors.len(), 2, "client {i} sees 2 players");
        assert!(client_colors.contains(&PlayerColor(0)));
        assert!(client_colors.contains(&PlayerColor(1)));
    }
}
