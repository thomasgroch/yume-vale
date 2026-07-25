//! Diagnostic measurement for the movement-jitter report (not a pass/fail
//! regression test — prints streams for analysis with --nocapture).
//!
//! Measures, under real 30Hz physics (Tnua+Avian) and a forced run input:
//! 1. Server per-tick Transform.y (float-spring oscillation?)
//! 2. Client per-update PlayerPosition deltas (smooth interpolation or steps?)

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::prelude::*;
use core::time::Duration;
use game_core::math::Direction;
use game_protocol::{PlayerPosition, ProtocolPlugin};
use game_server::systems::{
    NextPlayerColor, ServerSystems, WalkConfig, apply_client_input, handle_new_client_link,
    on_client_connected, setup_world, sync_transform_to_position,
};
use lightyear::connection::client::Connect;
use lightyear::crossbeam::CrossbeamIo;
use lightyear::prelude::client::{ClientPlugins, RawClient};
use lightyear::prelude::server::{LinkOf, RawServer, ServerPlugins, Started};
use lightyear::prelude::*;
use player::{Player, PlayerMovement, PlayerPlugin, YumeScheme};
use std::net::{Ipv4Addr, SocketAddr};

const TICK: Duration = Duration::from_millis(16);

fn physics_server_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(ServerPlugins {
        tick_duration: TICK,
    });
    app.add_plugins((
        PhysicsPlugins::default(),
        TnuaControllerPlugin::<YumeScheme>::new(FixedUpdate),
        TnuaAvian3dPlugin::new(FixedUpdate),
    ));
    app.add_plugins((ProtocolPlugin, PlayerPlugin));
    app.init_resource::<NextPlayerColor>();
    app.init_resource::<WalkConfig>();
    app.add_observer(handle_new_client_link);
    app.add_observer(on_client_connected);
    app.add_systems(FixedUpdate, apply_client_input.in_set(ServerSystems));
    app.configure_sets(FixedUpdate, player::PlayerMovementSet.after(ServerSystems));
    app.add_systems(
        FixedUpdate,
        sync_transform_to_position.after(PhysicsSystems::Writeback),
    );
    app.add_systems(Startup, setup_world);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(TICK));
    app.finish();
    app
}

fn client_app() -> App {
    client_app_with_render_step(TICK)
}

fn client_app_with_render_step(render_step: Duration) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(ClientPlugins {
        tick_duration: TICK,
    });
    app.add_plugins((ProtocolPlugin, PlayerPlugin));
    app.add_systems(
        Update,
        seed_transforms.before(game_client::visuals::sync_position_to_transform),
    );
    app.add_systems(PostUpdate, game_client::visuals::sync_position_to_transform);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(render_step));
    app.finish();
    app
}

/// Replicated entities arrive without a Transform (the real client seeds it in
/// attach_player_visuals); give them one so we can measure the sync output.
fn seed_transforms(
    mut commands: Commands,
    query: Query<Entity, (With<PlayerPosition>, Without<Transform>)>,
) {
    for entity in &query {
        commands.entity(entity).insert(Transform::default());
    }
}

fn client_player_transform(app: &mut App) -> Option<Vec3> {
    app.world_mut()
        .query_filtered::<&Transform, With<Player>>()
        .iter(app.world())
        .next()
        .map(|t| t.translation)
}

fn connect_client(server: &mut App, client: &mut App, port: u16) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
    let (client_io, server_io) = CrossbeamIo::new_pair();
    let se = server.world_mut().spawn_empty().id();
    server.world_mut().entity_mut(se).insert(RawServer);
    server.world_mut().entity_mut(se).insert(Started);
    let lo = server
        .world_mut()
        .spawn((LinkOf { server: se }, server_io, PeerAddr(addr)))
        .id();
    server.world_mut().trigger(LinkStart { entity: lo });
    let ce = client
        .world_mut()
        .spawn((RawClient, client_io, PeerAddr(addr), ReplicationReceiver))
        .id();
    client.world_mut().trigger(Connect { entity: ce });
}

fn server_player_transform(app: &mut App) -> Option<Vec3> {
    app.world_mut()
        .query_filtered::<&Transform, With<Player>>()
        .iter(app.world())
        .next()
        .map(|t| t.translation)
}

fn client_player_position(app: &mut App) -> Option<Vec3> {
    app.world_mut()
        .query_filtered::<&PlayerPosition, With<Player>>()
        .iter(app.world())
        .next()
        .map(|p| p.0)
}

