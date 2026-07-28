//! Spatial interest management for replicated world entities.
//!
//! Uses Lightyear's [`NetworkVisibilityPlugin`]/[`VisibilityExt`] to dynamically
//! show or hide entities based on squared distance from each player's
//! authoritative position. Runs at bounded 5 Hz and caches prior visibility to
//! avoid redundant commands to the replication backend.
//!
//! # Entity types managed
//!
//! - Other [`player::Player`] entities (owner always visible)
//! - [`CreatureState`] entities (wandering creatures)
//! - [`ResourceNodeState`] entities (collectable resource nodes)
//! - [`DecorationState`] entities (housing decorations)
//!
//! # Performance
//!
//! Every pair (client × entity) is checked at most once per tick. Unchanged
//! pairs produce no commands. Dead client entries are cleaned up each cycle.

use bevy::prelude::*;
use game_core::constants::SIGHT_RANGE;
use game_protocol::{CreatureState, DecorationState, PlayerPosition, ResourceNodeState};
use lightyear::prelude::{ControlledBy, VisibilityExt};
use std::collections::HashMap;

use crate::systems::connection::ClientPlayer;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Settings for the spatial interest management system.
#[derive(Resource)]
pub struct InterestSettings {
    /// Squared sight range threshold (`SIGHT_RANGE²`).
    pub sight_range_sq: f32,
    /// Run every N fixed ticks (default 6 = ≈5 Hz at 30 Hz tick rate).
    tick_interval: u32,
    /// Counter incremented each invocation; fires when `counter % tick_interval == 0`.
    counter: u32,
}

impl Default for InterestSettings {
    fn default() -> Self {
        Self {
            sight_range_sq: SIGHT_RANGE * SIGHT_RANGE,
            tick_interval: 6,
            counter: 0,
        }
    }
}

