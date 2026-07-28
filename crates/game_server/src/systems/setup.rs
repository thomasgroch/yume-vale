use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::builtins::{TnuaBuiltinJumpConfig, TnuaBuiltinWalkConfig};
use game_core::arena::{ArenaColliderShape, arena_layout};
use game_core::constants::RUN_SPEED;
use game_core::decorations::{DecorationKind, decoration_layout};
use lightyear::prelude::*;
use player::scheme::YumeSchemeConfig;
// WorldConfigResource is defined below in this module

use super::connection::ServerConfigResource;

/// Shared handle to the Tnua walk configuration (speed = `RUN_SPEED`; walking
/// is fed as a fraction of it via `desired_motion`).
#[derive(Resource, Clone)]
pub struct WalkConfig(pub Handle<YumeSchemeConfig>);

impl FromWorld for WalkConfig {
    fn from_world(world: &mut World) -> Self {
        world.init_resource::<Assets<YumeSchemeConfig>>();
        let mut configs = world.resource_mut::<Assets<YumeSchemeConfig>>();
        Self(configs.add(YumeSchemeConfig {
            basis: TnuaBuiltinWalkConfig {
                speed: RUN_SPEED,
                float_height: 0.6,
                acceleration: 40.0,
                air_acceleration: 10.0,
                ..default()
            },
            jump: TnuaBuiltinJumpConfig {
                height: 1.5,
                ..default()
            },
        }))
    }
}

/// Wraps a `WorldConfig` as a Bevy resource.
#[derive(Resource, Clone)]
pub struct WorldConfigResource(pub game_core::world_config::WorldConfig);

/// Spawns the static physics world: infinite ground plane at y=0, arena prop
/// colliders from `arena_layout()`, and decoration colliders from
/// `decoration_layout()` (tree trunks and boulders; flowers are visual-only).
/// Also spawns creatures from the world config.
pub fn setup_world(
    mut commands: Commands,
    world_config: Option<Res<WorldConfigResource>>,
    creature_query: Query<Entity, Added<creatures::Creature>>,
) {
    commands.spawn((RigidBody::Static, Collider::half_space(Vec3::Y)));

    // Spawn creatures from world config
    if let Some(config) = world_config {
        creatures::spawn_creatures(&mut commands, &config.0);
    }

    // Add physics components to creatures spawned above.
    // The query won't see them until next frame, but that's fine — the
    // creatures plugin adds its own velocity system, and we add colliders
    // here so that Avian sees them before the first physics tick.
    for entity in &creature_query {
        commands.entity(entity).insert((
            RigidBody::Dynamic,
            Collider::sphere(0.5),
            LockedAxes::ROTATION_LOCKED,
        ));
    }

    for prop in arena_layout() {
        let rot = Quat::from_rotation_y(prop.yaw);
        for collider in prop.colliders {
            let world_pos = prop.translation + rot * collider.offset;
            let shape = match collider.shape {
                ArenaColliderShape::Cuboid { half_extents } => {
                    Collider::cuboid(half_extents.x, half_extents.y, half_extents.z)
                }
                ArenaColliderShape::Cylinder {
                    radius,
                    half_height,
                } => Collider::cylinder(radius, half_height),
            };
            commands.spawn((
                RigidBody::Static,
                shape,
                Transform::from_translation(world_pos).with_rotation(rot),
            ));
        }
    }

    for prop in decoration_layout() {
        let (x, z) = (prop.position.x, prop.position.z);
        match prop.kind {
            DecorationKind::Tree => {
                commands.spawn((
                    RigidBody::Static,
                    Collider::cylinder(0.25, 0.8),
                    Transform::from_translation(Vec3::new(x, 0.8, z)),
                ));
            }
            DecorationKind::Rock(s) => {
                commands.spawn((
                    RigidBody::Static,
                    Collider::sphere(0.6 * s),
                    Transform::from_translation(Vec3::new(x, 0.3 * s, z)),
                ));
            }
            DecorationKind::Flower => {}
        }
    }
}

