use bevy::prelude::*;
use game_core::game_state::GroupId;
use game_core::id::PlayerId;
use game_protocol::channels::ReliableChannel;
use game_protocol::{
    ChatReceived, ChatSend, EmoteBroadcast, EmoteIntent, GroupAccept, GroupDecline, GroupInvite,
    GroupLeave, GroupUpdate,
};
use lightyear::prelude::{MessageReceiver, MessageSender};
use player::Player;
use tracing::info;

use crate::state::{SocialState, validate_chat};

// ---------------------------------------------------------------------------
// Bevy resources & components
// ---------------------------------------------------------------------------

/// Wraps the pure SocialState as a Bevy resource.
#[derive(Resource, Default)]
pub struct SocialStateResource(pub SocialState);

/// Added to player entities: marks their group membership.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct PlayerGroup(pub Option<GroupId>);

/// Added to client link entities so social systems can identify the player
/// without depending on game_server's `ClientPlayer`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SocialClientPlayer {
    pub player_id: PlayerId,
    pub player_entity: Entity,
}

/// Tracks which PlayerIds are currently connected+authenticated.
#[derive(Resource, Default)]
pub struct ConnectedRoster {
    pub players: Vec<PlayerId>,
}

impl ConnectedRoster {
    pub fn is_connected(&self, player: PlayerId) -> bool {
        self.players.contains(&player)
    }

    pub fn add(&mut self, player: PlayerId) {
        if !self.players.contains(&player) {
            self.players.push(player);
        }
    }

    pub fn remove(&mut self, player: PlayerId) {
        self.players.retain(|p| *p != player);
    }
}

/// Maps PlayerId -> client link Entity for sending messages to specific clients.
#[derive(Resource, Default)]
pub struct PlayerClientMap {
    mapping: Vec<(PlayerId, Entity)>,
}

impl PlayerClientMap {
    pub fn set(&mut self, player: PlayerId, entity: Entity) {
        self.mapping.retain(|(p, _)| *p != player);
        self.mapping.push((player, entity));
    }

    pub fn remove(&mut self, player: PlayerId) {
        self.mapping.retain(|(p, _)| *p != player);
    }

