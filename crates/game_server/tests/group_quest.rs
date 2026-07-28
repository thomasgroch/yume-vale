//! Integration: group exactly-once quest credit with two clients.
//!
//! Requires SocialPlugin (for PlayerGroup/PlayerClientMap) and QuestPlugin
//! (for quest tracking). Two players form a group; one collects a resource
//! that matches an active quest; both should receive progress credit.
//! A second collect does NOT double-count the group (exactly-once).

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use core::time::Duration;

use game_core::actions::ActionKind;
use game_core::id::PlayerId;
use game_core::inventory::ItemKind;
use game_core::resources::ResourceKind;
use game_core::world_config::{
    ObjectiveKind, QuestConfig, QuestObjective, QuestReward, ResourceConfig, WorldConfig,
};

use game_protocol::channels::ReliableChannel;
use game_protocol::messages::ActionIntent;
use game_protocol::{IdentityHello, PROTOCOL_ID, ProtocolPlugin};

use game_server::systems::auth;
use game_server::systems::setup::WorldConfigResource;
use game_server::systems::{
    NextPlayerColor, ServerSystems, WalkConfig, apply_client_input, handle_action_intent,
    handle_new_client_link, initialize_player_components, tick_player_cooldowns,
};

use lightyear::connection::client::Connect;
use lightyear::crossbeam::CrossbeamIo;
use lightyear::prelude::client::{ClientPlugins, RawClient};
use lightyear::prelude::server::{LinkOf, RawServer, ServerPlugins, Started};
use lightyear::prelude::*;

use player::{Player, PlayerPlugin};
use quests::QuestPlugin;
use quests::components::*;
use social::SocialPlugin;
use social::systems::SocialClientPlayer;

use std::net::{Ipv4Addr, SocketAddr};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TICK: Duration = Duration::from_millis(16);
const MAX_FRAMES: usize = 600;

// ---------------------------------------------------------------------------
// Test config: Berry nodes + a quest that requires collecting 3 Berry
// ---------------------------------------------------------------------------

fn test_world_config() -> WorldConfig {
    WorldConfig {
        resources: vec![ResourceConfig {
            id: game_core::id::ResourceId::new(1),
            kind: ResourceKind::Berry,
            count: 3,
            yield_amount: 1,
            respawn_seconds: 9999.0, // never respawn for this test
            positions: vec![
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.5, 0.0, 0.5),
                Vec3::new(0.5, 0.0, 1.5),
            ],
            model_path: "berry.glb".into(),
        }],
        creatures: vec![],
        quests: vec![QuestConfig {
            id: game_core::id::QuestId::new(1),
            title: "Berry Test".into(),
            description: "".into(),
            objectives: vec![QuestObjective {
                kind: ObjectiveKind::Collect(ResourceKind::Berry),
                target_quantity: 3,
            }],
            rewards: vec![QuestReward::Item(ItemKind::Resource(ResourceKind::Fiber))],
        }],
    }
}

// ---------------------------------------------------------------------------
// App builders
// ---------------------------------------------------------------------------

fn server_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(ServerPlugins {
        tick_duration: TICK,
    });
    let wc = test_world_config();
    app.add_plugins((
        ProtocolPlugin,
        PlayerPlugin,
        SocialPlugin,
        QuestPlugin {
            quests: wc.quests.clone(),
        },
    ));
    app.init_resource::<NextPlayerColor>();
    app.init_resource::<WalkConfig>();
    app.insert_resource(WorldConfigResource(wc));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(TICK));
    app.add_observer(handle_new_client_link);
    app.add_observer(auth::on_client_connected);
    app.add_systems(
        FixedUpdate,
        (
            auth::handle_identity_hello,
            apply_client_input,
            handle_action_intent,
            initialize_player_components,
            tick_player_cooldowns,
            resources::systems::tick_resource_respawn,
            quests::initialize_player_quests,
        )
            .in_set(ServerSystems),
    );
    app.add_systems(
        PostStartup,
        |commands: Commands, config: Res<WorldConfigResource>| {
            resources::systems::spawn_resource_nodes(commands, &config.0);
        },
    );
    app.finish();
    app
}

fn client_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(ClientPlugins {
        tick_duration: TICK,
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(TICK));
    app.finish();
    app
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn connect_and_auth(server: &mut App, client: &mut App, port: u16, token: &str) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    let (client_io, server_io) = CrossbeamIo::new_pair();
    let se = server.world_mut().spawn_empty().id();
    server
        .world_mut()
        .entity_mut(se)
        .insert((RawServer, Started));
    let _lo = server
        .world_mut()
        .spawn((LinkOf { server: se }, server_io, PeerAddr(addr)))
        .id();
    server.world_mut().trigger(LinkStart { entity: _lo });
    let ce = client
        .world_mut()
        .spawn((RawClient, client_io, PeerAddr(addr), ReplicationReceiver))
        .id();
    client.world_mut().trigger(Connect { entity: ce });

    step_all(server, client, 10);

    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<IdentityHello>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(IdentityHello {
            protocol_version: PROTOCOL_ID as u32,
            token: token.to_string(),
        });
    }
    step_all(server, client, 3);
}

fn step1(server: &mut App, _client: &mut App) {
    server.update();
}

fn step_all(server: &mut App, a: &mut App, frames: usize) {
    for _ in 0..frames {
        server.update();
        a.update();
    }
}

fn step_both(server: &mut App, a: &mut App, b: &mut App, frames: usize) {
    for _ in 0..frames {
        server.update();
        a.update();
        b.update();
    }
}

