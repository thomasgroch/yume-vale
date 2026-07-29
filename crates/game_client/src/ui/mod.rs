//! UI design system: theme tokens, reusable widget builders, roster, and emotes.
//!
//! - [`theme`] — colour palette, spacing, radius, font-size tokens
//! - [`widgets`] — builder functions for common UI patterns
//! - [`social`] — client-side emote state
//! - [`roster`] — connected-player count
//!
//! All UI screens (menu, HUD, touch overlays) import from here and never
//! inline raw colour / dimension literals.

pub mod focus;
pub mod roster;
pub mod social;
pub mod theme;
pub mod widgets;

#[cfg(test)]
pub(crate) mod layout_tests;
