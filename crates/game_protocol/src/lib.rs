pub mod channels;
pub mod components;
pub mod messages;
pub mod protocol;

pub const PROTOCOL_ID: u64 = 0x59c3_7a6e;
pub const PRIVATE_KEY: [u8; 32] = *b"yume-vale-dev-key-00000000000000";

pub use channels::*;
pub use components::*;
pub use messages::*;
pub use protocol::ProtocolPlugin;

#[cfg(test)]
mod tests {
    use crate::messages::*;
    use crate::protocol::ProtocolPlugin;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use bevy_replicon::shared::{AuthMethod, RepliconSharedPlugin};
    use lightyear::prelude::*;

    #[test]
    fn protocol_plugin_registers_channels() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.add_plugins(RepliconSharedPlugin {
            auth_method: AuthMethod::None,
        });
        app.add_plugins(ProtocolPlugin);
        assert!(app.world().contains_resource::<ChannelRegistry>());
    }

    #[test]
    fn protocol_plugin_registers_messages() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.add_plugins(RepliconSharedPlugin {
            auth_method: AuthMethod::None,
        });
        app.add_plugins(ProtocolPlugin);
        assert!(app.is_message_registered::<ClientInput>());
        assert!(app.is_message_registered::<Welcome>());
    }
}
