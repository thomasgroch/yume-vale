use game_core::actions::ActionKind;
use game_core::actions::EmoteKind;
use game_core::inventory::ItemKind;
use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Rejection kind for connection failures
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RejectionKind {
    ServerFull,
    ProtocolMismatch,
    InvalidIdentity,
}

// ---------------------------------------------------------------------------
// Input channel (sequenced-unreliable, ClientToServer only)
// ---------------------------------------------------------------------------

/// Movement/tick input sent every frame over the unreliable input channel.
/// Uses named primitives (no glam types) for reliable binary serialization.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClientInput {
    pub tick: u32,
    pub move_x: i8,
    pub move_z: i8,
    pub run: bool,
    pub jump: bool,
}

// ---------------------------------------------------------------------------
// Identity & connection (reliable, ClientToServer)
// ---------------------------------------------------------------------------

/// First message sent by a client after establishing transport.
/// The server issues or validates the identity token.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct IdentityHello {
    pub protocol_version: u32,
    pub token: String,
}

/// Debug redacts the token to avoid leaking secrets in logs.
impl fmt::Debug for IdentityHello {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdentityHello")
            .field("protocol_version", &self.protocol_version)
            .field("token", &"***")
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Connection responses (reliable, ServerToClient)
// ---------------------------------------------------------------------------

/// Sent on successful connection. Contains the assigned player ID and
/// an identity token for reconnection.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Welcome {
    pub player_id: u64,
    pub token: String,
}

/// Debug redacts the token.
impl fmt::Debug for Welcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Welcome")
            .field("player_id", &self.player_id)
            .field("token", &"***")
            .finish()
    }
}

/// Sent when the server rejects a connection attempt.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ConnectionRejected {
    pub reason: RejectionKind,
}

// ---------------------------------------------------------------------------
// Action intents (reliable, ClientToServer)
// ---------------------------------------------------------------------------

/// An action the player wants to perform (collect, build, interact, etc.).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ActionIntent {
    pub sequence: u64,
    pub kind: ActionKind,
    pub target_id: Option<u64>,
}

// ---------------------------------------------------------------------------
// Chat (reliable, bidirectional)
// ---------------------------------------------------------------------------

/// A chat message sent by a client.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChatSend {
    pub text: String,
}

/// A chat message forwarded by the server to all relevant clients.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChatReceived {
    pub from_player: u64,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Groups (reliable, ClientToServer)
// ---------------------------------------------------------------------------

/// Invite another player to a group.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroupInvite {
    pub target_player: u64,
}

/// Accept a pending group invitation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroupAccept;

/// Decline a pending group invitation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroupDecline;

/// Leave the current group.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroupLeave;

// ---------------------------------------------------------------------------
// Group state (reliable, ServerToClient)
// ---------------------------------------------------------------------------

/// Updated group member list sent to all group members.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroupUpdate {
    pub members: Vec<u64>,
}

// ---------------------------------------------------------------------------
// Emotes (reliable, ClientToServer)
// ---------------------------------------------------------------------------

/// An emote the player wants to perform.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EmoteIntent {
    pub emote: EmoteKind,
}

/// An emote broadcast by the server to all relevant clients.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EmoteBroadcast {
    pub from_player: u64,
    pub emote: EmoteKind,
}

// ---------------------------------------------------------------------------
// Input acknowledgement (reliable, ServerToClient)
// ---------------------------------------------------------------------------

/// Sent to each client periodically to confirm which tick the server has
/// processed. Enables client-side prediction corrections.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InputAck {
    pub last_processed_tick: u32,
}

// ---------------------------------------------------------------------------
// Snapshots (reliable, ServerToClient)
// ---------------------------------------------------------------------------

/// A single item slot within an inventory snapshot.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ItemSlotData {
    pub slot_index: u8,
    pub kind: ItemKind,
    pub quantity: u32,
}

/// Full inventory state sent to a client on reconnect or significant change.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InventorySnapshot {
    pub items: Vec<ItemSlotData>,
}

/// Convert a `game_core::Inventory` into snapshot items.
///
/// Shared by the collect, persistence, and quest systems so the slot→item
/// projection lives in one place rather than three.
pub fn inventory_to_snapshot_items(
    inventory: &game_core::inventory::Inventory,
) -> Vec<ItemSlotData> {
    inventory
        .slots
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            s.as_ref().map(|stack| ItemSlotData {
                slot_index: i as u8,
                kind: stack.kind,
                quantity: stack.quantity,
            })
        })
        .collect()
}

/// Sent when the server rejects an action due to a persistence error.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ActionRejected {
    /// The action sequence that was rejected.
    pub sequence: u64,
    /// Human-readable reason for the rejection.
    pub reason: String,
}

/// Progress toward a single objective within a quest.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ObjectiveProgress {
    pub objective_index: u8,
    pub current: u32,
    pub target: u32,
}

