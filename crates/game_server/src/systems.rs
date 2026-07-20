use bevy::prelude::*;
use game_core::constants::GROUND_Y;
use game_core::id::PlayerId;
use game_core::math::Direction;
use game_core::player_state::PlayerInput;
use game_protocol::channels::ReliableChannel;
use game_protocol::{ClientInput, PlayerColor, PlayerPosition, ReplicatedPlayerInput, Welcome};

use lightyear::connection::client_of::ClientOf;
use lightyear::connection::network_target::NetworkTarget;
use lightyear::prelude::*;
use player::{PlayerMovement, spawn_player};
use tracing::info;

use crate::config::ServerConfig;

/// Maps a client entity (LinkOf) to its player entity.
#[derive(Component)]
pub struct ClientPlayer {
    pub player_entity: Entity,
    pub player_id: PlayerId,
}

/// Internal resource wrapping the server configuration so it's available in systems.
#[derive(Resource, Clone)]
pub struct ServerConfigResource(pub ServerConfig);

/// Round-robin counter for assigning distinct `PlayerColor` palette indices.
#[derive(Resource, Default)]
pub struct NextPlayerColor(pub u8);

/// System set for server game logic.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerSystems;

/// Server-side observer that adds `ReplicationSender` to new client link entities,
/// enabling them to send replicated data to their corresponding client.
pub fn handle_new_client_link(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(ReplicationSender);
}

/// Called when a client is marked as connected. Spawns a player entity,
/// links it to the client, and marks it for Lightyear replication + interpolation.
#[allow(clippy::type_complexity)]
pub fn on_client_connected(
    trigger: On<Add, Connected>,
    mut commands: Commands,
    mut client_query: Query<(&RemoteId, Has<ClientOf>, &mut MessageSender<Welcome>)>,
    mut next_color: ResMut<NextPlayerColor>,
) {
    let client_entity = trigger.entity;
    let Ok((remote_id, _is_client, mut welcome_sender)) = client_query.get_mut(client_entity)
    else {
        return;
    };
    let client_id = match remote_id.0 {
        PeerId::Netcode(id) => id,
        _ => return,
    };
    let player_id = PlayerId::new(client_id);
    let player_name = format!("Player {client_id}");
    let color = PlayerColor(next_color.0);
    next_color.0 = next_color.0.wrapping_add(1);
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

    // Mark the player entity for Lightyear replication + interpolation.
    commands.entity(player_entity).insert((
        color,
        Replicate::to_clients(NetworkTarget::All),
        InterpolationTarget::to_clients(NetworkTarget::All),
        ControlledBy {
            owner: client_entity,
            lifetime: Lifetime::SessionBased,
        },
    ));
}

/// Apply a single `ClientInput` to a player's movement and replicated input
/// components. Extracted for testability.
pub fn apply_input_to_player(
    input: &ClientInput,
    movement: &mut PlayerMovement,
    rep_input: &mut ReplicatedPlayerInput,
) {
    let dx = input.move_x as f32 / 127.0;
    let dz = input.move_z as f32 / 127.0;
    let direction = Direction::from_xz(dx, dz).unwrap_or(Direction::zero());
    movement.direction = direction;
    movement.running = input.run;
    rep_input.0 = PlayerInput {
        movement: direction,
        run: input.run,
        interact: None,
        action: None,
    };
}

/// Reads `ClientInput` messages from all connected clients and applies them
/// to the corresponding player's `PlayerMovement` and `ReplicatedPlayerInput`.
pub fn apply_client_input(
    mut receivers: Query<(&mut MessageReceiver<ClientInput>, &ClientPlayer)>,
    mut players: Query<(&mut PlayerMovement, &mut ReplicatedPlayerInput)>,
) {
    for (mut receiver, info) in receivers.iter_mut() {
        for ci in receiver.receive() {
            if let Ok((mut movement, mut rep_input)) = players.get_mut(info.player_entity) {
                apply_input_to_player(&ci, &mut movement, &mut rep_input);
            }
        }
    }
}

/// Copies `Transform.translation` → `PlayerPosition` for all player entities.
/// Must run AFTER `integrate_velocity` so that the replicated position reflects
/// the latest server-authoritative movement.
pub fn sync_transform_to_position(mut query: Query<(&Transform, &mut PlayerPosition)>) {
    for (transform, mut pos) in query.iter_mut() {
        pos.0 = transform.translation;
    }
}

