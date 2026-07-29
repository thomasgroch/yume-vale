mod support;
use support::*;

use bevy::prelude::*;
use game_core::actions::EmoteKind;
use game_protocol::EmoteIntent;
use game_protocol::channels::ReliableChannel;
use lightyear::prelude::MessageSender;
use player::Player;

fn send_emote(client: &mut App, emote: EmoteKind) {
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<EmoteIntent>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(EmoteIntent { emote });
    }
}

fn player_count(server: &mut App) -> usize {
    server
        .world_mut()
        .query::<&Player>()
        .iter(server.world())
        .count()
}

#[test]
fn identity_populates_roster() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 40100);
    send_identity_hello(&mut server, &mut client, "");
    let ok = wait_until(&mut server, &mut client, |s, _| player_count(s) >= 1);
    assert!(ok, "player should spawn");

    let roster = server
        .world()
        .resource::<social::systems::ConnectedRoster>();
    assert_eq!(roster.players.len(), 1);
}

#[test]
fn emote_wave_processed() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 40400);
    send_identity_hello(&mut server, &mut client, "");
    let ok = wait_until(&mut server, &mut client, |s, _| player_count(s) >= 1);
    assert!(ok, "player should spawn");

    send_emote(&mut client, EmoteKind::Wave);
    step(&mut server, &mut client, 20);

    let roster = server
        .world()
        .resource::<social::systems::ConnectedRoster>();
    assert_eq!(roster.players.len(), 1);
}