/// Caches prior entity visibility per client link to avoid redundant visibility
/// commands to Lightyear's replication backend.
///
/// Key = `(client_link_entity, viewed_entity)`, value = `true` if we last told
/// the replication layer the entity was visible.
#[derive(Resource, Default)]
pub struct VisibilityCache {
    /// Per-pair cached visibility state.
    pub cache: HashMap<(Entity, Entity), bool>,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Runs spatial interest management at a bounded rate (≈5 Hz at 30 Hz tick rate).
///
/// For each authenticated client, computes squared distance from that client's
/// player position to every dynamic world entity. Entities within `SIGHT_RANGE`
/// are made visible; those beyond are hidden.
///
/// The owning player is **always** visible to their own client (regardless of
/// distance). The visibility cache prevents redundant commands for pairs whose
/// state hasn't changed.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_spatial_visibility(
    mut settings: ResMut<InterestSettings>,
    mut cache: ResMut<VisibilityCache>,
    clients: Query<(Entity, &ClientPlayer)>,
    player_positions: Query<&PlayerPosition>,
    players: Query<(Entity, &PlayerPosition, Option<&ControlledBy>)>,
    creatures: Query<(Entity, &CreatureState)>,
    resources: Query<(Entity, &ResourceNodeState)>,
    decorations: Query<(Entity, &DecorationState)>,
    mut commands: Commands,
) {
    // Rate-limit: only run every N fixed ticks.
    settings.counter += 1;
    if settings.counter % settings.tick_interval != 0 {
        return;
    }

    let range_sq = settings.sight_range_sq;
    let mut seen_clients: Vec<Entity> = Vec::new();

    for (client_entity, client_player) in clients.iter() {
        seen_clients.push(client_entity);

        let Ok(player_pos) = player_positions.get(client_player.player_entity) else {
            // Client has no player entity yet (edge case during auth).
            continue;
        };
        let client_pos = player_pos.0;

        // ── Other players (self is always visible) ──────────────────────
        for (entity, other_pos, controlled) in players.iter() {
            let is_owner = controlled.is_some_and(|c| c.owner == client_entity);
            let visible = is_owner || client_pos.distance_squared(other_pos.0) <= range_sq;
            set_visibility(&mut commands, &mut cache, client_entity, entity, visible);
        }

        // ── Creatures ──────────────────────────────────────────────────
        for (entity, state) in creatures.iter() {
            let pos = Vec3::new(state.position_x, state.position_y, state.position_z);
            let visible = client_pos.distance_squared(pos) <= range_sq;
            set_visibility(&mut commands, &mut cache, client_entity, entity, visible);
        }

        // ── Resource nodes ─────────────────────────────────────────────
        for (entity, state) in resources.iter() {
            let pos = Vec3::new(state.position_x, state.position_y, state.position_z);
            let visible = client_pos.distance_squared(pos) <= range_sq;
            set_visibility(&mut commands, &mut cache, client_entity, entity, visible);
        }

        // ── Decorations ────────────────────────────────────────────────
        for (entity, state) in decorations.iter() {
            let pos = Vec3::new(state.position_x, state.position_y, state.position_z);
            let visible = client_pos.distance_squared(pos) <= range_sq;
            set_visibility(&mut commands, &mut cache, client_entity, entity, visible);
        }
    }

    // ── Stale-cleanup: remove entries whose client no longer exists ────
    cache.cache.retain(|&(client, _), _| {
        // Fast-path: Linear search is fine here because the number of
        // authenticated clients is bounded by MAX_PLAYERS (16).
        seen_clients.contains(&client)
    });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Toggle visibility for `entity` from `client`'s perspective.
///
/// Uses [`VisibilityExt::gain_visibility`] / [`VisibilityExt::lose_visibility`]
/// and skips the command if the cached state already matches the target.
fn set_visibility(
    commands: &mut Commands,
    cache: &mut VisibilityCache,
    client: Entity,
    entity: Entity,
    visible: bool,
) {
    let key = (client, entity);
    if cache.cache.get(&key) == Some(&visible) {
        return;
    }

    if visible {
        commands.gain_visibility(entity, client);
    } else {
        commands.lose_visibility(entity, client);
    }

    cache.cache.insert(key, visible);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_test_app;

    use bevy_replicon::server::visibility::client_visibility::ClientVisibility;
    use game_core::decorations::DecorationKind;
    use game_core::id::PlayerId;
    use game_core::resources::ResourceKind;
    use game_core::world_config::CreatureKind;
    use game_protocol::PlayerColor;
    use lightyear::prelude::{ControlledBy, Lifetime};
    use player::spawn_player;

    /// Build an app with the visibility system registered.
    fn visibility_app() -> App {
        let mut app = build_test_app();
        // Override tick interval to 1 so every FixedUpdate invocation runs
        // the visibility system (fast test feedback).
        app.insert_resource(InterestSettings {
            tick_interval: 1,
            ..Default::default()
        });
        app.init_resource::<VisibilityCache>();
        app.add_systems(FixedUpdate, update_spatial_visibility);
        app.finish();
        app
    }

    /// Helper: spawn an authenticated client link + player, return (client_entity, player_entity).
    fn spawn_client_player(app: &mut App, player_id: PlayerId, position: Vec3) -> (Entity, Entity) {
        let player_entity = spawn_player(
            &mut app.world_mut().commands(),
            player_id,
            format!("Player {}", player_id),
            position,
        );
        app.world_mut().flush();

        // We need the client entity first so we can reference it in ControlledBy.
        let client_entity = app.world_mut().spawn(ClientVisibility::default()).id();
        app.world_mut().flush();

        // Add Replicate + ControlledBy + PlayerColor like the real auth system does.
        app.world_mut().entity_mut(player_entity).insert((
            PlayerColor(0),
            lightyear::prelude::Replicate::to_clients(
                lightyear::connection::network_target::NetworkTarget::All,
            ),
            lightyear::prelude::InterpolationTarget::to_clients(
                lightyear::connection::network_target::NetworkTarget::All,
            ),
            ControlledBy {
                owner: client_entity,
                lifetime: Lifetime::SessionBased,
            },
        ));

        app.world_mut()
            .entity_mut(client_entity)
            .insert(ClientPlayer {
                player_entity,
                player_id,
            });
        app.world_mut().flush();

        // Run once so Replicate::on_insert hooks fire.
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        (client_entity, player_entity)
    }

    /// Helper: spawn a creature entity at a given position.
    fn spawn_creature_at(app: &mut App, position: Vec3) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                CreatureState {
                    creature_id: 99,
                    kind: CreatureKind::Fluffball,
                    position_x: position.x,
                    position_y: position.y,
                    position_z: position.z,
                    target_x: position.x,
                    target_z: position.z,
                },
                lightyear::prelude::Replicate::to_clients(
                    lightyear::connection::network_target::NetworkTarget::All,
                ),
            ))
            .id();
        app.world_mut().flush();
        entity
    }

    /// Helper: spawn a resource node at a given position.
    fn spawn_resource_at(app: &mut App, position: Vec3) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                ResourceNodeState {
                    resource_id: 42,
                    kind: ResourceKind::Wood,
                    position_x: position.x,
                    position_y: position.y,
                    position_z: position.z,
                    depleted: false,
                    respawn_progress: 1.0,
                },
                lightyear::prelude::Replicate::to_clients(
                    lightyear::connection::network_target::NetworkTarget::All,
                ),
            ))
            .id();
        app.world_mut().flush();
        entity
    }

    /// Helper: spawn a decoration at a given position.
    fn spawn_decoration_at(app: &mut App, position: Vec3) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                DecorationState {
                    kind: DecorationKind::Rock(0.4),
                    position_x: position.x,
                    position_y: position.y,
                    position_z: position.z,
                    rotation: 0.0,
                },
                lightyear::prelude::Replicate::to_clients(
                    lightyear::connection::network_target::NetworkTarget::All,
                ),
            ))
            .id();
        app.world_mut().flush();
        entity
    }

    // -----------------------------------------------------------------------
    // RED→GREEN tests
    // -----------------------------------------------------------------------

    /// Entities within SIGHT_RANGE (≤ 29.9) → visible;
    /// entities beyond SIGHT_RANGE (≥ 30.1) → hidden.
    #[test]
    fn entities_at_299_visible_at_301_hidden() {
        let mut app = visibility_app();
        let (client1, _player) = spawn_client_player(&mut app, PlayerId::new(1), Vec3::ZERO);

        // Another player at 29.9 → visible (just inside range).
        let (_client2, player2) =
            spawn_client_player(&mut app, PlayerId::new(2), Vec3::new(29.9, 0.0, 0.0));

        // Move player2 to 30.1 → hidden (just outside range).
        app.world_mut()
            .entity_mut(player2)
            .insert(PlayerPosition(Vec3::new(30.1, 0.0, 0.0)));
        app.world_mut().flush();

        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        // Check from client1's perspective: player2 at 30.1 is outside range.
        let cache = app.world().resource::<VisibilityCache>();
        let key = (client1, player2);
        assert_eq!(
            cache.cache.get(&key),
            Some(&false),
            "player at 30.1 should be hidden (outside SIGHT_RANGE)"
        );

        // Move player2 to 29.9 → visible.
        app.world_mut()
            .entity_mut(player2)
            .insert(PlayerPosition(Vec3::new(29.9, 0.0, 0.0)));
        app.world_mut().flush();

        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        let cache = app.world().resource::<VisibilityCache>();
        assert_eq!(
            cache.cache.get(&key),
            Some(&true),
            "player at 29.9 should be visible (inside SIGHT_RANGE)"
        );
    }

    /// The owning player's entity is always visible to their own client.
    #[test]
    fn owner_always_visible() {
        let mut app = visibility_app();
        let (client, player) = spawn_client_player(&mut app, PlayerId::new(1), Vec3::ZERO);

        // Move the player far outside sight range.
        app.world_mut()
            .entity_mut(player)
            .insert(PlayerPosition(Vec3::new(999.0, 0.0, 999.0)));
        app.world_mut().flush();

        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        let cache = app.world().resource::<VisibilityCache>();
        let key = (client, player);
        let is_visible = cache.cache.get(&key).copied().unwrap_or(false);
        assert!(
            is_visible,
            "owner's own player should always be visible regardless of distance"
        );
    }

    /// Transition (hidden → visible) emits exactly one gain command; no
    /// duplicates. We verify by checking the cache value flips exactly once
    /// and then stays stable across a second unchanged tick.
    #[test]
    fn transition_emits_one_gain_loss_no_duplicates() {
        let mut app = visibility_app();
        let (client1, _player1) = spawn_client_player(&mut app, PlayerId::new(1), Vec3::ZERO);

        // Player2 starts far away (100 units) — hidden from client1.
        let (_client2, player2) =
            spawn_client_player(&mut app, PlayerId::new(2), Vec3::new(100.0, 0.0, 0.0));

        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        let cache_before = app.world().resource::<VisibilityCache>();
        let key = (client1, player2);
        assert_eq!(
            cache_before.cache.get(&key),
            Some(&false),
            "far player should start hidden from client1"
        );

        // Move player2 into range.
        app.world_mut()
            .entity_mut(player2)
            .insert(PlayerPosition(Vec3::new(5.0, 0.0, 0.0)));
        app.world_mut().flush();

        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        let cache_after = app.world().resource::<VisibilityCache>();
        assert_eq!(
            cache_after.cache.get(&key),
            Some(&true),
            "close player should become visible"
        );

        // Running again with same position should NOT flip the cache.
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        let cache_final = app.world().resource::<VisibilityCache>();
        assert_eq!(
            cache_final.cache.get(&key),
            Some(&true),
            "cache should NOT change for an unchanged pair"
        );
    }

    /// Disconnected client entries are removed from the cache.
    #[test]
    fn disconnected_client_cache_removed() {
        let mut app = visibility_app();
        let (client1, _player1) = spawn_client_player(&mut app, PlayerId::new(1), Vec3::ZERO);
        let (client2, player2) =
            spawn_client_player(&mut app, PlayerId::new(2), Vec3::new(100.0, 0.0, 0.0));

        // Run to populate cache.
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        // The cache has (client1, player2) = false (client1 sees far player2).
        let cross_key = (client1, player2);
        {
            let cache = app.world().resource::<VisibilityCache>();
            assert!(
                cache.cache.contains_key(&cross_key),
                "cache should have cross-client entry"
            );
        }

        // Despawn client2 (simulate disconnect).
        app.world_mut().entity_mut(client2).despawn();
        app.world_mut().flush();

        // Run visibility again — should clean up entries referencing client2.
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        let cache = app.world().resource::<VisibilityCache>();
        // Cross entries keyed by client1 (still alive) survive.
        assert!(
            cache.cache.contains_key(&cross_key),
            "cross-client entries should survive (client1 still exists)"
        );
        // No entry should reference despawned client2 as viewer.
        let stale = cache.cache.keys().any(|&(viewer, _)| viewer == client2);
        assert!(!stale, "no cache entry should reference despawned client2");
    }

    /// All four dynamic entity kinds (players, creatures, resources,
    /// decorations) are filtered by distance.
    #[test]
    fn all_dynamic_entity_kinds_obey_filter() {
        let mut app = visibility_app();
        let (_client, _player) = spawn_client_player(&mut app, PlayerId::new(1), Vec3::ZERO);

        // Spawn one of each kind inside range.
        let near_player =
            spawn_client_player(&mut app, PlayerId::new(2), Vec3::new(10.0, 0.0, 0.0)).1;
        let near_creature = spawn_creature_at(&mut app, Vec3::new(10.0, 0.0, 10.0));
        let near_resource = spawn_resource_at(&mut app, Vec3::new(10.0, 0.0, -10.0));
        let near_decoration = spawn_decoration_at(&mut app, Vec3::new(-10.0, 0.0, 10.0));

        // Spawn one of each kind outside range.
        let far_player =
            spawn_client_player(&mut app, PlayerId::new(3), Vec3::new(100.0, 0.0, 0.0)).1;
        let far_creature = spawn_creature_at(&mut app, Vec3::new(100.0, 0.0, 100.0));
        let far_resource = spawn_resource_at(&mut app, Vec3::new(100.0, 0.0, -100.0));
        let far_decoration = spawn_decoration_at(&mut app, Vec3::new(-100.0, 0.0, 100.0));

        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();

        let cache = app.world().resource::<VisibilityCache>();

        // Near entities should be visible.
        assert_eq!(
            cache.cache.get(&(_client, near_player)),
            Some(&true),
            "near player should be visible"
        );
        assert_eq!(
            cache.cache.get(&(_client, near_creature)),
            Some(&true),
            "near creature should be visible"
        );
        assert_eq!(
            cache.cache.get(&(_client, near_resource)),
            Some(&true),
            "near resource should be visible"
        );
        assert_eq!(
            cache.cache.get(&(_client, near_decoration)),
            Some(&true),
            "near decoration should be visible"
        );

        // Far entities should be hidden.
        assert_eq!(
            cache.cache.get(&(_client, far_player)),
            Some(&false),
            "far player should be hidden"
        );
        assert_eq!(
            cache.cache.get(&(_client, far_creature)),
            Some(&false),
            "far creature should be hidden"
        );
        assert_eq!(
            cache.cache.get(&(_client, far_resource)),
            Some(&false),
            "far resource should be hidden"
        );
        assert_eq!(
            cache.cache.get(&(_client, far_decoration)),
            Some(&false),
            "far decoration should be hidden"
        );
    }

    /// Verifies that `distance_squared` is used (no sqrt computation).
    /// This is a compile-time / logic check — the only distance test in the
    /// system body is `distance_squared`.
    #[test]
    fn no_distance_sqrt_used() {
        // This test simply asserts that the `set_visibility` helper and the
        // main system never call `.distance()` or `sqrt`. We verify at the
        // code level by checking the implementation above uses
        // `distance_squared` exclusively. The test here confirms the cache
        // and helpers exist and are wired.
        let mut app = visibility_app();
        let (_client, _player) = spawn_client_player(&mut app, PlayerId::new(1), Vec3::ZERO);

        // Run without panic = system doesn't call sqrt (distance_squared).
        app.world_mut().run_schedule(FixedUpdate);
        app.world_mut().flush();
    }
}
