use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::prelude::*;
use game_core::constants::GROUND_Y;
use game_core::id::PlayerId;
use game_protocol::channels::ReliableChannel;
use game_protocol::{PlayerColor, Welcome};
use lightyear::connection::client_of::ClientOf;
use lightyear::connection::network_target::NetworkTarget;
use lightyear::prelude::*;
use player::{YumeScheme, spawn_player};
use tracing::info;

use crate::config::ServerConfig;
use crate::systems::setup::WalkConfig;

/// Maps a client entity (LinkOf) to its player entity.
#[derive(Component)]
pub struct ClientPlayer {
    pub player_entity: Entity,
    pub player_id: PlayerId,
}

/// Wraps server config for Bevy resource access.
#[derive(Resource, Clone)]
pub struct ServerConfigResource(pub ServerConfig);

/// Round-robin counter for assigning distinct `PlayerColor` palette indices.
#[derive(Resource, Default)]
pub struct NextPlayerColor(pub u8);

/// System set for server game logic.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerSystems;

/// Adds `ReplicationSender` to new client link entities.
pub fn handle_new_client_link(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(ReplicationSender);
}

/// Spawns a player entity linked to the newly connected client,
/// assigns a round-robin color, and marks it for replication.
#[allow(clippy::type_complexity)]
pub fn on_client_connected(
    trigger: On<Add, Connected>,
    mut commands: Commands,
    mut client_query: Query<(&RemoteId, Has<ClientOf>, &mut MessageSender<Welcome>)>,
    mut next_color: ResMut<NextPlayerColor>,
    existing_players: Query<(Entity, &player::Player)>,
    walk_config: Res<WalkConfig>,
) {
    let client_entity = trigger.entity;
    let Ok((remote_id, _is_client, mut welcome_sender)) = client_query.get_mut(client_entity)
    else {
        return;
    };
    let client_id = match remote_id.0 {
        PeerId::Netcode(id) => id,
        // Non-netcode transports (raw, steam, local) identify via their PeerId bits
        _ => remote_id.0.to_bits(),
    };
    let player_id = PlayerId::new(client_id);

    // A reconnecting client may still have a stale player entity from a
    // previous session (e.g. the old link has not timed out yet); remove it so
    // the world never holds two players with the same id.
    for (entity, player) in existing_players.iter() {
        if player.id == player_id {
            info!("Despawning stale player {player_id} from previous session");
            commands.entity(entity).try_despawn();
        }
    }

    let color = PlayerColor(next_color.0);
    next_color.0 = next_color.0.wrapping_add(1);
    let player_name = format!("Player {}", color.0 + 1);
    let player_entity = spawn_player(
        &mut commands,
        player_id,
        player_name.clone(),
        Vec3::new(0.0, GROUND_Y, 0.0),
    );
    commands.entity(client_entity).insert(ClientPlayer {
        player_entity,
        player_id,
    });
    info!(
        "Player {player_id} connected with color {} (client entity {client_entity:?})",
        color.0
    );

    welcome_sender.send::<ReliableChannel>(Welcome { player_id });

    commands.entity(player_entity).insert((
        color,
        Replicate::to_clients(NetworkTarget::All),
        InterpolationTarget::to_clients(NetworkTarget::All),
        ControlledBy {
            owner: client_entity,
            lifetime: Lifetime::SessionBased,
        },
    ));

    commands.entity(player_entity).insert((
        RigidBody::Dynamic,
        Collider::capsule(0.35, 0.5),
        LockedAxes::ROTATION_LOCKED,
        TnuaAvian3dSensorShape(Collider::cylinder(0.34, 0.0)),
        TnuaController::<YumeScheme>::default(),
        TnuaConfig::<YumeScheme>(walk_config.0.clone()),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_test_app;

    #[test]
    fn on_client_connected_observer_adds_replication_components() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);
        app.add_observer(handle_new_client_link);

        let remote_id = RemoteId(PeerId::Netcode(42));
        let client_entity = app.world_mut().spawn((Connected, remote_id, ClientOf)).id();

        app.world_mut().run_schedule(FixedUpdate);

        let client_player = app.world().get::<ClientPlayer>(client_entity);
        assert!(
            client_player.is_some(),
            "ClientPlayer should have been added"
        );
        if let Some(cp) = client_player {
            assert_eq!(cp.player_id, PlayerId::new(42));
            let player_entity = cp.player_entity;
            assert!(
                app.world().get::<Replicate>(player_entity).is_some(),
                "Player should have Replicate"
            );
            assert!(
                app.world()
                    .get::<InterpolationTarget>(player_entity)
                    .is_some(),
                "Player should have InterpolationTarget"
            );
            let controlled_by = app.world().get::<ControlledBy>(player_entity);
            assert!(
                controlled_by.is_some_and(|c| c.owner == client_entity),
                "Player should have ControlledBy owned by the client link"
            );
            assert_eq!(
                app.world().get::<PlayerColor>(player_entity),
                Some(&PlayerColor(0)),
                "First player should get palette index 0"
            );
        }
    }

    #[test]
    fn on_client_connected_assigns_distinct_colors() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);
        app.add_observer(handle_new_client_link);

        let mut colors = Vec::new();
        for id in [42_u64, 43] {
            let client_entity = app
                .world_mut()
                .spawn((Connected, RemoteId(PeerId::Netcode(id)), ClientOf))
                .id();
            app.world_mut().run_schedule(FixedUpdate);
            let cp = app.world().get::<ClientPlayer>(client_entity).unwrap();
            colors.push(*app.world().get::<PlayerColor>(cp.player_entity).unwrap());
        }

        assert_eq!(colors, vec![PlayerColor(0), PlayerColor(1)]);
    }

    #[test]
    fn on_client_connected_replaces_stale_player_with_same_id() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);
        app.add_observer(handle_new_client_link);

        let first_client = app
            .world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(42)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);
        let first_player = app
            .world()
            .get::<ClientPlayer>(first_client)
            .unwrap()
            .player_entity;

        // Same client_id reconnects on a new link before the old one expired
        let second_client = app
            .world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(42)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);
        let second_player = app
            .world()
            .get::<ClientPlayer>(second_client)
            .unwrap()
            .player_entity;

        assert!(
            app.world().get_entity(first_player).is_err(),
            "stale player should be despawned"
        );
        let matching: Vec<_> = app
            .world_mut()
            .query::<&player::Player>()
            .iter(app.world())
            .filter(|p| p.id == PlayerId::new(42))
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "exactly one player with the id should remain"
        );
        assert_ne!(first_player, second_player);
    }
}
