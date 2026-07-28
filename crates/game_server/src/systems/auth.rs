//! Authentication flow: pending sessions, identity resolution, player spawn.
//!
//! ## Flow
//! 1. `Connected` observer creates a [`PendingSession`] — no player spawned yet.
//! 2. Client sends `IdentityHello { protocol_version, token }`.
//! 3. `handle_identity_hello` system processes the message:
//!    - Validates protocol version
//!    - Checks max players
//!    - Hashes token, resolves identity via persistence
//!    - Sends Welcome { token, player_id } or ConnectionRejected
//! 4. On success, spawns exactly one player entity.
//! 5. Stale session for the same PlayerId is replaced (disconnected before spawn).

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::prelude::*;
use game_core::constants::{GROUND_Y, MAX_PLAYERS};
use game_core::id::PlayerId;
use game_protocol::channels::ReliableChannel;
use game_protocol::{
    ConnectionRejected, IdentityHello, PROTOCOL_ID, PlayerColor, RejectionKind, Welcome,
};
use lightyear::connection::client_of::ClientOf;
use lightyear::connection::network_target::NetworkTarget;
use lightyear::prelude::*;
use player::{YumeScheme, spawn_player};
use tracing::{info, warn};

use crate::config::ServerConfig;

use super::connection::{ClientPlayer, NextPlayerColor, ServerConfigResource};
use super::setup::WalkConfig;
use social::systems::{ConnectedRoster, PlayerClientMap, SocialClientPlayer};

// ---------------------------------------------------------------------------
// Components / Resources
// ---------------------------------------------------------------------------

/// Marks a client link that has connected but not yet authenticated.
#[derive(Component)]
pub struct PendingSession;

/// Bevy resource wrapping the persistence handle for identity resolution.
#[derive(Resource, Clone)]
pub struct PersistenceResource(pub game_persistence::PersistenceHandle);