#[test]
fn measure_movement_streams() {
    let mut server = physics_server_app();
    let mut client = client_app();
    connect_client(&mut server, &mut client, 20010);

    // Wait for the replicated player on both sides.
    let mut connected = false;
    for _ in 0..400 {
        server.update();
        client.update();
        if server_player_transform(&mut server).is_some()
            && client_player_position(&mut client).is_some()
        {
            connected = true;
            break;
        }
    }
    assert!(connected, "player should replicate");

    // Force a steady run input directly on the server-side player.
    {
        let mut q = server
            .world_mut()
            .query_filtered::<&mut PlayerMovement, With<Player>>();
        let mut movement = q.single_mut(server.world_mut()).unwrap();
        movement.direction = Direction::from_xz(1.0, 0.0).unwrap();
        movement.running = true;
    }

    // Record ~3s of simulated time (16ms steps => FixedUpdate at ~30Hz).
    let mut server_series: Vec<Vec3> = Vec::new();
    let mut client_transform_series: Vec<Vec3> = Vec::new();
    for _ in 0..190 {
        server.update();
        client.update();
        server_series.push(server_player_transform(&mut server).unwrap());
        client_transform_series.push(client_player_transform(&mut client).unwrap());
    }

    // --- Debug: is the controller fed? what does physics report? ---
    {
        let mut q = server.world_mut().query_filtered::<(
            &PlayerMovement,
            &TnuaController<YumeScheme>,
            &LinearVelocity,
            &Transform,
        ), With<Player>>();
        match q.single(server.world()) {
            Ok((movement, controller, velocity, transform)) => {
                println!(
                    "DEBUG movement.dir={:?} running={} basis.desired_motion={:?} vel={:?} pos={:?}",
                    movement.direction.0,
                    movement.running,
                    controller.basis.desired_motion,
                    velocity.0,
                    transform.translation,
                );
            }
            Err(e) => println!("DEBUG introspection query failed: {e}"),
        }
    }

    // --- Analysis 4: stop -> restart cycle (the user's "any movement input") ---
    // Stop for ~1s, then run again: does the client freeze+jump again?
    {
        let mut q = server
            .world_mut()
            .query_filtered::<&mut PlayerMovement, With<Player>>();
        let mut movement = q.single_mut(server.world_mut()).unwrap();
        movement.direction = Direction::zero();
        movement.running = false;
    }
    for _ in 0..62 {
        server.update();
        client.update();
    }
    let pre_restart = client_player_position(&mut client).unwrap();
    {
        let mut q = server
            .world_mut()
            .query_filtered::<&mut PlayerMovement, With<Player>>();
        let mut movement = q.single_mut(server.world_mut()).unwrap();
        movement.direction = Direction::from_xz(0.0, 1.0).unwrap();
        movement.running = true;
    }
    let mut restart_deltas: Vec<f32> = Vec::new();
    let mut prev = pre_restart;
    for _ in 0..45 {
        server.update();
        client.update();
        let p = client_player_position(&mut client).unwrap();
        restart_deltas.push((p - prev).length());
        prev = p;
    }
    let zero = restart_deltas.iter().filter(|d| **d < 1e-6).count();
    let max_d = restart_deltas.iter().cloned().fold(0.0f32, f32::max);
    println!("RESTART after stop: zero_frames={zero}/45 max_delta={max_d:.4} expected=0.160");
    let pat: Vec<String> = restart_deltas.iter().map(|d| format!("{d:.3}")).collect();
    println!("RESTART delta pattern: {}", pat.join(" "));

    // --- Analysis 1: server Y oscillation after ramp-up (skip first 60 steps) ---
    let settled = &server_series[60..];
    let ys: Vec<f32> = settled.iter().map(|v| v.y).collect();
    let y_min = ys.iter().cloned().fold(f32::INFINITY, f32::min);
    let y_max = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let y_sign_changes = ys
        .windows(3)
        .filter(|w| {
            (w[1] - w[0]).signum() != (w[2] - w[1]).signum() && w[1] != w[0] && w[2] != w[1]
        })
        .count();
    println!(
        "SERVER Y: min={y_min:.4} max={y_max:.4} range={:.4} zigzag_windows={y_sign_changes}/{}",
        y_max - y_min,
        ys.len()
    );

    // --- Analysis 2: server per-step planar speed after ramp-up ---
    let speeds: Vec<f32> = settled
        .windows(2)
        .map(|w| {
            let d = w[1] - w[0];
            Vec3::new(d.x, 0.0, d.z).length() / TICK.as_secs_f32()
        })
        .collect();
    let nonzero: Vec<f32> = speeds.iter().cloned().filter(|s| *s > 0.01).collect();
    let mean = nonzero.iter().sum::<f32>() / nonzero.len().max(1) as f32;
    let var = nonzero.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / nonzero.len().max(1) as f32;
    println!(
        "SERVER planar speed: mean={mean:.2} std={:.2} min={:.2} max={:.2}",
        var.sqrt(),
        nonzero.iter().cloned().fold(f32::INFINITY, f32::min),
        nonzero.iter().cloned().fold(0.0f32, f32::max),
    );

    // --- Analysis 3: client Transform stream during MOVEMENT (post fix B) ---
    // The sync system snaps to newest confirmed while the interpolated
    // component is stale: expect small steps from the start instead of the
    // previous 16-frame freeze + 1.63m teleport.
    let deltas: Vec<f32> = client_transform_series
        .windows(2)
        .map(|w| (w[1] - w[0]).length())
        .collect();
    let server_speeds: Vec<f32> = server_series
        .windows(2)
        .map(|w| {
            let d = w[1] - w[0];
            Vec3::new(d.x, 0.0, d.z).length() / TICK.as_secs_f32()
        })
        .collect();

    // Window where the server is actually moving (speed > 1 m/s).
    let moving: Vec<usize> = server_speeds
        .iter()
        .enumerate()
        .filter(|(_, s)| **s > 1.0)
        .map(|(i, _)| i)
        .collect();
    println!(
        "SERVER moved during steps {}..{} (final pos {:?})",
        moving.first().unwrap_or(&0),
        moving.last().unwrap_or(&0),
        server_series.last().unwrap()
    );

    if !moving.is_empty() {
        let lo = *moving.first().unwrap();
        let hi = *moving.last().unwrap();
        let window = &deltas[lo..=hi.min(deltas.len() - 1)];
        let zero = window.iter().filter(|d| **d < 1e-6).count();
        let max_d = window.iter().cloned().fold(0.0f32, f32::max);
        let server_mean = server_speeds[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
        let expected = server_mean * TICK.as_secs_f32();
        println!(
            "CLIENT Transform during movement: zero_frames={zero}/{} max_delta={max_d:.4} expected_delta_per_step={expected:.4}",
            window.len()
        );
        let pat: Vec<String> = window.iter().map(|d| format!("{d:.3}")).collect();
        println!(
            "CLIENT Transform delta pattern (movement): {}",
            pat.join(" ")
        );
    }
}

/// Render cadence vs tick cadence: the client samples the interpolated
/// position at ~200fps (5ms) while the server ticks at 62.5Hz in-test.
/// Measures whether the per-sample speed ripples during acceleration
/// (velocity jumps at 30Hz anchor boundaries) vs steady cruise.
#[test]
fn measure_render_cadence_mismatch() {
    const RENDER: Duration = Duration::from_millis(5);

    let mut server = physics_server_app();
    let mut client = client_app_with_render_step(RENDER);
    connect_client(&mut server, &mut client, 20011);

    let mut connected = false;
    for _ in 0..400 {
        server.update();
        client.update();
        if server_player_transform(&mut server).is_some()
            && client_player_position(&mut client).is_some()
        {
            connected = true;
            break;
        }
    }
    assert!(connected, "player should replicate");

    {
        let mut q = server
            .world_mut()
            .query_filtered::<&mut PlayerMovement, With<Player>>();
        let mut movement = q.single_mut(server.world_mut()).unwrap();
        movement.direction = Direction::from_xz(1.0, 0.0).unwrap();
        movement.running = true;
    }

    let mut server_series: Vec<Vec3> = Vec::new();
    let mut client_series: Vec<Vec3> = Vec::new();
    for _ in 0..190 {
        server.update();
        server_series.push(server_player_transform(&mut server).unwrap());
        for _ in 0..3 {
            client.update();
            client_series.push(client_player_transform(&mut client).unwrap());
        }
    }

    let server_speeds: Vec<f32> = server_series
        .windows(2)
        .map(|w| (w[1] - w[0]).length() / TICK.as_secs_f32())
        .collect();
    let moving: Vec<usize> = server_speeds
        .iter()
        .enumerate()
        .filter(|(_, s)| **s > 1.0)
        .map(|(i, _)| i)
        .collect();
    let (lo, hi) = (*moving.first().unwrap(), *moving.last().unwrap());

    // Map server steps to client samples (3 samples per server step).
    let speeds: Vec<f32> = client_series
        .windows(2)
        .map(|w| (w[1] - w[0]).length() / RENDER.as_secs_f32())
        .collect();

    // Acceleration window = first half of movement, cruise = second half.
    let accel_samples: Vec<f32> = speeds[lo * 3..(lo + (hi - lo) / 2) * 3].to_vec();
    let cruise_samples: Vec<f32> = speeds[(lo + (hi - lo) / 2) * 3..hi * 3].to_vec();
    let stats = |v: &[f32]| {
        let nz: Vec<f32> = v.iter().cloned().filter(|s| *s > 0.01).collect();
        let mean = nz.iter().sum::<f32>() / nz.len().max(1) as f32;
        let var = nz.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / nz.len().max(1) as f32;
        (mean, var.sqrt())
    };
    let (am, asd) = stats(&accel_samples);
    let (cm, csd) = stats(&cruise_samples);
    println!(
        "CADENCE accel: mean={am:.2} std={asd:.2} (n={})",
        accel_samples.len()
    );
    println!(
        "CADENCE cruise: mean={cm:.2} std={csd:.2} (n={})",
        cruise_samples.len()
    );
    let pat: Vec<String> = cruise_samples[..40.min(cruise_samples.len())]
        .iter()
        .map(|s| format!("{s:.1}"))
        .collect();
    println!("CADENCE cruise speed pattern: {}", pat.join(" "));
    let apat: Vec<String> = accel_samples[..40.min(accel_samples.len())]
        .iter()
        .map(|s| format!("{s:.1}"))
        .collect();
    println!("CADENCE accel speed pattern: {}", apat.join(" "));
}