fn player_count(server: &mut App) -> usize {
    server
        .world_mut()
        .query::<&Player>()
        .iter(server.world())
        .count()
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

fn send_group_invite(client: &mut App, target: u64) {
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<game_protocol::GroupInvite>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(game_protocol::GroupInvite {
            target_player: target,
        });
    }
}

fn send_group_accept(client: &mut App) {
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<game_protocol::GroupAccept>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(game_protocol::GroupAccept);
    }
}

fn send_collect(client: &mut App, _server: &mut App, sequence: u64, target_id: u64) {
    let mut query = client
        .world_mut()
        .query::<&mut MessageSender<ActionIntent>>();
    if let Some(mut sender) = query.iter_mut(client.world_mut()).next() {
        sender.send::<ReliableChannel>(ActionIntent {
            sequence,
            kind: ActionKind::Collect,
            target_id: Some(target_id),
        });
    }
}

fn quest_progress_for_player(server: &mut App, pid: PlayerId) -> Option<u32> {
    let mut query = server.world_mut().query::<(&Player, &PlayerQuests)>();
    for (player, quests) in query.iter(server.world()) {
        if player.id == pid {
            return quests.quests.first().map(|q| q.current);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Two clients form a group via invite/accept. One collects Berry.
/// Both should have quest progress > 0.
#[test]
fn group_member_gets_quest_credit() {
    let mut server = server_app();
    let mut ca = client_app();
    let mut cb = client_app();

    connect_and_auth(&mut server, &mut ca, 52001, "gqa_a");
    connect_and_auth(&mut server, &mut cb, 52002, "gqa_b");

    // Wait for both players
    let ok = {
        let mut i = 0;
        loop {
            if player_count(&mut server) >= 2 {
                break true;
            }
            if i >= MAX_FRAMES {
                break false;
            }
            step1(&mut server, &mut ca);
            i += 1;
        }
    };
    assert!(ok, "both players should spawn");

    // Enable quests on both players
    step_all(&mut server, &mut ca, 10);
    step_all(&mut server, &mut cb, 10);

    let pid_b = get_player_id(&mut server, 1);

    // A invites B, B accepts
    send_group_invite(&mut ca, pid_b.get());
    step_both(&mut server, &mut ca, &mut cb, 15);

    send_group_accept(&mut cb);
    step_both(&mut server, &mut ca, &mut cb, 15);

    // Verify both have quest initialised at 0
    let pid_a = get_player_id(&mut server, 0);
    let prog_before_a = quest_progress_for_player(&mut server, pid_a).unwrap_or(0);
    assert_eq!(prog_before_a, 0, "A quest starts at 0");

    // A collects Berry (node 0) — triggers quest credit for both group members
    send_collect(&mut ca, &mut server, 1, 0);
    step_both(&mut server, &mut ca, &mut cb, 10);
    step_both(&mut server, &mut ca, &mut cb, 10);

    let prog_a = quest_progress_for_player(&mut server, pid_a).unwrap_or(0);
    let prog_b = quest_progress_for_player(&mut server, pid_b).unwrap_or(0);

    assert!(
        prog_a >= 1,
        "collecting player (A) should have quest progress >= 1, got {prog_a}"
    );
    assert!(
        prog_b >= 1,
        "group member (B) should have quest progress >= 1, got {prog_b}"
    );
}

/// A second collection does NOT double-count the group credit.
/// Both members should reach exactly 3 progress with 3 total collections
/// (not 6, which would indicate double-credit).
#[test]
fn group_exactly_once_credit() {
    let mut server = server_app();
    let mut ca = client_app();
    let mut cb = client_app();

    connect_and_auth(&mut server, &mut ca, 52101, "geoc_a");
    connect_and_auth(&mut server, &mut cb, 52102, "geoc_b");

    let ok = {
        let mut i = 0;
        loop {
            if player_count(&mut server) >= 2 {
                break true;
            }
            if i >= MAX_FRAMES {
                break false;
            }
            step1(&mut server, &mut ca);
            i += 1;
        }
    };
    assert!(ok, "both players should spawn");

    step_all(&mut server, &mut ca, 10);
    step_all(&mut server, &mut cb, 10);

    let pid_a = get_player_id(&mut server, 0);
    let pid_b = get_player_id(&mut server, 1);

    // Form group
    send_group_invite(&mut ca, pid_b.get());
    step_both(&mut server, &mut ca, &mut cb, 15);
    send_group_accept(&mut cb);
    step_both(&mut server, &mut ca, &mut cb, 15);

    // Collect 3 times — progress should be 3 for both
    // (exactly-once means each collect credits both, but no double-count
    // for a single collect event).
    // Wait for cooldown (0.5s ≈ 32 ticks at 16ms each) between collects.
    for i in 0..3 {
        // Wait >500ms for cooldown to expire before each collect
        step_both(&mut server, &mut ca, &mut cb, 50);
        send_collect(&mut ca, &mut server, (i + 1) as u64, i as u64);
        step_both(&mut server, &mut ca, &mut cb, 10);
    }
    step_both(&mut server, &mut ca, &mut cb, 10);

    let prog_a = quest_progress_for_player(&mut server, pid_a).unwrap_or(0);
    let prog_b = quest_progress_for_player(&mut server, pid_b).unwrap_or(0);

    assert_eq!(
        prog_a, 3,
        "A should have exactly 3 quest progress after 3 collects, got {prog_a}"
    );
    assert_eq!(
        prog_b, 3,
        "B should have exactly 3 quest progress after 3 collects, got {prog_b}"
    );
}