impl PersistenceResource {
    pub fn handle(&self) -> &game_persistence::PersistenceHandle {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Token generation
// ---------------------------------------------------------------------------

/// Generate a cryptographically random opaque identity token (v4 UUID hex).
fn generate_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Mix in a process-unique value so concurrent instances don't collide,
    // even when SystemTime has low resolution.
    let entropy = nanos | ((std::process::id() as u128) << 64);
    format!("tok_{:032x}", entropy)
}

// ---------------------------------------------------------------------------
// Observers / Systems
// ---------------------------------------------------------------------------

/// Handles a newly connected client: adds replication sender and marks it as
/// a pending session. **Does not spawn a player** — that happens only after
/// authentication via `IdentityHello`.
pub fn on_client_connected(
    trigger: On<Add, Connected>,
    mut commands: Commands,
    client_query: Query<&RemoteId, With<ClientOf>>,
    pending_query: Query<&PendingSession>,
    player_query: Query<&ClientPlayer>,
) {
    let client_entity = trigger.entity;
    let Ok(remote_id) = client_query.get(client_entity) else {
        return;
    };

    let total_sessions = pending_query.iter().count() + player_query.iter().count();
    if total_sessions >= MAX_PLAYERS {
        warn!(
            "server full ({total_sessions} sessions, max {MAX_PLAYERS}), rejecting new connection"
        );
        return;
    }

    let client_id = match remote_id.0 {
        PeerId::Netcode(id) => id,
        _ => remote_id.0.to_bits(),
    };

    commands
        .entity(client_entity)
        .insert((ReplicationSender, PendingSession));

    info!("Client {client_id} connected — pending session created (entity {client_entity:?})");
}

/// Processes `IdentityHello` messages from pending clients.
///
/// Validates protocol version, checks server capacity, resolves the identity
/// token via persistence, spawns a player on success, or disconnects the
/// client on failure.
#[allow(clippy::type_complexity)]
pub fn handle_identity_hello(
    mut receivers: Query<(
        Entity,
        &mut MessageReceiver<IdentityHello>,
        Option<&PendingSession>,
    )>,
    mut commands: Commands,
    mut welcome_senders: Query<&mut MessageSender<Welcome>>,
    mut rejected_senders: Query<&mut MessageSender<ConnectionRejected>>,
    persistence: Option<Res<PersistenceResource>>,
    server_config: Option<Res<ServerConfigResource>>,
    mut next_color: ResMut<NextPlayerColor>,
    mut roster: Option<ResMut<ConnectedRoster>>,
    mut player_client_map: Option<ResMut<PlayerClientMap>>,
    existing_players: Query<(Entity, &player::Player)>,
    pending_counter: Query<&PendingSession>,
    walk_config: Res<WalkConfig>,
) {
    let total_pending = pending_counter.iter().count();

    for (entity, mut receiver, pending) in receivers.iter_mut() {
        if pending.is_none() {
            continue;
        }

        for hello in receiver.receive() {
            let cfg = server_config
                .as_ref()
                .map(|c| c.0.clone())
                .unwrap_or_default();

            let p_ref = persistence.as_ref().map(|p| p.handle());
            let existing_player_count = existing_players.iter().count();
            let other_pending = total_pending.saturating_sub(1);
            let total_sessions = existing_player_count + other_pending;

            let result = resolve_identity(&hello, p_ref, &cfg, total_sessions);

            let (player_id, assigned_token) = match result {
                Ok(outcome) => (outcome.player_id, outcome.token),
                Err(rejection) => {
                    send_rejection(entity, &mut rejected_senders, rejection);
                    commands.entity(entity).despawn();
                    continue;
                }
            };

            // ── Load persisted state before spawning ───────────────────────
            let loaded_inventory: Vec<game_persistence::InventoryRow> = p_ref
                .and_then(|h| h.load_inventory(player_id.get() as i64).ok())
                .unwrap_or_default();

            // Stale session replacement: if the same PlayerId already has
            // a player entity, despawn it before spawning the new one.
            for (existing_entity, p) in existing_players.iter() {
                if p.id == player_id {
                    info!("Despawning stale player {player_id} for reconnecting identity");
                    commands.entity(existing_entity).try_despawn();
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

            // ── Apply loaded state to player entity ────────────────────────
            if !loaded_inventory.is_empty() {
                commands
                    .entity(player_entity)
                    .insert(resources::components::PlayerInventory {
                        inventory: inventory_from_rows(&loaded_inventory),
                    });
            }

            commands.entity(entity).insert(ClientPlayer {
                player_entity,
                player_id,
            });

            commands.entity(entity).insert(SocialClientPlayer {
                player_id,
                player_entity,
            });

            if let Some(ref mut r) = roster {
                r.add(player_id);
            }
            if let Some(ref mut pcm) = player_client_map {
                pcm.set(player_id, entity);
            }

            info!("Player {player_id} authenticated with color {}", color.0);

            commands.entity(entity).remove::<PendingSession>();

            if let Ok(mut sender) = welcome_senders.get_mut(entity) {
                sender.send::<ReliableChannel>(Welcome {
                    player_id: player_id.get(),
                    token: assigned_token,
                });
            }

            commands.entity(player_entity).insert((
                color,
                Replicate::to_clients(NetworkTarget::All),
                InterpolationTarget::to_clients(NetworkTarget::All),
                ControlledBy {
                    owner: entity,
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
    }
}

// ---------------------------------------------------------------------------
// Core auth logic (directly testable without Lightyear)
// ---------------------------------------------------------------------------

/// Outcome of a successful identity resolution.
#[derive(Debug)]
pub(crate) struct AuthOutcome {
    pub player_id: PlayerId,
    pub token: String,
}

/// Resolve an identity from an IdentityHello.
///
/// Returns `Ok(AuthOutcome)` on success or `Err(RejectionKind)` if the
/// connection should be rejected.
///
/// Callers should provide `persistence = None` when running without a
/// database (e.g. some unit tests). In that case, a new random player_id
/// and token are generated for every request — suitable for ephemeral
/// tests only.
pub(crate) fn resolve_identity(
    hello: &IdentityHello,
    persistence: Option<&game_persistence::PersistenceHandle>,
    server_config: &ServerConfig,
    player_count: usize,
) -> Result<AuthOutcome, RejectionKind> {
    // 1. Protocol version check
    if hello.protocol_version != PROTOCOL_ID as u32 {
        warn!(
            "protocol mismatch: client={}, server={}",
            hello.protocol_version, PROTOCOL_ID
        );
        return Err(RejectionKind::ProtocolMismatch);
    }

    // 2. Max players check
    if player_count >= server_config.max_players {
        warn!(
            "server full ({}/{}) — rejecting connection",
            player_count, server_config.max_players
        );
        return Err(RejectionKind::ServerFull);
    }

    // 3. Resolve identity
    let token = if hello.token.is_empty() {
        generate_token()
    } else {
        hello.token.clone()
    };

    let token_hash = game_persistence::hash_token(&token);

    let identity = match persistence {
        Some(handle) => match handle.resolve_identity(&token_hash) {
            Ok(row) => row,
            Err(e) => {
                warn!("identity resolution failed: {e}");
                return Err(RejectionKind::InvalidIdentity);
            }
        },
        // No persistence available: generate ephemeral identity for testing.
        None => {
            let fake_id = (token_hash.as_bytes()[0] as u64)
                | ((token_hash.as_bytes()[1] as u64) << 8)
                | ((token_hash.as_bytes()[2] as u64) << 16)
                | ((token_hash.as_bytes()[3] as u64) << 24);
            info!(
                "no persistence configured — using ephemeral id for token {}",
                &token_hash[..8]
            );
            return Ok(AuthOutcome {
                player_id: PlayerId::new(fake_id.max(1)),
                token,
            });
        }
    };

    let player_id = PlayerId::new(identity.player_id as u64);

    info!(
        "identity resolved: player_id={}, token_hash={}",
        player_id,
        &token_hash[..8]
    );

    Ok(AuthOutcome { player_id, token })
}

/// Send a `ConnectionRejected` message to a client.
fn send_rejection(
    client_entity: Entity,
    senders: &mut Query<&mut MessageSender<ConnectionRejected>>,
    reason: RejectionKind,
) {
    if let Ok(mut sender) = senders.get_mut(client_entity) {
        sender.send::<ReliableChannel>(ConnectionRejected { reason });
        info!("Sent {reason:?} rejection to client {client_entity:?}");
    } else {
        warn!("no ConnectionRejected sender for client {client_entity:?}");
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert persistence inventory rows to an ECS `Inventory`.
pub(crate) fn inventory_from_rows(
    rows: &[game_persistence::InventoryRow],
) -> game_core::inventory::Inventory {
    let mut inv = game_core::inventory::Inventory::default();
    for row in rows {
        let kind = match row.resource_kind.as_str() {
            "Wood" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Wood)
            }
            "Stone" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Stone)
            }
            "Berry" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Berry)
            }
            "Crystal" => game_core::inventory::ItemKind::Resource(
                game_core::resources::ResourceKind::Crystal,
            ),
            "Flower" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Flower)
            }
            "Fiber" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Fiber)
            }
            "Mushroom" => game_core::inventory::ItemKind::Resource(
                game_core::resources::ResourceKind::Mushroom,
            ),
            "Sap" => {
                game_core::inventory::ItemKind::Resource(game_core::resources::ResourceKind::Sap)
            }
            _ => continue,
        };
        let _ = inv.add(kind, row.quantity as u32);
    }
    inv
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(unused_must_use, clippy::redundant_locals)]
mod tests {
    use super::*;
    use crate::build_test_app;
    use crate::systems::connection::handle_new_client_link;
    use lightyear::connection::client_of::ClientOf;

