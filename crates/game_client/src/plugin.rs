use bevy::prelude::*;
use core::net::SocketAddr;
use core::time::Duration;
use game_core::constants::TICK_RATE_HZ;
use game_protocol::{PRIVATE_KEY, PROTOCOL_ID, ProtocolPlugin, Welcome};
use lightyear::connection::client::Connect;
use lightyear::netcode::{auth::Authentication, client_plugin::NetcodeConfig};
use lightyear::prelude::client::{ClientPlugins, NetcodeClient};
use lightyear::prelude::*;
use player::{LocalPlayer, PlayerPlugin};

#[cfg(target_arch = "wasm32")]
use lightyear::prelude::client::WebTransportClientIo;

use crate::camera::{
    CameraOrbit, follow_local_player, rotate_camera_input, spawn_camera, spawn_ground,
};
use crate::config::ClientConfig;
use crate::decorations::spawn_decorations;
use crate::input::{InputState, gather_input};
use crate::snapshot::LocalPlayerId;

#[derive(Default)]
pub struct ClientPlugin {
    pub config: ClientConfig,
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        let tick_duration = Duration::from_secs_f64(1.0 / TICK_RATE_HZ as f64);
        app.add_plugins(ClientPlugins { tick_duration });
        app.add_plugins((ProtocolPlugin, PlayerPlugin));

        app.insert_resource(self.config.clone());
        app.init_resource::<InputState>();
        app.init_resource::<LocalPlayerId>();
        app.init_resource::<CameraOrbit>();

        app.add_systems(
            Startup,
            (spawn_camera, spawn_ground, spawn_decorations, setup_client),
        );

        app.add_systems(
            Update,
            (
                handle_welcome,
                attach_player_visuals,
                mark_local_player_visuals,
                gather_input,
                rotate_camera_input,
            ),
        );
        app.add_systems(
            PostUpdate,
            (sync_position_to_transform, follow_local_player).chain(),
        );
    }
}

/// Unique netcode client id per instance: the server drops connection requests
/// with an already-connected id (anti-spoofing). On native, `YUME_CLIENT_ID` env
/// overrides for tests. On wasm, a random id is generated via getrandom.
fn derive_client_id() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        client_id_from_env(std::env::var("YUME_CLIENT_ID").ok().as_deref())
            .unwrap_or_else(time_based_client_id)
    }
    #[cfg(target_arch = "wasm32")]
    {
        random_client_id()
    }
}

/// Parses `YUME_CLIENT_ID` env override. Returns `None` if absent or invalid.
/// Only used on native (env vars unavailable on wasm).
#[cfg(not(target_arch = "wasm32"))]
fn client_id_from_env(raw: Option<&str>) -> Option<u64> {
    raw.and_then(|s| s.parse::<u64>().ok()).map(|id| id.max(1))
}

/// Native: time + process-id based client id (not entropy-safe, but unique per
/// local process instance — good enough for dev).
#[cfg(not(target_arch = "wasm32"))]
fn time_based_client_id() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x59c3_7a6e);
    (nanos ^ ((std::process::id() as u64) << 32)).max(1)
}

/// Wasm: random client id via getrandom (SystemTime and process::id unavailable).
#[cfg(target_arch = "wasm32")]
fn random_client_id() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).expect("getrandom failed to generate client id");
    u64::from_le_bytes(buf).max(1)
}

fn setup_client(mut commands: Commands, config: Res<ClientConfig>) {
    let addr: SocketAddr = config
        .server_addr
        .parse()
        .expect("invalid server address in ClientConfig");

    let client_id = derive_client_id();

    let auth = Authentication::Manual {
        server_addr: addr,
        client_id,
        private_key: PRIVATE_KEY,
        protocol_id: PROTOCOL_ID,
    };

    let netcode_config = NetcodeConfig::default();
    let client = NetcodeClient::new(auth, netcode_config).expect("failed to create NetcodeClient");

    #[cfg(not(target_arch = "wasm32"))]
    let entity = commands
        .spawn((
            Client::default(),
            LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
            PeerAddr(addr),
            Link::new(None),
            client,
            UdpIo::default(),
            ReplicationReceiver,
        ))
        .id();

    // Wasm: use WebTransport by default (Phase 2 will add ?transport=ws selection)
    #[cfg(target_arch = "wasm32")]
    let entity = commands
        .spawn((
            Client::default(),
            LocalAddr(SocketAddr::from(([0, 0, 0, 0], 0))),
            PeerAddr(addr),
            Link::new(None),
            client,
            WebTransportClientIo {
                certificate_digest: String::new(),
            },
            ReplicationReceiver,
        ))
        .id();

    commands.entity(entity).trigger(|e| Connect { entity: e });
}