/// Spawns the Lightyear server entities (UDP, WebTransport, WebSocket) and starts them.
pub fn setup_server(
    mut commands: Commands,
    server_config: Res<ServerConfigResource>,
    tls_identity: Option<Res<super::tls::TlsIdentity>>,
) {
    use lightyear::prelude::server::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    let cfg = &server_config.0;
    let host: IpAddr = cfg.host.parse().unwrap_or_else(|_| {
        tracing::warn!("invalid server host {:?}, binding 0.0.0.0", cfg.host);
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    });

    let config = NetcodeConfig::default()
        .with_protocol_id(game_protocol::PROTOCOL_ID)
        .with_key(game_protocol::PRIVATE_KEY);

    // UDP / Netcode listener (existing native transport)
    let udp_entity = commands
        .spawn((
            NetcodeServer::new(config.clone()),
            LocalAddr(SocketAddr::new(host, cfg.port)),
            ServerUdpIo::default(),
        ))
        .id();
    commands.entity(udp_entity).trigger(|e| Start { entity: e });

    // WebTransport listener (browser clients)
    let wt_identity = match &tls_identity {
        Some(id) => id.identity.clone_identity(),
        None => {
            tracing::warn!(
                "no TLS identity resource — generating self-signed \
                 (client hash pinning will not work)"
            );
            lightyear::webtransport::prelude::Identity::self_signed([
                "localhost",
                "127.0.0.1",
                "::1",
            ])
            .expect("self-signed WT identity")
        }
    };
    let wt_entity = commands
        .spawn((
            NetcodeServer::new(config.clone()),
            LocalAddr(SocketAddr::new(host, cfg.web_transport_port)),
            WebTransportServerIo {
                certificate: wt_identity,
            },
        ))
        .id();
    commands.entity(wt_entity).trigger(|e| Start { entity: e });

    // WebSocket listener (browser clients, fallback)
    let ws_config = aeronet_websocket::server::ServerConfig::builder()
        .with_bind_address(SocketAddr::new(host, cfg.websocket_port))
        .with_no_encryption();
    let ws_entity = commands
        .spawn((
            NetcodeServer::new(config),
            LocalAddr(SocketAddr::new(host, cfg.websocket_port)),
            WebSocketServerIo { config: ws_config },
        ))
        .id();
    commands.entity(ws_entity).trigger(|e| Start { entity: e });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Total collider count from all arena props.
    fn prop_collider_count() -> usize {
        arena_layout().iter().map(|p| p.colliders.len()).sum()
    }

    /// Decoration colliders: one per tree and boulder (flowers excluded).
    fn decoration_collider_count() -> usize {
        decoration_layout()
            .iter()
            .filter(|p| p.kind != DecorationKind::Flower)
            .count()
    }

    #[test]
    fn setup_world_spawns_ground_and_prop_colliders() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Startup, setup_world);
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<(), (With<RigidBody>, With<Collider>)>();
        let count = query.iter(app.world()).count();

        // 1 ground plane + all prop colliders + all decoration colliders
        assert_eq!(
            count,
            1 + prop_collider_count() + decoration_collider_count()
        );
    }

    #[test]
    fn wall_collider_at_slot_one_position() {
        let layout = arena_layout();
        // Slot 1 is a wall (slot 0 is portal)
        let wall_prop = &layout[1];
        assert_eq!(wall_prop.model, game_core::arena::ArenaModel::Wall);
        let rot = Quat::from_rotation_y(wall_prop.yaw);
        let expected_pos = wall_prop.translation + rot * wall_prop.colliders[0].offset;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Startup, setup_world);
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&Transform, (With<RigidBody>, With<Collider>)>();
        let found = query
            .iter(app.world())
            .any(|t| (t.translation - expected_pos).length() < 0.01);

        assert!(
            found,
            "wall collider at slot 1 should exist at ({expected_pos})",
        );
    }
}
