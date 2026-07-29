//! Sequential single-in-flight GLB loading pipeline.
//!
//! Builds a 16-entry manifest from the arena models, fox rigging + animations,
//! and world-config resources/creatures. Issues at most one `asset_server.load()`
//! at a time (FIFO) and advances only when the active handle is `Loaded` or
//! `Failed`. After all entries complete, finalizes the typed resource structs
//! (`ArenaAssets`, `FoxAssets`, `CreatureAssets`) using the still-alive cached
//! handles so no cold second wave occurs.

mod queue;
mod systems;
#[cfg(test)]
mod tests;

pub(crate) use queue::SeqLoader;
pub(crate) use systems::{create_loading_queue, poll_and_finalize};