fn handle_welcome(
    mut receivers: Query<&mut MessageReceiver<Welcome>>,
    mut local_id: ResMut<LocalPlayerId>,
) {
    for mut receiver in receivers.iter_mut() {
        for welcome in receiver.receive() {
            local_id.id = Some(welcome.player_id);
        }
    }
}

type UnmeshedInterpolatedPlayers<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static player::Player,
        &'static game_protocol::PlayerColor,
    ),
    (With<Interpolated>, Without<Mesh3d>),
>;

/// Attaches mesh visuals to any player entity that arrives via Lightyear
/// replication (marked with `Interpolated`) but does not yet have a `Mesh3d`.
/// Color comes from the server-assigned `PlayerColor` so every client renders
/// the same player identically; if it has not replicated yet, the entity is
/// retried on later frames via the `Without<Mesh3d>` filter.
fn attach_player_visuals(
    mut commands: Commands,
    local_id: Res<LocalPlayerId>,
    players: UnmeshedInterpolatedPlayers,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, player, color) in players.iter() {
        let is_local = local_id.id == Some(player.id);
        info!(
            "player visuals attached for {:?} (local={}, color={})",
            player.id, is_local, color.0
        );

        let base = game_protocol::palette_color(color.0);

        let mut entity_cmds = commands.entity(entity);
        entity_cmds.insert((
            Mesh3d(meshes.add(Capsule3d::new(0.4, 1.2))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: base.into(),
                metallic: 0.1,
                perceptual_roughness: 0.8,
                emissive: LinearRgba::from(base) * 0.3,
                ..Default::default()
            })),
        ));

        if is_local {
            entity_cmds.insert(LocalPlayer);
        }
    }
}