    // ------------------------------------------------------------------
    // RED test 1: Connected observer must NOT spawn a player
    // ------------------------------------------------------------------

    #[test]
    fn connected_observer_does_not_spawn_player() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);
        app.add_observer(handle_new_client_link);

        app.world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(42)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);

        let player_count = app
            .world_mut()
            .query::<&player::Player>()
            .iter(app.world())
            .count();
        assert_eq!(
            player_count, 0,
            "must not spawn a player before IdentityHello auth"
        );
    }

    // ------------------------------------------------------------------
    // RED test 2: PendingSession is added on connect
    // ------------------------------------------------------------------

    #[test]
    fn connected_adds_pending_session() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);

        let client_entity = app
            .world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(1)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);

        assert!(
            app.world_mut()
                .get::<PendingSession>(client_entity)
                .is_some(),
            "Connected client must have PendingSession"
        );
    }

    // ------------------------------------------------------------------
    // resolve_identity tests (directly testable, no Lightyear needed)
    // ------------------------------------------------------------------

    #[test]
    fn resolve_identity_rejects_wrong_protocol_version() {
        let hello = IdentityHello {
            protocol_version: 999,
            token: String::new(),
        };
        let cfg = ServerConfig::default();
        let result = resolve_identity(&hello, None, &cfg, 0);
        assert!(
            matches!(result, Err(RejectionKind::ProtocolMismatch)),
            "expected ProtocolMismatch, got {result:?}"
        );
    }

    #[test]
    fn resolve_identity_rejects_when_server_full() {
        let hello = IdentityHello {
            protocol_version: PROTOCOL_ID as u32,
            token: String::new(),
        };
        let cfg = ServerConfig::default();
        let result = resolve_identity(&hello, None, &cfg, cfg.max_players);
        assert!(
            matches!(result, Err(RejectionKind::ServerFull)),
            "expected ServerFull, got {result:?}"
        );
    }

    #[test]
    fn resolve_identity_empty_token_generates_new_identity() {
        let hello = IdentityHello {
            protocol_version: PROTOCOL_ID as u32,
            token: String::new(),
        };
        let cfg = ServerConfig::default();
        let result = resolve_identity(&hello, None, &cfg, 0);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let outcome = result.unwrap();
        assert!(!outcome.token.is_empty(), "must generate a non-empty token");
        assert!(
            outcome.player_id.get() > 0,
            "must assign a positive player_id"
        );
    }

    #[test]
    fn resolve_identity_returning_token_restores_same_id() {
        let cfg = ServerConfig::default();

        let first = resolve_identity(
            &IdentityHello {
                protocol_version: PROTOCOL_ID as u32,
                token: String::new(),
            },
            None,
            &cfg,
            0,
        )
        .unwrap();

        let second = resolve_identity(
            &IdentityHello {
                protocol_version: PROTOCOL_ID as u32,
                token: first.token.clone(),
            },
            None,
            &cfg,
            0,
        )
        .unwrap();

        assert_eq!(
            second.player_id, first.player_id,
            "same token must restore same PlayerId"
        );
    }

    #[test]
    fn resolve_identity_concurrent_empty_token_distinct_ids() {
        let cfg = ServerConfig::default();
        let a = resolve_identity(
            &IdentityHello {
                protocol_version: PROTOCOL_ID as u32,
                token: String::new(),
            },
            None,
            &cfg,
            0,
        )
        .unwrap();
        let b = resolve_identity(
            &IdentityHello {
                protocol_version: PROTOCOL_ID as u32,
                token: String::new(),
            },
            None,
            &cfg,
            1,
        )
        .unwrap();

        assert_ne!(
            a.player_id, b.player_id,
            "two empty-token requests must get different ids"
        );
    }

    #[test]
    fn generate_token_produces_unique_values() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b, "tokens must be unique");
        assert!(!a.is_empty(), "token must not be empty");
    }

    // ------------------------------------------------------------------
    // RED→GREEN: capacity enforcement (MAX_PLAYERS = 16)
    // ------------------------------------------------------------------

    #[test]
    fn rejects_connection_when_server_full() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);

        // Fill the server with 16 authenticated clients
        for i in 0..MAX_PLAYERS {
            let dummy_player = app.world_mut().spawn_empty().id();
            app.world_mut().spawn(ClientPlayer {
                player_entity: dummy_player,
                player_id: PlayerId::new(i as u64 + 1),
            });
        }

        // 17th client attempts to connect
        let rejected = app
            .world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(999)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);

        assert!(
            app.world_mut().get::<PendingSession>(rejected).is_none(),
            "17th client must NOT receive PendingSession when server is full"
        );
    }

    #[test]
    fn slot_available_after_disconnect() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);

        // Fill the server with 16 authenticated clients
        let mut client_entities = Vec::new();
        for i in 0..MAX_PLAYERS {
            let dummy_player = app.world_mut().spawn_empty().id();
            let client = app
                .world_mut()
                .spawn(ClientPlayer {
                    player_entity: dummy_player,
                    player_id: PlayerId::new(i as u64 + 1),
                })
                .id();
            client_entities.push(client);
        }

        // Confirm 17th is rejected
        let rejected = app
            .world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(101)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);
        assert!(
            app.world_mut().get::<PendingSession>(rejected).is_none(),
            "17th client must be rejected when full"
        );

        // Despawn one authenticated client (simulates disconnect)
        app.world_mut().despawn(client_entities[0]);

        // Now a new client should be able to connect
        let replacement = app
            .world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(102)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);
        assert!(
            app.world_mut().get::<PendingSession>(replacement).is_some(),
            "After disconnect, a slot must become available for a new client"
        );
    }

    #[test]
    fn pending_disconnect_releases_slot() {
        let mut app = build_test_app();
        app.add_observer(on_client_connected);

        // Add 15 authenticated clients (one slot taken by a pending session)
        for i in 0..15 {
            let dummy_player = app.world_mut().spawn_empty().id();
            app.world_mut().spawn(ClientPlayer {
                player_entity: dummy_player,
                player_id: PlayerId::new(i as u64 + 1),
            });
        }

        // Add a pending session (simulates a hung/connecting client)
        let pending_client = app.world_mut().spawn(PendingSession).id();

        // A new connection should be rejected (15 + 1 = 16 = full)
        let rejected = app
            .world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(201)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);
        assert!(
            app.world_mut().get::<PendingSession>(rejected).is_none(),
            "New client must be rejected when 15 auth + 1 pending = full"
        );

        // Despawn the pending session (simulates pending disconnect/cleanup)
        app.world_mut().despawn(pending_client);

        // Now a new client should be accepted
        let replacement = app
            .world_mut()
            .spawn((Connected, RemoteId(PeerId::Netcode(202)), ClientOf))
            .id();
        app.world_mut().run_schedule(FixedUpdate);
        assert!(
            app.world_mut().get::<PendingSession>(replacement).is_some(),
            "After pending disconnect, a slot must become available"
        );
    }
}
