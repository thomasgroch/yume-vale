pub mod channels;
pub mod components;
pub mod messages;
pub mod protocol;

pub const PROTOCOL_ID: u64 = 0x59c3_7a73;
pub const PRIVATE_KEY: [u8; 32] = *b"yume-vale-dev-key-00000000000000";

/// Seconds of silence before netcode considers a connection dead.
///
/// The client bakes this into the connect token it builds (`Authentication::
/// Manual`), and that token-embedded value is what actually governs —
/// the server's own `NetcodeConfig.client_timeout_secs` is currently unused
/// by this connect-token-based auth flow. Both sides still set it from this
/// one constant so a future change to the auth model (e.g. server-issued
/// tokens) can't silently end up with the client and server disagreeing.
pub const CLIENT_TIMEOUT_SECS: i32 = 10;

pub use channels::*;
pub use components::*;
pub use messages::*;
pub use protocol::ProtocolPlugin;

#[cfg(test)]
mod tests {
    use crate::components::*;
    use crate::messages::*;
    use crate::protocol::ProtocolPlugin;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use bevy_replicon::shared::{AuthMethod, RepliconSharedPlugin};
    use lightyear::prelude::*;

    /// Helper: build a minimal app with ProtocolPlugin.
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.add_plugins(RepliconSharedPlugin {
            auth_method: AuthMethod::None,
        });
        app.add_plugins(ProtocolPlugin);
        app
    }

    #[test]
    fn protocol_plugin_registers_channels() {
        let app = test_app();
        assert!(app.world().contains_resource::<ChannelRegistry>());
    }

    // ── Message registration tests ────────────────────────────────────────

    #[test]
    fn all_input_channel_messages_registered() {
        let app = test_app();
        assert!(
            app.is_message_registered::<ClientInput>(),
            "ClientInput must be registered"
        );
    }

    #[test]
    fn all_c2s_reliable_messages_registered() {
        let app = test_app();
        assert!(app.is_message_registered::<IdentityHello>());
        assert!(app.is_message_registered::<ActionIntent>());
        assert!(app.is_message_registered::<EmoteIntent>());
        assert!(app.is_message_registered::<PlotBuildIntent>());
        assert!(app.is_message_registered::<PlotRemoveIntent>());
    }

    #[test]
    fn all_s2c_reliable_messages_registered() {
        let app = test_app();
        assert!(app.is_message_registered::<Welcome>());
        assert!(app.is_message_registered::<ConnectionRejected>());
        assert!(app.is_message_registered::<EmoteBroadcast>());
        assert!(app.is_message_registered::<InventorySnapshot>());
        assert!(app.is_message_registered::<BondSnapshot>());
        assert!(app.is_message_registered::<PlotSnapshot>());
    }

    #[test]
    fn total_messages_registered() {
        let app = test_app();
        // We don't have a direct count API, but this serves as a smoke test
        // that the registration block doesn't panic.
        assert!(app.is_message_registered::<ClientInput>());
        assert!(app.is_message_registered::<PlotSnapshot>());
    }

    // ── Component replication tests ───────────────────────────────────────

    fn assert_component_registered<C: 'static>(app: &App) {
        let registry = app.world().resource::<ComponentRegistry>();
        assert!(registry.is_registered::<C>());
    }

    #[test]
    fn player_position_is_replicated() {
        assert_component_registered::<PlayerPosition>(&test_app());
    }

    #[test]
    fn player_color_is_replicated() {
        assert_component_registered::<PlayerColor>(&test_app());
    }

    #[test]
    fn resource_node_state_is_replicated() {
        assert_component_registered::<ResourceNodeState>(&test_app());
    }

    #[test]
    fn creature_state_is_replicated() {
        assert_component_registered::<CreatureState>(&test_app());
    }

    #[test]
    fn decoration_state_is_replicated() {
        assert_component_registered::<DecorationState>(&test_app());
    }

    // ── Direction tests: verify the expected sender can send each type ────
    // These confirm the protocol layout by checking that registration exists
    // and the plugin completes successfully. True sender/receiver enforcement
    // happens in the transport layer; these tests ensure the protocol
    // declaration is internally consistent.

    #[test]
    fn c2s_messages_include_input_and_intents() {
        let app = test_app();
        // All client-to-server message types
        assert!(app.is_message_registered::<ClientInput>());
        assert!(app.is_message_registered::<IdentityHello>());
        assert!(app.is_message_registered::<ActionIntent>());
        assert!(app.is_message_registered::<EmoteIntent>());
    }

    #[test]
    fn s2c_messages_include_welcome_and_snapshots() {
        let app = test_app();
        // All server-to-client message types
        assert!(app.is_message_registered::<Welcome>());
        assert!(app.is_message_registered::<ConnectionRejected>());
        assert!(app.is_message_registered::<EmoteBroadcast>());
        assert!(app.is_message_registered::<InventorySnapshot>());
        assert!(app.is_message_registered::<BondSnapshot>());
        assert!(app.is_message_registered::<PlotSnapshot>());
    }
}