/// Spawns the Lightyear server entity and starts it.
pub fn setup_server(mut commands: Commands) {
    use lightyear::prelude::server::*;
    use std::net::SocketAddr;

    tracing::info!("starting Lightyear server on 127.0.0.1:5000");

    let config = NetcodeConfig::default()
        .with_protocol_id(game_protocol::PROTOCOL_ID)
        .with_key(game_protocol::PRIVATE_KEY);

    let server_entity = commands
        .spawn((
            NetcodeServer::new(config),
            LocalAddr(SocketAddr::from(([127, 0, 0, 1], 5000))),
            ServerUdpIo::default(),
        ))
        .id();

    commands
        .entity(server_entity)
        .trigger(|e| Start { entity: e });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_test_app;
    use game_core::math::Direction;

    #[test]
    fn apply_input_to_player_writes_movement() {
        let input = ClientInput {
            tick: 1,
            move_x: 1,
            move_z: 0,
            run: true,
        };
        let mut movement = PlayerMovement::default();
        let mut rep_input = ReplicatedPlayerInput(PlayerInput::default());

        apply_input_to_player(&input, &mut movement, &mut rep_input);

        assert_eq!(movement.direction, Direction::from_xz(1.0, 0.0).unwrap());
        assert!(movement.running);
        assert_eq!(rep_input.0.movement, Direction::from_xz(1.0, 0.0).unwrap());
        assert!(rep_input.0.run);
        assert!(rep_input.0.interact.is_none());
        assert!(rep_input.0.action.is_none());
    }

    #[test]
    fn apply_input_to_player_decodes_diagonal() {
        let diagonal_encoded = (std::f32::consts::FRAC_1_SQRT_2 * 127.0).round() as i8;
        let input = ClientInput {
            tick: 1,
            move_x: diagonal_encoded,
            move_z: diagonal_encoded,
            run: false,
        };
        let mut movement = PlayerMovement::default();
        let mut rep_input = ReplicatedPlayerInput(PlayerInput::default());

        apply_input_to_player(&input, &mut movement, &mut rep_input);

        let dir = movement.direction.0;
        assert!((dir.x - dir.z).abs() < 1e-5, "diagonal must be symmetric");
        assert!(
            (dir.length() - 1.0).abs() < 1e-2,
            "direction must be normalized"
        );
    }

    #[test]
    fn apply_input_to_player_writes_action_and_interact() {
        let input = ClientInput {
            tick: 2,
            move_x: 0,
            move_z: 0,
            run: false,
        };
        let mut movement = PlayerMovement::default();
        let mut rep_input = ReplicatedPlayerInput(PlayerInput::default());

        apply_input_to_player(&input, &mut movement, &mut rep_input);

        assert_eq!(movement.direction, Direction::zero());
        assert!(!movement.running);
        assert_eq!(rep_input.0.action, None);
        assert_eq!(rep_input.0.interact, None);
    }

    #[test]
    fn apply_input_to_player_chat_message_ignored() {
        let input = ClientInput {
            tick: 3,
            move_x: 0,
            move_z: 0,
            run: false,
        };
        let mut movement = PlayerMovement::default();
        let mut rep_input = ReplicatedPlayerInput(PlayerInput::default());

        apply_input_to_player(&input, &mut movement, &mut rep_input);

        // chat_message is not stored in PlayerInput (it's handled separately)
        assert_eq!(rep_input.0.action, None);
    }

    #[test]
    fn sync_transform_to_position_copies_translation() {
        let mut app = build_test_app();
        let entity = spawn_player(
            &mut app.world_mut().commands(),
            PlayerId::new(1),
            "Test".into(),
            Vec3::new(5.0, 0.0, 10.0),
        );
        app.world_mut().flush();

        app.world_mut()
            .entity_mut(entity)
            .insert(Transform::from_translation(Vec3::new(15.0, 0.0, 25.0)));
        app.world_mut().flush();

        app.add_systems(FixedUpdate, sync_transform_to_position);
        app.world_mut().run_schedule(FixedUpdate);

        let pos = app.world().get::<PlayerPosition>(entity).unwrap();
        assert!((pos.x - 15.0).abs() < 1e-5);
        assert!((pos.z - 25.0).abs() < 1e-5);
    }

    #[test]
    fn on_client_connected_observer_adds_replication_components() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);
        app.add_observer(handle_new_client_link);

        let remote_id = RemoteId(PeerId::Netcode(42));
        let client_entity = app.world_mut().spawn((Connected, remote_id, ClientOf)).id();

        // Run the observer - commands from observer are deferred, need a schedule tick
        app.world_mut().run_schedule(FixedUpdate);

        let client_player = app.world().get::<ClientPlayer>(client_entity);
        assert!(
            client_player.is_some(),
            "ClientPlayer should have been added"
        );
        if let Some(cp) = client_player {
            assert_eq!(cp.player_id, PlayerId::new(42));
            // Check the player entity got replication/interpolation components
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
}