/// State of a single quest for a player.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuestStateData {
    pub quest_id: u64,
    pub completed: bool,
    pub progress: Vec<ObjectiveProgress>,
}

/// Full quest state sent to a client.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuestSnapshot {
    pub quests: Vec<QuestStateData>,
}

/// A social bond entry between two players.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BondEntry {
    pub target_player: u64,
    pub bond_level: u32,
}

/// Social bond state sent to a client.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BondSnapshot {
    pub bonds: Vec<BondEntry>,
}

// ---------------------------------------------------------------------------
// Housing plot intents (reliable, ClientToServer)
// ---------------------------------------------------------------------------

/// Client requests to place a crystal decoration on their housing plot.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlotBuildIntent {
    /// Inventory slot containing the decoration item.
    pub inventory_slot: u8,
}

/// Client requests to remove the decoration from their housing plot.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlotRemoveIntent;

/// A claimed plot or build entry.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlotEntry {
    pub plot_id: u64,
    pub position_x: f32,
    pub position_z: f32,
    pub building_kind: u8,
}

/// Plot/build state sent to a client.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlotSnapshot {
    pub plots: Vec<PlotEntry>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::resources::ResourceKind;

    // --- ClientInput ---

    #[test]
    fn client_input_serde_roundtrip() {
        let orig = ClientInput {
            tick: 42,
            move_x: 1,
            move_z: 0,
            run: true,
            jump: false,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ClientInput = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- RejectionKind ---

    #[test]
    fn rejection_kind_serde_roundtrip() {
        for kind in &[
            RejectionKind::ServerFull,
            RejectionKind::ProtocolMismatch,
            RejectionKind::InvalidIdentity,
        ] {
            let json = serde_json::to_string(kind).unwrap();
            let back: RejectionKind = serde_json::from_str(&json).unwrap();
            assert_eq!(*kind, back);
        }
    }

    // --- IdentityHello ---

    #[test]
    fn identity_hello_serde_roundtrip() {
        let orig = IdentityHello {
            protocol_version: 1,
            token: "secret-token".into(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: IdentityHello = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn identity_hello_debug_redacts_token() {
        let msg = IdentityHello {
            protocol_version: 1,
            token: "super-secret".into(),
        };
        let debug = format!("{msg:?}");
        assert!(!debug.contains("super-secret"), "Debug must not leak token");
        assert!(debug.contains("***"), "Debug should show redacted token");
    }

    // --- Welcome ---

    #[test]
    fn welcome_serde_roundtrip() {
        let orig = Welcome {
            player_id: 42,
            token: "issued-token".into(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: Welcome = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn welcome_debug_redacts_token() {
        let msg = Welcome {
            player_id: 1,
            token: "should-not-appear".into(),
        };
        let debug = format!("{msg:?}");
        assert!(
            !debug.contains("should-not-appear"),
            "Debug must not leak token"
        );
        assert!(debug.contains("***"), "Debug should show redacted token");
    }

    // --- ConnectionRejected ---

    #[test]
    fn connection_rejected_serde_roundtrip() {
        let orig = ConnectionRejected {
            reason: RejectionKind::ServerFull,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ConnectionRejected = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- ActionIntent ---

    #[test]
    fn action_intent_serde_roundtrip() {
        let orig = ActionIntent {
            sequence: 7,
            kind: ActionKind::Collect,
            target_id: Some(3),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ActionIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn action_intent_no_target() {
        let orig = ActionIntent {
            sequence: 1,
            kind: ActionKind::Emote,
            target_id: None,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ActionIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- Chat ---

    #[test]
    fn chat_send_serde_roundtrip() {
        let orig = ChatSend {
            text: "hello world".into(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ChatSend = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn chat_received_serde_roundtrip() {
        let orig = ChatReceived {
            from_player: 10,
            text: "hi!".into(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ChatReceived = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- Group messages ---

    #[test]
    fn group_invite_serde_roundtrip() {
        let orig = GroupInvite { target_player: 5 };
        let json = serde_json::to_string(&orig).unwrap();
        let back: GroupInvite = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn group_accept_serde_roundtrip() {
        let orig = GroupAccept;
        let json = serde_json::to_string(&orig).unwrap();
        let back: GroupAccept = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn group_decline_serde_roundtrip() {
        let orig = GroupDecline;
        let json = serde_json::to_string(&orig).unwrap();
        let back: GroupDecline = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn group_leave_serde_roundtrip() {
        let orig = GroupLeave;
        let json = serde_json::to_string(&orig).unwrap();
        let back: GroupLeave = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn group_update_serde_roundtrip() {
        let orig = GroupUpdate {
            members: vec![1, 2, 3],
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: GroupUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- EmoteIntent ---

    #[test]
    fn emote_intent_serde_roundtrip() {
        let orig = EmoteIntent {
            emote: EmoteKind::Dance,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: EmoteIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- EmoteBroadcast ---

    #[test]
    fn emote_broadcast_serde_roundtrip() {
        let orig = EmoteBroadcast {
            from_player: 3,
            emote: EmoteKind::Wave,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: EmoteBroadcast = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- InputAck ---

    #[test]
    fn input_ack_serde_roundtrip() {
        let orig = InputAck {
            last_processed_tick: 128,
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: InputAck = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- InventorySnapshot ---

    #[test]
    fn inventory_snapshot_serde_roundtrip() {
        let orig = InventorySnapshot {
            items: vec![
                ItemSlotData {
                    slot_index: 0,
                    kind: ItemKind::Resource(ResourceKind::Wood),
                    quantity: 10,
                },
                ItemSlotData {
                    slot_index: 1,
                    kind: ItemKind::Resource(ResourceKind::Berry),
                    quantity: 25,
                },
            ],
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: InventorySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- QuestSnapshot ---

    #[test]
    fn quest_snapshot_serde_roundtrip() {
        let orig = QuestSnapshot {
            quests: vec![QuestStateData {
                quest_id: 1,
                completed: false,
                progress: vec![ObjectiveProgress {
                    objective_index: 0,
                    current: 3,
                    target: 5,
                }],
            }],
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: QuestSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- BondSnapshot ---

    #[test]
    fn bond_snapshot_serde_roundtrip() {
        let orig = BondSnapshot {
            bonds: vec![BondEntry {
                target_player: 7,
                bond_level: 3,
            }],
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: BondSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- PlotBuildIntent ---

    #[test]
    fn plot_build_intent_serde_roundtrip() {
        let orig = PlotBuildIntent { inventory_slot: 3 };
        let json = serde_json::to_string(&orig).unwrap();
        let back: PlotBuildIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn plot_remove_intent_serde_roundtrip() {
        let orig = PlotRemoveIntent;
        let json = serde_json::to_string(&orig).unwrap();
        let back: PlotRemoveIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- ActionRejected ---

    #[test]
    fn action_rejected_serde_roundtrip() {
        let orig = ActionRejected {
            sequence: 7,
            reason: "persistence error: database full".into(),
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: ActionRejected = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- PlotSnapshot ---

    #[test]
    fn plot_snapshot_serde_roundtrip() {
        let orig = PlotSnapshot {
            plots: vec![PlotEntry {
                plot_id: 1,
                position_x: 10.0,
                position_z: -5.0,
                building_kind: 2,
            }],
        };
        let json = serde_json::to_string(&orig).unwrap();
        let back: PlotSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, back);
    }

    // --- All messages exercise Debug (no panics, token redacted) ---

    #[test]
    fn all_messages_debug_does_not_panic() {
        // Smoke test: every message type can be Debug-formatted without panic.
        let _ = format!(
            "{:?}",
            ClientInput {
                tick: 0,
                move_x: 0,
                move_z: 0,
                run: false,
                jump: false
            }
        );
        let _ = format!(
            "{:?}",
            IdentityHello {
                protocol_version: 1,
                token: "x".into()
            }
        );
        let _ = format!(
            "{:?}",
            Welcome {
                player_id: 0,
                token: "x".into()
            }
        );
        let _ = format!(
            "{:?}",
            ConnectionRejected {
                reason: RejectionKind::ServerFull
            }
        );
        let _ = format!(
            "{:?}",
            ActionIntent {
                sequence: 0,
                kind: ActionKind::Walk,
                target_id: None
            }
        );
        let _ = format!("{:?}", ChatSend { text: "".into() });
        let _ = format!(
            "{:?}",
            ChatReceived {
                from_player: 0,
                text: "".into()
            }
        );
        let _ = format!("{:?}", GroupInvite { target_player: 0 });
        let _ = format!("{:?}", GroupAccept);
        let _ = format!("{:?}", GroupDecline);
        let _ = format!("{:?}", GroupLeave);
        let _ = format!("{:?}", GroupUpdate { members: vec![] });
        let _ = format!(
            "{:?}",
            EmoteIntent {
                emote: EmoteKind::Wave
            }
        );
        let _ = format!(
            "{:?}",
            EmoteBroadcast {
                from_player: 0,
                emote: EmoteKind::Wave
            }
        );
        let _ = format!("{:?}", PlotBuildIntent { inventory_slot: 1 });
        let _ = format!("{:?}", PlotRemoveIntent);
        let _ = format!(
            "{:?}",
            InputAck {
                last_processed_tick: 0
            }
        );
        let _ = format!("{:?}", InventorySnapshot { items: vec![] });
        let _ = format!("{:?}", QuestSnapshot { quests: vec![] });
        let _ = format!("{:?}", BondSnapshot { bonds: vec![] });
        let _ = format!("{:?}", PlotSnapshot { plots: vec![] });
        let _ = format!(
            "{:?}",
            ActionRejected {
                sequence: 1,
                reason: "err".into()
            }
        );
    }
}
