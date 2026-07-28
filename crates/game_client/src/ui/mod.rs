//! UI design system: theme tokens, reusable widget builders, and social
//! panels (chat, roster, group).
//!
//! - [`theme`] — colour palette, spacing, radius, font-size tokens
//! - [`widgets`] — builder functions for common UI patterns
//! - [`social`] — client-side social state resources (chat, group, emotes)
//! - [`chat`] — collapsible chat panel with text input
//! - [`roster`] — connected-player/group panel with invite controls
//!
//! All UI screens (menu, HUD, touch overlays) import from here and never
//! inline raw colour / dimension literals.

pub mod chat;
pub mod focus;
pub mod roster;
pub mod social;
pub mod theme;
pub mod widgets;

#[cfg(test)]
pub(crate) mod layout_tests;
