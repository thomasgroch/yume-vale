use bevy::prelude::*;
use game_core::housing_layout::HOUSING_PLOT_COUNT;

use crate::systems;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Tracks which housing plots are occupied and their decoration entities.
#[derive(Resource)]
pub struct PlotStateResource {
    /// Slot index → PlayerId (u64). None = vacant.
    pub decorations: [Option<u64>; HOUSING_PLOT_COUNT],
    /// Slot index → spawned decoration entity (for cleanup).
    pub decoration_entities: [Option<Entity>; HOUSING_PLOT_COUNT],
    /// Per-slot last build/remove timestamp (cooldown, seconds).
    pub last_build_time: [f64; HOUSING_PLOT_COUNT],
}

impl Default for PlotStateResource {
    fn default() -> Self {
        Self {
            decorations: [None; HOUSING_PLOT_COUNT],
            decoration_entities: [None; HOUSING_PLOT_COUNT],
            last_build_time: [0.0; HOUSING_PLOT_COUNT],
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Server-side housing plugin. Handles plot build/remove intents, spawns/
/// despawns colliders and replicated decoration entities, and persists state.
pub struct ServerHousingPlugin;

impl Plugin for ServerHousingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlotStateResource>();

        app.add_systems(
            FixedUpdate,
            (
                systems::handle_plot_build_intent,
                systems::handle_plot_remove_intent,
            )
                .chain(),
        );
    }
}
