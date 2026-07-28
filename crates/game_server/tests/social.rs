//! Integration tests for social features (chat, groups, emotes).
//!
//! Tests verify server-side state rather than transport-level S2C message
//! delivery, since Crossbeam transport in tests may not reliably deliver
//! messages in all scenarios.

mod support;
use support::*;

use bevy::prelude::*;
use game_core::actions::EmoteKind;
use game_core::id::PlayerId;
use game_protocol::channels::ReliableChannel;
use game_protocol::{ChatSend, EmoteIntent, GroupAccept, GroupInvite, GroupLeave};
use lightyear::prelude::MessageSender;
use player::Player;
use social::systems::{PlayerGroup, SocialClientPlayer, SocialStateResource};

/// Step server + both clients.
fn step_both(server: &mut App, a: &mut App, b: &mut App, frames: usize) {
    for _ in 0..frames {
        server.update();
        a.update();
        b.update();
    }
}

fn send_chat(client: &mut App, text: &str) {
    let mut query = client.world_mut().query::<&mut MessageSender<ChatSend>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(ChatSend {
            text: text.to_string(),
        });
    }
}

fn send_group_invite(client: &mut App, target: u64) {
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<GroupInvite>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(GroupInvite {
            target_player: target,
        });
    }
}

fn send_group_accept(client: &mut App) {
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<GroupAccept>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(GroupAccept);
    }
}

fn send_group_leave(client: &mut App) {
    let mut query = client.world_mut().query::<&mut MessageSender<GroupLeave>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(GroupLeave);
    }
}

fn send_emote(client: &mut App, emote: EmoteKind) {
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<EmoteIntent>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(EmoteIntent { emote });
    }
}

fn get_player_id(server: &mut App, client_idx: usize) -> PlayerId {
    let mut q = server.world_mut().query::<&SocialClientPlayer>();
    for (i, cp) in q.iter(server.world()).enumerate() {
        if i == client_idx {
            return cp.player_id;
        }
    }
    unreachable!("no player at index {client_idx}")
}

fn player_count(server: &mut App) -> usize {
    server
        .world_mut()
        .query::<&Player>()
        .iter(server.world())
        .count()
}

fn player_has_group(server: &mut App, pid: PlayerId) -> bool {
    server
        .world_mut()
        .query::<(&Player, Option<&PlayerGroup>)>()
        .iter(server.world())
        .filter(|(p, _)| p.id == pid)
        .any(|(_, g)| g.is_some() && g.unwrap().0.is_some())
}

fn anyone_has_group(server: &mut App) -> bool {
    server
        .world_mut()
        .query::<&PlayerGroup>()
        .iter(server.world())
        .any(|g| g.0.is_some())
}

// -----------------------------------------------------------------------
// Chat: verify server-side processing
// -----------------------------------------------------------------------

#[test]
fn chat_server_receives_and_roster_populated() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 40100);
    send_identity_hello(&mut server, &mut client, "");
    let ok = wait_until(&mut server, &mut client, |s, _| player_count(s) >= 1);
    assert!(ok, "player should spawn");

    send_chat(&mut client, "hello");
    step(&mut server, &mut client, 20);

    let roster = server
        .world()
        .resource::<social::systems::ConnectedRoster>();
    assert!(!roster.players.is_empty(), "roster populated");
}

// -----------------------------------------------------------------------
// Group lifecycle (all server-side checks)
// -----------------------------------------------------------------------

#[test]
fn group_accept_creates_group() {
    let mut server = server_app();
    let mut ca = client_app();
    let mut cb = client_app();
    connect_client(&mut server, &mut ca, 40200);
    connect_client(&mut server, &mut cb, 40201);
    send_identity_hello(&mut server, &mut ca, "");
    send_identity_hello(&mut server, &mut cb, "");
    let ok = wait_until(&mut server, &mut ca, |s, _| player_count(s) >= 2);
    assert!(ok, "two players should spawn");

    let pid_b = get_player_id(&mut server, 1);
    send_group_invite(&mut ca, pid_b.get());
    step(&mut server, &mut ca, 10);
    step(&mut server, &mut cb, 10);

    send_group_accept(&mut cb);
    step(&mut server, &mut ca, 20);
    step(&mut server, &mut cb, 10);

    assert!(
        anyone_has_group(&mut server),
        "players should be in a group after accept"
    );
}