type UnmarkedLocalPlayers<'w, 's> =
    Query<'w, 's, (Entity, &'static player::Player), (With<Interpolated>, Without<LocalPlayer>)>;

/// Retroactively marks the local player entity once `LocalPlayerId` is known,
/// in case the entity replicated before the Welcome message arrived.
fn mark_local_player_visuals(
    mut commands: Commands,
    local_id: Res<LocalPlayerId>,
    players: UnmarkedLocalPlayers,
) {
    let Some(my_id) = local_id.id else {
        return;
    };
    for (entity, player) in players.iter() {
        if player.id == my_id {
            commands.entity(entity).insert(LocalPlayer);
        }
    }
}

/// Copies `PlayerPosition` → `Transform.translation` for interpolated entities.
/// Runs in PostUpdate, chained before `follow_local_player`.
fn sync_position_to_transform(
    mut query: Query<(&game_protocol::PlayerPosition, &mut Transform), With<Interpolated>>,
) {
    for (pos, mut transform) in query.iter_mut() {
        transform.translation = pos.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::id::PlayerId;

    #[test]
    fn default_plugin_has_default_config() {
        let plugin = ClientPlugin::default();
        assert_eq!(plugin.config.server_addr, "127.0.0.1:5000");
    }

    #[test]
    fn custom_plugin_config() {
        let plugin = ClientPlugin {
            config: ClientConfig {
                server_addr: "192.168.1.100:8080".into(),
                player_name: "Yume".into(),
            },
        };
        assert_eq!(plugin.config.server_addr, "192.168.1.100:8080");
    }

    #[test]
    fn client_config_defaults() {
        let cfg = ClientConfig::default();
        assert_eq!(cfg.server_addr, "127.0.0.1:5000");
        assert_eq!(cfg.player_name, "Player");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn client_id_from_env_parses_valid_id() {
        assert_eq!(client_id_from_env(Some("42")), Some(42));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn client_id_from_env_rejects_zero() {
        assert_eq!(client_id_from_env(Some("0")), Some(1));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn client_id_from_env_ignores_garbage() {
        assert_eq!(client_id_from_env(Some("not-a-number")), None);
        assert_eq!(client_id_from_env(None), None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn time_based_client_id_is_nonzero_and_unique() {
        let a = time_based_client_id();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let b = time_based_client_id();
        assert_ne!(a, 0);
        assert_ne!(b, 0);
        assert_ne!(a, b, "two client instances must not share a netcode id");
    }

    #[test]
    fn sync_position_to_transform_copies_translation() {
        use game_protocol::PlayerPosition;

        let mut app = App::new();
        app.add_systems(Update, sync_position_to_transform);

        let entity = app
            .world_mut()
            .spawn((
                PlayerPosition(Vec3::new(10.0, 0.0, 20.0)),
                Transform::from_translation(Vec3::ZERO),
                Interpolated,
            ))
            .id();
        app.update();

        let t = app.world().get::<Transform>(entity).unwrap();
        assert!((t.translation.x - 10.0).abs() < 1e-5);
        assert!((t.translation.z - 20.0).abs() < 1e-5);
    }

    #[test]
    fn attach_player_visuals_adds_mesh_to_interpolated() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.init_resource::<LocalPlayerId>();
        app.add_systems(Update, attach_player_visuals);

        // Non-local player (no LocalPlayerId match)
        let remote_id = local_player_id(Some(PlayerId::new(1)));
        app.world_mut().insert_resource(remote_id);

        let entity = app
            .world_mut()
            .spawn((
                player::Player {
                    id: PlayerId::new(2),
                },
                game_protocol::PlayerColor(3),
                Interpolated,
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Mesh3d>(entity).is_some(),
            "remote player should get a mesh"
        );
        assert!(
            app.world().get::<LocalPlayer>(entity).is_none(),
            "non-local player should not get LocalPlayer marker"
        );

        let material = app.world().get::<MeshMaterial3d<StandardMaterial>>(entity);
        let handle = material.expect("player should have a material");
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let mat = materials.get(&handle.0).unwrap();
        let expected: Color = game_protocol::palette_color(3).into();
        assert_eq!(
            mat.base_color, expected,
            "color must come from the replicated PlayerColor"
        );
    }

    #[test]
    fn attach_player_visuals_marks_local_player() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.init_resource::<LocalPlayerId>();
        app.add_systems(Update, attach_player_visuals);

        // Local player (LocalPlayerId matches)
        let local_id = local_player_id(Some(PlayerId::new(1)));
        app.world_mut().insert_resource(local_id);

        let entity = app
            .world_mut()
            .spawn((
                player::Player {
                    id: PlayerId::new(1),
                },
                game_protocol::PlayerColor(0),
                Interpolated,
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Mesh3d>(entity).is_some(),
            "local player should get a mesh"
        );
        assert!(
            app.world().get::<LocalPlayer>(entity).is_some(),
            "local player should get LocalPlayer marker"
        );
    }

    #[test]
    fn attach_player_visuals_waits_for_player_color() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.init_resource::<LocalPlayerId>();
        app.add_systems(Update, attach_player_visuals);

        let entity = app
            .world_mut()
            .spawn((
                player::Player {
                    id: PlayerId::new(2),
                },
                Interpolated,
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Mesh3d>(entity).is_none(),
            "mesh must wait for the replicated PlayerColor"
        );

        app.world_mut()
            .entity_mut(entity)
            .insert(game_protocol::PlayerColor(5));
        app.update();

        assert!(
            app.world().get::<Mesh3d>(entity).is_some(),
            "mesh attaches once PlayerColor arrives"
        );
    }

    fn local_player_id(id: Option<PlayerId>) -> LocalPlayerId {
        LocalPlayerId { id }
    }
}
