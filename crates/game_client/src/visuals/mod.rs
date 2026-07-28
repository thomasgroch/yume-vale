pub mod bond;
pub mod creatures;
pub mod fox;
pub mod housing;

pub use bond::{ClientBonds, handle_bond_snapshot, show_bond_display, show_feed_prompt};
pub use creatures::{
    CreatureAssets, CreatureVisual, attach_creature_visuals, load_creature_assets,
};
pub use fox::{
    FoxAnimation, FoxAssets, animate_foxes, attach_player_visuals, load_fox_assets,
    mark_local_player_visuals, send_wave_emote, setup_fox_animators, sync_position_to_transform,
    trigger_wave_from_emote,
};
pub use housing::{
    BuildControls, BuildMode, HousingDecoration, PlotBoundary, ProvisionalPreview,
    attach_decoration_visuals, build_controls_ui, handle_action_rejected, spawn_plot_boundaries,
    toggle_build_mode, update_plot_owner_indicators,
};