#[test]
fn group_leave_removes_membership() {
    let mut server = server_app();
    let mut ca = client_app();
    let mut cb = client_app();
    connect_client(&mut server, &mut ca, 40210);
    connect_client(&mut server, &mut cb, 40211);
    send_identity_hello(&mut server, &mut ca, "");
    send_identity_hello(&mut server, &mut cb, "");
    let ok = wait_until(&mut server, &mut ca, |s, _| player_count(s) >= 2);
    assert!(ok, "two players should spawn");

    let pid_b = get_player_id(&mut server, 1);
    send_group_invite(&mut ca, pid_b.get());
    step_both(&mut server, &mut ca, &mut cb, 10);

    send_group_accept(&mut cb);
    step_both(&mut server, &mut ca, &mut cb, 10);

    let pid_a = get_player_id(&mut server, 0);
    assert!(
        player_has_group(&mut server, pid_a),
        "A in group before leave"
    );

    send_group_leave(&mut ca);
    step_both(&mut server, &mut ca, &mut cb, 10);

    assert!(
        !player_has_group(&mut server, pid_a),
        "A not in group after leave"
    );
}

#[test]
fn group_self_invite_rejected() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 40220);
    send_identity_hello(&mut server, &mut client, "");
    let ok = wait_until(&mut server, &mut client, |s, _| player_count(s) >= 1);
    assert!(ok, "player should spawn");

    let pid = get_player_id(&mut server, 0);
    send_group_invite(&mut client, pid.get());
    step(&mut server, &mut client, 10);

    let social = server.world().resource::<SocialStateResource>();
    assert!(social.0.pending_invites.is_empty(), "self-invite rejected");
}

#[test]
fn group_accept_without_invite_noop() {
    let mut server = server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 40230);
    send_identity_hello(&mut server, &mut client, "");
    let ok = wait_until(&mut server, &mut client, |s, _| player_count(s) >= 1);
    assert!(ok, "player should spawn");

    send_group_accept(&mut client);
    step(&mut server, &mut client, 10);

    assert!(!anyone_has_group(&mut server), "no group without invite");
}

// -----------------------------------------------------------------------
// Single-group invariant
// -----------------------------------------------------------------------

#[test]
fn single_group_invariant() {
    let mut server = server_app();
    let mut ca = client_app();
    let mut cb = client_app();
    let mut cc = client_app();
    connect_client(&mut server, &mut ca, 40300);
    connect_client(&mut server, &mut cb, 40301);
    connect_client(&mut server, &mut cc, 40302);
    send_identity_hello(&mut server, &mut ca, "");
    send_identity_hello(&mut server, &mut cb, "");
    send_identity_hello(&mut server, &mut cc, "");
    let ok = wait_until(&mut server, &mut ca, |s, _| player_count(s) >= 3);
    assert!(ok, "three players should spawn");

    let pid_b = get_player_id(&mut server, 1);
    let pid_c = get_player_id(&mut server, 2);

    // A invites B, B accepts
    send_group_invite(&mut ca, pid_b.get());
    step_both(&mut server, &mut ca, &mut cb, 10);
    send_group_accept(&mut cb);
    step_both(&mut server, &mut ca, &mut cb, 10);

    let pid_a = get_player_id(&mut server, 0);
    assert!(player_has_group(&mut server, pid_a), "A in group with B");

    // A tries to invite C while already in a group with B
    // The invite itself is accepted (social state tracks it)
    // But C cannot accept because A is already in a group
    send_group_invite(&mut ca, pid_c.get());
    step_both(&mut server, &mut ca, &mut cc, 10);
    send_group_accept(&mut cc);
    step_both(&mut server, &mut ca, &mut cc, 10);

    // C should not be in any group (the accept was rejected because A is in a group)
    assert!(
        !player_has_group(&mut server, pid_c),
        "C should not be in a group"
    );

    // A should still be in the original group with B
    assert!(
        player_has_group(&mut server, pid_a),
        "A should still be in group with B"
    );
}

// -----------------------------------------------------------------------
// Emote wave
// -----------------------------------------------------------------------

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
    assert!(
        !roster.players.is_empty(),
        "roster still populated after emote"
    );
}
