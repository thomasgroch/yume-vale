pub mod plugin;
pub mod state;
pub mod systems;

pub use plugin::*;
pub use state::*;
pub use systems::*;

#[cfg(test)]
mod tests {
    use super::state::*;
    use game_core::id::PlayerId;

    // -----------------------------------------------------------------------
    // Chat validation
    // -----------------------------------------------------------------------

    #[test]
    fn validate_chat_normal() {
        assert_eq!(validate_chat("hello").unwrap(), "hello");
    }

    #[test]
    fn validate_chat_trims_whitespace() {
        assert_eq!(validate_chat("  hello world  ").unwrap(), "hello world");
    }

    #[test]
    fn validate_chat_rejects_empty() {
        assert!(validate_chat("").is_none());
    }

    #[test]
    fn validate_chat_rejects_whitespace_only() {
        assert!(validate_chat("   ").is_none());
    }

    #[test]
    fn validate_chat_rejects_overflow() {
        let long = "a".repeat(257);
        assert!(validate_chat(&long).is_none());
    }

    #[test]
    fn validate_chat_accepts_max_length() {
        let just_right = "a".repeat(256);
        assert!(validate_chat(&just_right).is_some());
    }

    #[test]
    fn validate_chat_rejects_unicode_overflow() {
        // 257 multi-byte Unicode scalars (each is 3 bytes in UTF-8)
        let long: String = std::iter::repeat_n('\u{00e9}', 257).collect();
        assert!(validate_chat(&long).is_none());
    }

    #[test]
    fn validate_chat_accepts_unicode_max() {
        let just_right: String = std::iter::repeat_n('\u{00e9}', 256).collect();
        assert!(validate_chat(&just_right).is_some());
    }

    // -----------------------------------------------------------------------
    // SocialState: invites
    // -----------------------------------------------------------------------

    #[test]
    fn add_invite_normal() {
        let mut state = SocialState::new();
        let a = PlayerId::new(1);
        let b = PlayerId::new(2);
        assert!(state.add_invite(a, b).is_ok());
        assert!(state.has_invite(a, b));
    }

    #[test]
    fn add_invite_rejects_duplicate() {
        let mut state = SocialState::new();
        let a = PlayerId::new(1);
        let b = PlayerId::new(2);
        state.add_invite(a, b).unwrap();
        assert_eq!(
            state.add_invite(a, b),
            Err(SocialError::InviteAlreadyExists)
        );
    }

    #[test]
    fn add_invite_rejects_self_invite() {
        let mut state = SocialState::new();
        let a = PlayerId::new(1);
        assert_eq!(state.add_invite(a, a), Err(SocialError::SelfInvite));
    }

    #[test]
    fn consume_invite_for_returns_oldest_first() {
        let mut state = SocialState::new();
        let a = PlayerId::new(1);
        let b = PlayerId::new(2);
        let c = PlayerId::new(3);
        state.add_invite(a, c).unwrap();
        state.add_invite(b, c).unwrap();

        // Consume for c: should return the oldest (a -> c)
        let invite = state.consume_invite_for(c).unwrap();
        assert_eq!(invite.from_player, a);
        assert_eq!(invite.target_player, c);

        // Second consume should return b -> c
        let invite = state.consume_invite_for(c).unwrap();
        assert_eq!(invite.from_player, b);

        // No more invites
        assert!(state.consume_invite_for(c).is_none());
    }

    #[test]
    fn remove_all_for_player_clears_both_directions() {
        let mut state = SocialState::new();
        let a = PlayerId::new(1);
        let b = PlayerId::new(2);
        let c = PlayerId::new(3);
        state.add_invite(a, b).unwrap();
        state.add_invite(b, a).unwrap();
        state.add_invite(a, c).unwrap();

        state.remove_all_for_player(a);
        assert!(!state.has_invite(a, b));
        assert!(!state.has_invite(b, a));
        assert!(!state.has_invite(a, c));
        assert_eq!(state.pending_invites.len(), 0);
    }

    // -----------------------------------------------------------------------
    // SocialState: group ID allocation
    // -----------------------------------------------------------------------

    #[test]
    fn allocate_group_id_increments() {
        let mut state = SocialState::new();
        let id1 = state.allocate_group_id();
        let id2 = state.allocate_group_id();
        assert_ne!(id1, id2);
    }
}
