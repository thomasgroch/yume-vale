use bevy::asset::{AssetServer, LoadState, UntypedHandle};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;

use game_core::arena::ArenaModel;
use game_core::world_config::WorldConfig;

// ---------------------------------------------------------------------------
// Canonical path constants
// ---------------------------------------------------------------------------

/// Single source of truth for all GLB asset paths that the loader issues.
pub(crate) mod paths {
    pub(crate) const FOX_RIG: &str = "models/fox/rigged.glb";
    pub(crate) const FOX_IDLE: &str = "models/fox/idle.glb";
    pub(crate) const FOX_WALK: &str = "models/fox/walking.glb";
    pub(crate) const FOX_RUN: &str = "models/fox/running.glb";
    pub(crate) const FOX_WAVE: &str = "models/fox/wave.glb";
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Discriminator for GLB sub-asset labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LabelKind {
    Scene,
    Animation,
}

/// A single item in the load manifest.
#[derive(Debug, Clone)]
pub(crate) struct ManifestEntry {
    pub(crate) path: String,
    pub(crate) label: LabelKind,
}

/// The single in-flight load request (at most one at any time).
pub(crate) struct ActiveLoad {
    pub(crate) path: String,
    pub(crate) handle: UntypedHandle,
}

// ---------------------------------------------------------------------------
// SeqLoader — sequential loader state machine
// ---------------------------------------------------------------------------

/// Tracks the queue, the single active request, completed items, and error.
#[derive(Resource)]
pub(crate) struct SeqLoader {
    pub(crate) queue: Vec<ManifestEntry>,
    pub(crate) active: Option<ActiveLoad>,
    /// Completed item handles — strong refs keep assets alive for cache re-use.
    pub(crate) completed: Vec<UntypedHandle>,
    /// How many items have reached `Loaded`.
    pub(crate) progress: usize,
    /// Total items in the manifest.
    pub(crate) total: usize,
    /// Set when an item enters `Failed` — freezes further advancement.
    pub(crate) failing_path: Option<String>,
}

impl SeqLoader {
    /// Build the manifest from parsed world config.
    pub(crate) fn from_config(config: &WorldConfig) -> Self {
        let mut queue = Vec::with_capacity(16);

        // 6 arena models — Scene(0)
        for model in &[
            ArenaModel::Portal,
            ArenaModel::Wall,
            ArenaModel::Pillar,
            ArenaModel::CrystalBig,
            ArenaModel::CrystalSmall,
            ArenaModel::Rock,
        ] {
            queue.push(ManifestEntry {
                path: model.asset_path().to_string(),
                label: LabelKind::Scene,
            });
        }

        // Fox rigged — Scene(0)
        queue.push(ManifestEntry {
            path: paths::FOX_RIG.to_string(),
            label: LabelKind::Scene,
        });

        // Resources from config — Scene(0), strip `assets/` prefix
        for res in &config.resources {
            queue.push(ManifestEntry {
                path: res
                    .model_path
                    .strip_prefix("assets/")
                    .unwrap_or(&res.model_path)
                    .to_string(),
                label: LabelKind::Scene,
            });
        }

        // Creatures from config — Scene(0), strip `assets/` prefix
        for creature in &config.creatures {
            queue.push(ManifestEntry {
                path: creature
                    .model_path
                    .strip_prefix("assets/")
                    .unwrap_or(&creature.model_path)
                    .to_string(),
                label: LabelKind::Scene,
            });
        }

        // Fox animations — Animation(0)
        for anim in &[
            paths::FOX_IDLE,
            paths::FOX_WALK,
            paths::FOX_RUN,
            paths::FOX_WAVE,
        ] {
            queue.push(ManifestEntry {
                path: anim.to_string(),
                label: LabelKind::Animation,
            });
        }

        let total = queue.len();
        SeqLoader {
            queue,
            active: None,
            completed: Vec::with_capacity(total),
            progress: 0,
            total,
            failing_path: None,
        }
    }

    /// True when every item has been processed (all loaded or failed).
    pub(crate) fn is_finished(&self) -> bool {
        self.queue.is_empty() && self.active.is_none() && self.progress == self.total
    }

    /// True when all items completed without failure.
    pub(crate) fn all_loaded(&self) -> bool {
        self.is_finished() && self.failing_path.is_none()
    }

    /// Number of completed items (loaded successfully).
    pub(crate) fn loaded_count(&self) -> usize {
        self.progress
    }

    /// True if there is capacity to start a new load.
    pub(crate) fn can_start_next(&self) -> bool {
        self.active.is_none() && !self.queue.is_empty() && self.failing_path.is_none()
    }

    /// Attempt to start the next queued load — no-op if one is already active
    /// or the queue is empty. Returns true iff a new load was started.
    pub(crate) fn try_start_next(&mut self, server: &AssetServer) -> bool {
        if !self.can_start_next() {
            return false;
        }
        // Snapshot before `remove(0)` — from_asset needs 'static lifetime.
        let path = self.queue[0].path.clone();
        let label = self.queue[0].label;
        self.queue.remove(0);
        let handle = match label {
            LabelKind::Scene => {
                let h: Handle<WorldAsset> =
                    server.load(GltfAssetLabel::Scene(0).from_asset(path.clone()));
                h.untyped()
            }
            LabelKind::Animation => {
                let h: Handle<AnimationClip> =
                    server.load(GltfAssetLabel::Animation(0).from_asset(path.clone()));
                h.untyped()
            }
        };
        self.active = Some(ActiveLoad { path, handle });
        true
    }

    /// Poll the active handle and record the outcome.
    ///
    /// Returns `Ok(true)` if loaded (progress advanced), `Ok(false)` if still
    /// pending, `Err(path)` if failed.
    pub(crate) fn poll_active(&mut self, server: &AssetServer) -> Result<bool, String> {
        let (id, path, handle) = match self.active.as_ref() {
            Some(a) => (a.handle.id(), a.path.clone(), a.handle.clone()),
            None => return Ok(false),
        };

        match server.load_state(id) {
            LoadState::Loaded => {
                self.active = None;
                self.completed.push(handle);
                self.progress += 1;
                Ok(true)
            }
            LoadState::Failed(_) => {
                self.active = None;
                self.failing_path = Some(path.clone());
                Err(path)
            }
            _ => Ok(false),
        }
    }
}

// ---------------------------------------------------------------------------
// Test helpers (no AssetServer needed)
// ---------------------------------------------------------------------------

impl SeqLoader {
    /// Test-only: mark the active load as completed without checking the real
    /// asset state. Useful for testing the state-machine transitions.
    #[cfg(test)]
    pub(crate) fn mark_active_completed(&mut self) {
        if let Some(active) = self.active.take() {
            self.completed.push(active.handle);
            self.progress += 1;
        }
    }

    /// Test-only: mark the active load as failed.
    #[cfg(test)]
    pub(crate) fn mark_active_failed(&mut self) {
        if let Some(active) = self.active.take() {
            self.failing_path = Some(active.path);
        }
    }
}
