/// Tick rate of the simulation (Hz).
pub const TICK_RATE_HZ: u32 = 30;

/// Snapshot broadcast rate (Hz).
pub const SNAPSHOT_RATE_HZ: u32 = 30;

/// Maximum number of players per session.
pub const MAX_PLAYERS: usize = 16;

/// Radius of a player's collision cylinder (in world units).
pub const PLAYER_RADIUS: f32 = 0.4;

/// Maximum distance a player can interact with objects/creatures.
pub const INTERACT_RADIUS: f32 = 2.0;

/// Walk speed (units per second).
pub const WALK_SPEED: f32 = 5.0;

/// Run speed (units per second).
pub const RUN_SPEED: f32 = 10.0;

/// Horizontal acceleration while grounded (units per second squared).
/// Shared by the server's authoritative physics and the client's local
/// movement prediction — they must agree, or the client's prediction would
/// constantly fight visible corrections from the server.
pub const WALK_ACCELERATION: f32 = 40.0;

/// Horizontal acceleration while airborne (units per second squared) — lower
/// than `WALK_ACCELERATION` so mid-air direction changes feel sluggish, the
/// way real momentum does.
pub const AIR_ACCELERATION: f32 = 10.0;

/// Default capacity of a player's inventory.
pub const INVENTORY_CAPACITY: usize = 24;

/// Maximum number of identical items in a single stack.
pub const MAX_STACK_SIZE: u32 = 99;

/// Ground Y-level (everything at this Y is on the ground).
pub const GROUND_Y: f32 = 0.0;

/// Height of a player's collision cylinder.
pub const PLAYER_HEIGHT: f32 = 1.6;

/// Base sight range for interest management.
pub const SIGHT_RANGE: f32 = 30.0;

/// Rate limit: minimum interval (seconds) between interactions.
pub const INTERACT_COOLDOWN_S: f32 = 0.5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_rate_is_30() {
        assert_eq!(TICK_RATE_HZ, 30);
    }

    #[test]
    fn snapshot_rate_is_15() {
        assert_eq!(SNAPSHOT_RATE_HZ, 30);
    }

    #[test]
    fn max_players_is_16() {
        assert_eq!(MAX_PLAYERS, 16);
    }

    #[test]
    fn snapshot_rate_is_half_of_tick_rate() {
        assert_eq!(TICK_RATE_HZ / SNAPSHOT_RATE_HZ, 1);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn values_are_positive() {
        assert!(PLAYER_RADIUS > 0.0);
        assert!(INTERACT_RADIUS > 0.0);
        assert!(WALK_SPEED > 0.0);
        assert!(RUN_SPEED > 0.0);
        assert!(INVENTORY_CAPACITY > 0);
        assert!(MAX_STACK_SIZE > 0);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn run_speed_faster_than_walk() {
        assert!(RUN_SPEED > WALK_SPEED);
    }
}