    pub fn get(&self, player: PlayerId) -> Option<Entity> {
        self.mapping
            .iter()
            .find(|(p, _)| *p == player)
            .map(|(_, e)| *e)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn find_player_group(
    player: PlayerId,
    player_query: &Query<(Entity, &Player, Option<&PlayerGroup>)>,
) -> Option<(GroupId, Vec<PlayerId>)> {
    for (_, p, group) in player_query.iter() {
        if p.id == player {
            if let Some(Some(group_id)) = group.map(|g| &g.0) {
                let mut members: Vec<PlayerId> = player_query
                    .iter()
                    .filter_map(|(_, p2, g2)| {
                        if let Some(Some(gid)) = g2.map(|gg| &gg.0) {
                            if gid == group_id {
                                return Some(p2.id);
                            }
                        }
                        None
                    })
                    .collect();
                members.sort_by_key(|id| id.get());
                return Some((*group_id, members));
            }
        }
    }
    None
}

fn broadcast_group_update(
    members: &[PlayerId],
    client_map: &PlayerClientMap,
    senders: &mut Query<&mut MessageSender<GroupUpdate>>,
) {
    let update = GroupUpdate {
        members: members.iter().map(|p| p.get()).collect(),
    };
    for player_id in members {
        if let Some(client_entity) = client_map.get(*player_id) {
            if let Ok(mut sender) = senders.get_mut(client_entity) {
                sender.send::<ReliableChannel>(update.clone());
            }
        }
    }
}

fn remove_from_group(
    player_id: PlayerId,
    player_query: &mut Query<(Entity, &Player, Option<&PlayerGroup>)>,
    commands: &mut Commands,
    client_map: &PlayerClientMap,
    group_senders: &mut Query<&mut MessageSender<GroupUpdate>>,
) {
    let Some((_group_id, members)) = find_player_group(player_id, player_query) else {
        return;
    };

    let remaining: Vec<PlayerId> = members
        .iter()
        .filter(|&&id| id != player_id)
        .copied()
        .collect();

    for (entity, p, _) in player_query.iter_mut() {
        if p.id == player_id {
            commands.entity(entity).remove::<PlayerGroup>();
        }
    }

    broadcast_group_update(&remaining, client_map, group_senders);
}

// ---------------------------------------------------------------------------
// Chat system
// ---------------------------------------------------------------------------

/// Validate and broadcast `ChatSend` to all connected clients.
pub fn handle_chat_send(
    mut receivers: Query<(&mut MessageReceiver<ChatSend>, &SocialClientPlayer)>,
    client_map: Res<PlayerClientMap>,
    mut chat_senders: Query<&mut MessageSender<ChatReceived>>,
    roster: Res<ConnectedRoster>,
) {
    for (mut receiver, client_player) in receivers.iter_mut() {
        for msg in receiver.receive() {
            let Some(validated) = validate_chat(&msg.text) else {
                continue;
            };
            if !roster.is_connected(client_player.player_id) {
                continue;
            }

            let broadcast = ChatReceived {
                from_player: client_player.player_id.get(),
                text: validated,
            };

            for &pid in &roster.players {
                if let Some(target_entity) = client_map.get(pid) {
                    if let Ok(mut sender) = chat_senders.get_mut(target_entity) {
                        sender.send::<ReliableChannel>(broadcast.clone());
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Group invite
// ---------------------------------------------------------------------------

/// Record a pending group invite.
pub fn handle_group_invite(
    mut receivers: Query<(&mut MessageReceiver<GroupInvite>, &SocialClientPlayer)>,
    mut social: ResMut<SocialStateResource>,
    roster: Res<ConnectedRoster>,
) {
    for (mut receiver, client_player) in receivers.iter_mut() {
        for msg in receiver.receive() {
            let from = client_player.player_id;
            let target = PlayerId::new(msg.target_player);

            if !roster.is_connected(from) || !roster.is_connected(target) {
                info!("invite rejected: {from} -> {target} (not connected)");
                continue;
            }

            if let Err(e) = social.0.add_invite(from, target) {
                info!("invite rejected: {from} -> {target} ({e:?})");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Group accept
// ---------------------------------------------------------------------------

/// Accept a pending invite -- create a group and send `GroupUpdate`.
pub fn handle_group_accept(
    mut receivers: Query<(&mut MessageReceiver<GroupAccept>, &SocialClientPlayer)>,
    mut social: ResMut<SocialStateResource>,
    mut player_query: Query<(Entity, &Player, Option<&PlayerGroup>)>,
    roster: Res<ConnectedRoster>,
    mut commands: Commands,
    client_map: Res<PlayerClientMap>,
    mut group_senders: Query<&mut MessageSender<GroupUpdate>>,
) {
    for (mut receiver, client_player) in receivers.iter_mut() {
        for _ in receiver.receive() {
            let accepter = client_player.player_id;
            if !roster.is_connected(accepter) {
                continue;
            }

            let Some(invite) = social.0.consume_invite_for(accepter) else {
                continue;
            };
            let inviter = invite.from_player;

            if !roster.is_connected(inviter) {
                continue;
            }

            let in_group = |pid: PlayerId| -> bool {
                player_query
                    .iter()
                    .any(|(_, p, g)| p.id == pid && g.is_some() && g.unwrap().0.is_some())
            };

            if in_group(accepter) || in_group(inviter) {
                continue;
            }

            let group_id = social.0.allocate_group_id();
            let members = vec![inviter, accepter];

            for (entity, p, _) in player_query.iter_mut() {
                if p.id == inviter || p.id == accepter {
                    commands.entity(entity).insert(PlayerGroup(Some(group_id)));
                }
            }

            broadcast_group_update(&members, &client_map, &mut group_senders);
        }
    }
}

// ---------------------------------------------------------------------------
// Group decline
// ---------------------------------------------------------------------------

/// Decline a pending invite.
pub fn handle_group_decline(
    mut receivers: Query<(&mut MessageReceiver<GroupDecline>, &SocialClientPlayer)>,
    mut social: ResMut<SocialStateResource>,
) {
    for (mut receiver, client_player) in receivers.iter_mut() {
        for _ in receiver.receive() {
            if social
                .0
                .consume_invite_for(client_player.player_id)
                .is_none()
            {
                info!("decline ignored: no invite for {}", client_player.player_id);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Group leave
// ---------------------------------------------------------------------------

/// Leave current group and broadcast update to remaining members.
pub fn handle_group_leave(
    mut receivers: Query<(&mut MessageReceiver<GroupLeave>, &SocialClientPlayer)>,
    mut player_query: Query<(Entity, &Player, Option<&PlayerGroup>)>,
    mut commands: Commands,
    client_map: Res<PlayerClientMap>,
    mut group_senders: Query<&mut MessageSender<GroupUpdate>>,
    roster: Res<ConnectedRoster>,
) {
    for (mut receiver, client_player) in receivers.iter_mut() {
        for _ in receiver.receive() {
            if !roster.is_connected(client_player.player_id) {
                continue;
            }
            remove_from_group(
                client_player.player_id,
                &mut player_query,
                &mut commands,
                &client_map,
                &mut group_senders,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Emote broadcast
// ---------------------------------------------------------------------------

/// Validate and broadcast `EmoteIntent` to all connected clients.
pub fn handle_emote_intent(
    mut receivers: Query<(&mut MessageReceiver<EmoteIntent>, &SocialClientPlayer)>,
    client_map: Res<PlayerClientMap>,
    mut emote_senders: Query<&mut MessageSender<EmoteBroadcast>>,
    roster: Res<ConnectedRoster>,
) {
    for (mut receiver, client_player) in receivers.iter_mut() {
        for msg in receiver.receive() {
            if !roster.is_connected(client_player.player_id) {
                continue;
            }

            let broadcast = EmoteBroadcast {
                from_player: client_player.player_id.get(),
                emote: msg.emote,
            };

            for &pid in &roster.players {
                if let Some(target_entity) = client_map.get(pid) {
                    if let Ok(mut sender) = emote_senders.get_mut(target_entity) {
                        sender.send::<ReliableChannel>(broadcast.clone());
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Disconnect cleanup
// ---------------------------------------------------------------------------

/// Clean up social state when a player disconnects.
///
/// Detects disconnection by monitoring `Connected` component removal.
#[allow(clippy::too_many_arguments)]
pub fn cleanup_disconnected_player(
    mut removals: bevy::prelude::RemovedComponents<lightyear::prelude::Connected>,
    connected_players: Query<(Entity, &SocialClientPlayer)>,
    mut roster: ResMut<ConnectedRoster>,
    mut social: ResMut<SocialStateResource>,
    mut player_query: Query<(Entity, &Player, Option<&PlayerGroup>)>,
    mut commands: Commands,
    mut group_senders: Query<&mut MessageSender<GroupUpdate>>,
    mut player_client_map: ResMut<PlayerClientMap>,
) {
    for entity in removals.read() {
        if let Ok((_, client_player)) = connected_players.get(entity) {
            let pid = client_player.player_id;
            social.0.remove_all_for_player(pid);
            roster.remove(pid);
            player_client_map.remove(pid);
            remove_from_group(
                pid,
                &mut player_query,
                &mut commands,
                &player_client_map,
                &mut group_senders,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Read API for quest system
// ---------------------------------------------------------------------------

/// Get a player's group ID, if any.
pub fn get_player_group_id(
    player: PlayerId,
    player_query: &Query<(Entity, &Player, Option<&PlayerGroup>)>,
) -> Option<GroupId> {
    for (_, p, group) in player_query.iter() {
        if p.id == player {
            return group.and_then(|g| g.0);
        }
    }
    None
}

/// Get all members of a player's group.
pub fn get_group_members(
    player: PlayerId,
    player_query: &Query<(Entity, &Player, Option<&PlayerGroup>)>,
) -> Vec<PlayerId> {
    match get_player_group_id(player, player_query) {
        None => Vec::new(),
        Some(group_id) => player_query
            .iter()
            .filter_map(|(_, p, g)| {
                if let Some(Some(gid)) = g.map(|gg| &gg.0) {
                    if gid == &group_id {
                        return Some(p.id);
                    }
                }
                None
            })
            .collect(),
    }
}
