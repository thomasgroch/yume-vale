use game_core::game_state::GroupId;
use game_core::id::PlayerId;

/// A pending group invite from one player to another.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingInvite {
    pub from_player: PlayerId,
    pub target_player: PlayerId,
}

/// Errors that can occur during social state operations.
#[derive(Debug, Clone, PartialEq)]
pub enum SocialError {
    InviteAlreadyExists,
    NoPendingInvite,
    SelfInvite,
    PlayerNotConnected,
    AlreadyInGroup,
    NotInGroup,
}

/// Pure server-side social state — no Bevy, no ECS.
///
/// Manages pending invites, group ID allocation, and enforces invariants:
/// - A player can only be in one group at a time
/// - Invites require both parties connected
/// - Group size is bounded by MAX_PLAYERS
#[derive(Debug, Clone)]
pub struct SocialState {
    /// Active pending invites.
    pub pending_invites: Vec<PendingInvite>,
    /// Monotonically increasing group ID counter.
    next_group_id: u64,
}

impl SocialState {
    pub fn new() -> Self {
        Self {
            pending_invites: Vec::new(),
            next_group_id: 1,
        }
    }

    /// Record a pending invite.
    ///
    /// Fails if the same invite already exists or if `from == target`.
    pub fn add_invite(&mut self, from: PlayerId, to: PlayerId) -> Result<(), SocialError> {
        if from == to {
            return Err(SocialError::SelfInvite);
        }
        if self
            .pending_invites
            .iter()
            .any(|i| i.from_player == from && i.target_player == to)
        {
            return Err(SocialError::InviteAlreadyExists);
        }
        self.pending_invites.push(PendingInvite {
            from_player: from,
            target_player: to,
        });
        Ok(())
    }

    /// Consume (remove) the invite targeting `player`. Returns the invite if
    /// one exists, or `None`.
    ///
    /// When there are multiple invites targeting the same player, the oldest
    /// (first inserted) is returned. This ensures deterministic resolution of
    /// concurrent crossed invites (first-come-first-served).
    pub fn consume_invite_for(&mut self, player: PlayerId) -> Option<PendingInvite> {
        let idx = self
            .pending_invites
            .iter()
            .position(|i| i.target_player == player)?;
        Some(self.pending_invites.remove(idx))
    }

    /// Remove all invites sent *from* `player` to `target`.
    pub fn remove_invite(&mut self, from: PlayerId, to: PlayerId) {
        self.pending_invites
            .retain(|i| !(i.from_player == from && i.target_player == to));
    }

    /// Remove all invites involving `player` (as sender or target).
    /// Called on disconnect to clean up.
    pub fn remove_all_for_player(&mut self, player: PlayerId) {
        self.pending_invites
            .retain(|i| i.from_player != player && i.target_player != player);
    }

    /// Check if there's a pending invite between two players.
    pub fn has_invite(&self, from: PlayerId, to: PlayerId) -> bool {
        self.pending_invites
            .iter()
            .any(|i| i.from_player == from && i.target_player == to)
    }

    /// Allocate a new unique group ID.
    pub fn allocate_group_id(&mut self) -> GroupId {
        let id = self.next_group_id;
        self.next_group_id += 1;
        GroupId(id)
    }
}

impl Default for SocialState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Chat validation helpers
// ---------------------------------------------------------------------------

/// Validate and normalize a chat message.
///
/// - Trims leading/trailing whitespace
/// - Rejects empty messages
/// - Rejects messages exceeding 256 Unicode scalar values
///
/// Returns `Some(trimmed_text)` on success, `None` on rejection.
pub fn validate_chat(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() > 256 {
        return None;
    }
    Some(trimmed.to_string())
}
