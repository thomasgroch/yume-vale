use bevy::animation::graph::{AnimationGraphHandle, AnimationNodeIndex};
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use game_core::constants::{RUN_SPEED, WALK_SPEED};
use game_protocol::{PlayerColor, PlayerPosition};
use lightyear::prelude::{Interpolated, Predicted};
use player::{LocalPlayer, Player};

use crate::connection::LocalPlayerId;

/// Duration of the wave animation clip in seconds.
const WAVE_DURATION: f32 = 2.0;

/// Below this planar speed (m/s) the fox plays the idle clip.
const IDLE_SPEED_THRESHOLD: f32 = 0.5;
/// Speed at which the fox switches between walk and run clips.
const RUN_SPEED_THRESHOLD: f32 = (WALK_SPEED + RUN_SPEED) * 0.5;
/// How fast the fox turns toward its movement direction (higher = snappier).
const TURN_SMOOTHING: f32 = 12.0;
/// Seconds for a clip weight to slide fully from 0→1 (and back).
const CROSSFADE_SECS: f32 = 0.25;
/// EMA rate for the speed estimate (~125ms time constant): keeps residual
/// network jitter from chattering the clip thresholds.
const SPEED_EMA_RATE: f32 = 8.0;
/// Hysteresis band around each threshold so borderline speeds don't flap.
const HYSTERESIS: f32 = 0.3;
/// How fast the rendered transform chases the interpolated position (~55ms
/// time constant). Linear 30Hz interpolation advances in bursts whenever the
/// velocity changes (accelerate/stop/turn) — the chase smooths those bursts
/// into continuous motion.
const RENDER_SMOOTHING: f32 = 18.0;
/// Vertical offset of the fox visual below the entity origin. The physics body
/// sits at float_height (0.6) above the ground, while the fox GLB's origin is
/// at its feet — without this pivot the fox would render 0.6m in the air.
const FOX_GROUND_OFFSET_Y: f32 = -0.6;

/// Which animation clip the fox should be playing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FoxClip {
    Idle,
    Walk,
    Run,
    Wave,
}

/// Handles and graph nodes for the fox character, built once at startup.
#[derive(Resource, Clone)]
pub struct FoxAssets {
    pub(crate) scene: Handle<WorldAsset>,
    pub(crate) graph: Handle<AnimationGraph>,
    pub(crate) idle: AnimationNodeIndex,
    pub(crate) walk: AnimationNodeIndex,
    pub(crate) run: AnimationNodeIndex,
    pub(crate) wave: AnimationNodeIndex,
}

impl FoxAssets {
    fn node(&self, clip: FoxClip) -> AnimationNodeIndex {
        match clip {
            FoxClip::Idle => self.idle,
            FoxClip::Walk => self.walk,
            FoxClip::Run => self.run,
            FoxClip::Wave => self.wave,
        }
    }

    fn nodes(&self) -> [AnimationNodeIndex; 4] {
        [self.idle, self.walk, self.run, self.wave]
    }
}

/// Per-player animation state: the clip being blended toward, the
/// EMA-smoothed speed, where the entity was last frame, the
/// scene-spawned `AnimationPlayer`, and an optional active wave timer.
/// Weights blend continuously every frame — no crossfade restarts, so
/// rapid clip changes never snap.
#[derive(Component)]
pub struct FoxAnimation {
    current: FoxClip,
    speed: f32,
    prev_translation: Vec3,
    animator: Option<Entity>,
    /// Seconds remaining in the wave animation, or `None` if not waving.
    pub(crate) wave_timer: Option<f32>,
}

type UnvisualizedPlayers<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Player,
        &'static PlayerColor,
        &'static PlayerPosition,
    ),
    (
        Or<(With<Interpolated>, With<Predicted>)>,
        Without<FoxAnimation>,
    ),
>;

/// Attaches the fox scene to any player entity that arrives via Lightyear
/// replication — marked `Interpolated` for remote players, or `Predicted`
/// for the local player's own entity (the server sends exactly one of the
/// two per client, never both) — but does not yet have `FoxAnimation`.
/// Without matching both markers here, the local player's own character
/// never gets a mesh, a `Transform`, or the `LocalPlayer` tag the camera
/// needs to follow it.
/// The scene goes on a pivot child offset down to the ground
/// (the entity origin rides at the physics float height). Also inserts the
/// `Transform` that replicated entities lack, seeded from the replicated
/// `PlayerPosition`.
/// Waits for the replicated `PlayerColor` so the entity is fully described
/// before visuals attach; retries on later frames via the
/// `Without<FoxAnimation>` filter while `FoxAssets` or `PlayerColor` are
/// not ready yet.
pub fn attach_player_visuals(
    mut commands: Commands,
    local_id: Res<LocalPlayerId>,
    fox: Option<Res<FoxAssets>>,
    players: UnvisualizedPlayers,
) {
    let Some(fox) = fox else {
        return;
    };
    for (entity, player, _color, position) in players.iter() {
        let is_local = local_id.id == Some(player.id);
        info!(
            "fox visuals attached for {:?} (local={})",
            player.id, is_local
        );

        let pivot = commands
            .spawn((
                Transform::from_xyz(0.0, FOX_GROUND_OFFSET_Y, 0.0),
                Visibility::Inherited,
                WorldAssetRoot(fox.scene.clone()),
            ))
            .id();
        commands.entity(entity).add_child(pivot);

        let mut entity_cmds = commands.entity(entity);
        entity_cmds.insert((
            Transform::from_translation(position.0),
            FoxAnimation {
                current: FoxClip::Idle,
                speed: 0.0,
                prev_translation: position.0,
                animator: None,
                wave_timer: None,
            },
        ));

        if is_local {
            entity_cmds.insert(LocalPlayer);
        }
    }
}

/// The GLB scene spawns its `AnimationPlayer` on a descendant of the player
/// entity once loaded. This system finds newly spawned animators, links them
/// back to their owning player entity, attaches the shared graph, and starts
/// the idle clip.
#[allow(clippy::type_complexity)]
pub fn setup_fox_animators(
    mut commands: Commands,
    fox: Option<Res<FoxAssets>>,
    mut queries: ParamSet<(
        Query<Entity, Added<AnimationPlayer>>,
        Query<&mut AnimationPlayer>,
    )>,
    parents: Query<&ChildOf>,
    mut foxes: Query<&mut FoxAnimation>,
) {
    let Some(fox) = fox else {
        return;
    };
    let new_animators: Vec<Entity> = queries.p0().iter().collect();
    for animator in new_animators {
        let mut current = animator;
        let owner = loop {
            if foxes.contains(current) {
                break Some(current);
            }
            match parents.get(current) {
                Ok(child_of) => current = child_of.parent(),
                Err(_) => break None,
            }
        };
        let Some(owner) = owner else {
            continue;
        };

        commands
            .entity(animator)
            .insert(AnimationGraphHandle(fox.graph.clone()));
        if let Ok(mut player) = queries.p1().get_mut(animator) {
            player.play(fox.idle).repeat();
            player.play(fox.wave).set_weight(0.0);
        }
        if let Ok(mut state) = foxes.get_mut(owner) {
            state.animator = Some(animator);
        }
    }
}

/// Drives each fox from its replicated movement: estimates planar speed from
/// the per-frame translation delta (EMA-smoothed), picks idle/walk/run with
/// hysteresis (or wave when `wave_timer` is active), blends every clip's
/// weight toward its target each frame (so interrupted fades and rapid changes
/// never snap), and rotates the entity to face its movement direction.
/// Runs in PostUpdate, chained after `sync_position_to_transform`.
pub fn animate_foxes(
    time: Res<Time>,
    fox: Option<Res<FoxAssets>>,
    mut foxes: Query<(&mut Transform, &mut FoxAnimation)>,
    mut anim_players: Query<&mut AnimationPlayer>,
) {
    let Some(fox) = fox else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut transform, mut state) in foxes.iter_mut() {
        let delta = transform.translation - state.prev_translation;
        state.prev_translation = transform.translation;
        let planar = Vec3::new(delta.x, 0.0, delta.z);
        let raw_speed = planar.length() / dt;
        let alpha = 1.0 - (-SPEED_EMA_RATE * dt).exp();
        state.speed += (raw_speed - state.speed) * alpha;
        let speed = state.speed;

        // Determine the clip to play.
        // If wave is active, override clip and decrement the timer.
        let active_clip = if let Some(timer) = state.wave_timer.as_mut() {
            *timer -= dt;
            if *timer <= 0.0 {
                state.wave_timer = None;
                // Fall back to speed-based clip selection when wave finishes.
                clip_by_speed(state.current, speed)
            } else {
                FoxClip::Wave
            }
        } else {
            clip_by_speed(state.current, speed)
        };
        state.current = active_clip;

        if let Some(animator) = state.animator {
            if let Ok(mut player) = anim_players.get_mut(animator) {
                let step = dt / CROSSFADE_SECS;
                for node in fox.nodes() {
                    let target = if node == fox.node(active_clip) {
                        1.0
                    } else {
                        0.0
                    };
                    // A never-played clip starts at weight 0 and blends in;
                    // already-playing clips slide toward their target.
                    if player.animation(node).is_none() {
                        // Wave plays once (not repeated); locomotion clips loop.
                        let repeating = node != fox.wave;
                        if repeating {
                            player.play(node).repeat().set_weight(0.0);
                        } else {
                            player.play(node).set_weight(0.0);
                        }
                    }
                    if let Some(anim) = player.animation_mut(node) {
                        let w = anim.weight();
                        anim.set_weight(w + (target - w).clamp(-step, step));
                    }
                }
            }
        }

        if speed >= IDLE_SPEED_THRESHOLD && state.wave_timer.is_none() {
            let yaw = planar.x.atan2(planar.z);
            let target = Quat::from_rotation_y(yaw);
            let t = 1.0 - (-TURN_SMOOTHING * dt).exp();
            transform.rotation = transform.rotation.slerp(target, t);
        }
    }
}

/// Pick the locomotion clip based on EMA speed with hysteresis.
fn clip_by_speed(current: FoxClip, speed: f32) -> FoxClip {
    match current {
        FoxClip::Wave if speed > RUN_SPEED_THRESHOLD => FoxClip::Run,
        FoxClip::Wave if speed > IDLE_SPEED_THRESHOLD => FoxClip::Walk,
        FoxClip::Wave => FoxClip::Idle,
        FoxClip::Idle if speed > IDLE_SPEED_THRESHOLD + HYSTERESIS => FoxClip::Walk,
        FoxClip::Walk if speed < IDLE_SPEED_THRESHOLD - HYSTERESIS => FoxClip::Idle,
        FoxClip::Walk if speed > RUN_SPEED_THRESHOLD + HYSTERESIS => FoxClip::Run,
        FoxClip::Run if speed < RUN_SPEED_THRESHOLD - HYSTERESIS => FoxClip::Walk,
        c => c,
    }
}

use crate::ui::social::ClientEmote;
use game_core::actions::EmoteKind;
use game_protocol::channels::ReliableChannel;
use game_protocol::messages::EmoteIntent;
use lightyear::prelude::MessageSender;

/// Reads `ClientEmote` and triggers the wave animation on the matching
/// player entity. Consumes the pending emote each frame.
pub fn trigger_wave_from_emote(
    mut emote: ResMut<ClientEmote>,
    fox: Option<Res<FoxAssets>>,
    mut foxes: Query<(&Player, &mut FoxAnimation)>,
    mut anim_players: Query<&mut AnimationPlayer>,
    _parents: Query<&ChildOf>,
) {
    let Some(_fox_assets) = fox else { return };
    let Some(broadcast) = emote.pending.take() else {
        return;
    };
    if broadcast.emote != EmoteKind::Wave {
        return;
    }
    for (player, mut state) in &mut foxes {
        if player.id.get() != broadcast.from_player {
            continue;
        }
        state.wave_timer = Some(WAVE_DURATION);
        // Restart the wave clip from the beginning.
        if let Some(animator) = state.animator {
            if let Ok(mut ap) = anim_players.get_mut(animator) {
                ap.play(FoxAssets::node(&_fox_assets, FoxClip::Wave))
                    .set_weight(0.0);
            }
        }
    }
}

/// Listens for the wave key (Z) and sends `EmoteIntent(Wave)` to the server.
pub fn send_wave_emote(
    keys: Res<ButtonInput<KeyCode>>,
    mut senders: Query<&mut MessageSender<EmoteIntent>>,
) {
    if keys.just_pressed(KeyCode::KeyZ) {
        if let Ok(mut sender) = senders.single_mut() {
            sender.send::<ReliableChannel>(EmoteIntent {
                emote: EmoteKind::Wave,
            });
        }
    }
}

type UnmarkedLocalPlayers<'w, 's> =
    Query<'w, 's, (Entity, &'static Player), (With<Interpolated>, Without<LocalPlayer>)>;

/// Retroactively marks the local player entity once `LocalPlayerId` is known,
/// in case the entity replicated before the Welcome message arrived.
pub fn mark_local_player_visuals(
    mut commands: Commands,
    local_id: Res<LocalPlayerId>,
    players: UnmarkedLocalPlayers,
) {
    let Some(my_id) = local_id.id else {
        return;
    };
    for (entity, player) in players.iter() {
        if player.id == my_id {
            commands.entity(entity).insert(LocalPlayer);
        }
    }
}

type InterpolatedPlayers<'w, 's> =
    Query<'w, 's, (&'static PlayerPosition, &'static mut Transform), With<Interpolated>>;

/// Copies `PlayerPosition` → `Transform.translation` for interpolated entities,
/// chasing the interpolated value with an exponential smoothing
/// (`RENDER_SMOOTHING`) instead of snapping. Linear 30Hz interpolation
/// advances in bursts whenever the velocity changes (accelerate/stop/turn),
/// and teleports when the timeline finishes syncing after spawn — the chase
/// absorbs both into continuous motion. On network underruns the fox simply
/// decelerates and resumes smoothly.
/// Runs in PostUpdate, chained before `animate_foxes`/`follow_local_player`.
pub fn sync_position_to_transform(time: Res<Time>, mut query: InterpolatedPlayers) {
    let dt = time.delta_secs();
    let t = 1.0 - (-RENDER_SMOOTHING * dt).exp();
    for (pos, mut transform) in query.iter_mut() {
        let delta = pos.0 - transform.translation;
        transform.translation += delta * t;
    }
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use super::*;
    use bevy::animation::graph::AnimationGraph;
    use core::time::Duration;
    use game_core::id::PlayerId;

    #[test]
    fn module_constants_characterization() {
        assert_eq!(IDLE_SPEED_THRESHOLD, 0.5);
        assert_eq!(RUN_SPEED_THRESHOLD, (WALK_SPEED + RUN_SPEED) * 0.5);
        assert_eq!(TURN_SMOOTHING, 12.0);
        assert_eq!(CROSSFADE_SECS, 0.25);
        assert_eq!(SPEED_EMA_RATE, 8.0);
        assert_eq!(HYSTERESIS, 0.3);
        assert_eq!(RENDER_SMOOTHING, 18.0);
        assert_eq!(FOX_GROUND_OFFSET_Y, -0.6);
        assert_eq!(WAVE_DURATION, 2.0);
    }

    #[test]
    fn fox_assets_constructible() {
        let a = Handle::<AnimationClip>::default();
        let b = Handle::<AnimationClip>::default();
        let c = Handle::<AnimationClip>::default();
        let d = Handle::<AnimationClip>::default();
        let (_graph, indices) = AnimationGraph::from_clips([a, b, c, d]);
        let assets = FoxAssets {
            scene: Handle::default(),
            graph: Handle::default(),
            idle: indices[0],
            walk: indices[1],
            run: indices[2],
            wave: indices[3],
        };
        assert_eq!(assets.node(FoxClip::Idle), indices[0]);
        assert_eq!(assets.node(FoxClip::Wave), indices[3]);
        assert_eq!(
            assets.nodes(),
            [indices[0], indices[1], indices[2], indices[3]]
        );
    }

    #[test]
    fn fox_animation_component_constructible() {
        let mut anim = FoxAnimation {
            current: FoxClip::Idle,
            speed: 0.0,
            prev_translation: Vec3::ZERO,
            animator: None,
            wave_timer: None,
        };
        anim.speed = 5.0;
        assert_eq!(anim.speed, 5.0);
    }

    #[test]
    fn fox_animation_wave_timer_sets_and_clears() {
        let mut anim = FoxAnimation {
            current: FoxClip::Idle,
            speed: 0.0,
            prev_translation: Vec3::ZERO,
            animator: None,
            wave_timer: Some(2.0),
        };
        assert!(anim.wave_timer.is_some());
        anim.wave_timer = None;
        assert!(anim.wave_timer.is_none());
    }

    #[test]
    fn all_public_systems_can_be_registered() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.add_systems(
            Update,
            (
                attach_player_visuals,
                setup_fox_animators,
                animate_foxes,
                mark_local_player_visuals,
                sync_position_to_transform,
            ),
        );
    }

    fn insert_fox_assets(app: &mut App) {
        app.init_asset::<WorldAsset>();
        app.init_asset::<AnimationClip>();
        app.init_asset::<AnimationGraph>();
        let (graph, indices) = AnimationGraph::from_clips([
            Handle::default(),
            Handle::default(),
            Handle::default(),
            Handle::default(),
        ]);
        let graph_handle = app
            .world_mut()
            .resource_mut::<Assets<AnimationGraph>>()
            .add(graph);
        app.world_mut().insert_resource(FoxAssets {
            scene: Handle::default(),
            graph: graph_handle,
            idle: indices[0],
            walk: indices[1],
            run: indices[2],
            wave: indices[3],
        });
    }

    fn attach_app(local_id: Option<PlayerId>) -> App {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.init_resource::<LocalPlayerId>();
        app.world_mut()
            .insert_resource(LocalPlayerId { id: local_id });
        app.add_systems(Update, attach_player_visuals);
        app
    }

    #[test]
    fn animate_foxes_switches_clips_with_speed() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.init_resource::<Time>();
        app.add_systems(Update, animate_foxes);

        let animator = app.world_mut().spawn(AnimationPlayer::default()).id();
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: Some(animator),
                    wave_timer: None,
                },
            ))
            .id();

        // Advance 0.1s without moving: stays idle.
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.1));
        app.update();
        let state = app.world().get::<FoxAnimation>(entity).unwrap();
        assert_eq!(state.current, FoxClip::Idle);

        // Sustain 5 m/s over several frames: the EMA converges and the fox
        // switches to walk (with hysteresis, ~0.3s of sustained speed).
        for i in 1..=8 {
            app.world_mut()
                .entity_mut(entity)
                .get_mut::<Transform>()
                .unwrap()
                .translation = Vec3::new(0.5 * i as f32, 0.0, 0.0);
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(0.1));
            app.update();
        }
        let state = app.world().get::<FoxAnimation>(entity).unwrap();
        assert_eq!(state.current, FoxClip::Walk);

        // Sustain 10 m/s: converges to run; weight blends gradually.
        for i in 1..=8 {
            app.world_mut()
                .entity_mut(entity)
                .get_mut::<Transform>()
                .unwrap()
                .translation = Vec3::new(4.0 + i as f32, 0.0, 0.0);
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(0.1));
            app.update();
            if i == 3 {
                let state = app.world().get::<FoxAnimation>(entity).unwrap();
                assert_eq!(state.current, FoxClip::Run);
                let player = app.world().get::<AnimationPlayer>(animator).unwrap();
                let fox = app.world().resource::<FoxAssets>();
                let run_w = player.animation(fox.run).map(|a| a.weight()).unwrap_or(0.0);
                assert!(
                    (0.0..1.0).contains(&run_w),
                    "run weight should be mid-blend right after switching, got {run_w}"
                );
            }
        }
        let player = app.world().get::<AnimationPlayer>(animator).unwrap();
        let fox = app.world().resource::<FoxAssets>();
        let run_w = player.animation(fox.run).map(|a| a.weight()).unwrap_or(0.0);
        assert!(
            (run_w - 1.0).abs() < 1e-5,
            "run weight should fully converge, got {run_w}"
        );

        // Moving along +X: fox must yaw toward +X (rotation away from identity).
        let t = app.world().get::<Transform>(entity).unwrap();
        assert_ne!(
            t.rotation,
            Quat::IDENTITY,
            "fox should turn toward its movement direction"
        );
    }

    #[test]
    fn sync_position_uses_component_without_history() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, sync_position_to_transform);

        let entity = app
            .world_mut()
            .spawn((
                PlayerPosition(Vec3::new(10.0, 0.0, 20.0)),
                Transform::from_translation(Vec3::ZERO),
                Interpolated,
            ))
            .id();
        // Exponential chase: converge over several 0.1s frames.
        for _ in 0..10 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(0.1));
            app.update();
        }

        let t = app.world().get::<Transform>(entity).unwrap();
        assert!((t.translation.x - 10.0).abs() < 1e-2);
        assert!((t.translation.z - 20.0).abs() < 1e-2);
    }

    #[test]
    fn sync_position_smooths_a_component_teleport() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, sync_position_to_transform);

        // The component jumping 5m in one frame (timeline sync warm-up) must
        // NOT teleport the rendered transform: it chases gradually.
        let entity = app
            .world_mut()
            .spawn((
                PlayerPosition(Vec3::ZERO),
                Transform::from_translation(Vec3::ZERO),
                Interpolated,
            ))
            .id();
        app.world_mut()
            .entity_mut(entity)
            .get_mut::<PlayerPosition>()
            .unwrap()
            .0 = Vec3::new(5.0, 0.0, 0.0);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.016));
        app.update();

        let t = app.world().get::<Transform>(entity).unwrap();
        assert!(
            t.translation.x < 2.5,
            "transform must chase, not teleport: got {:?}",
            t.translation
        );
        for _ in 0..20 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(0.1));
            app.update();
        }
        let t = app.world().get::<Transform>(entity).unwrap();
        assert!(
            (t.translation.x - 5.0).abs() < 1e-2,
            "transform should converge to the component, got {:?}",
            t.translation
        );
    }

    #[test]
    fn attach_player_visuals_adds_fox_scene_to_interpolated() {
        let mut app = attach_app(Some(PlayerId::new(1)));

        let entity = app
            .world_mut()
            .spawn((
                Player {
                    id: PlayerId::new(2),
                },
                PlayerColor(3),
                Interpolated,
                PlayerPosition(Vec3::ZERO),
            ))
            .id();
        app.update();

        let children = app.world().get::<Children>(entity).unwrap();
        assert!(
            children
                .iter()
                .any(|child| app.world().get::<WorldAssetRoot>(child).is_some()),
            "remote player should get the fox scene on a pivot child"
        );
        assert!(
            app.world().get::<FoxAnimation>(entity).is_some(),
            "remote player should get animation state"
        );
        assert!(
            app.world().get::<LocalPlayer>(entity).is_none(),
            "non-local player should not get LocalPlayer marker"
        );
    }

    #[test]
    fn attach_player_visuals_marks_local_player() {
        let mut app = attach_app(Some(PlayerId::new(1)));

        let entity = app
            .world_mut()
            .spawn((
                Player {
                    id: PlayerId::new(1),
                },
                PlayerColor(0),
                Interpolated,
                PlayerPosition(Vec3::ZERO),
            ))
            .id();
        app.update();

        let children = app.world().get::<Children>(entity).unwrap();
        assert!(
            children
                .iter()
                .any(|child| app.world().get::<WorldAssetRoot>(child).is_some()),
            "local player should get the fox scene on a pivot child"
        );
        assert!(
            app.world().get::<LocalPlayer>(entity).is_some(),
            "local player should get LocalPlayer marker"
        );
    }

    /// The real server never sends both markers to the same client — the
    /// owning client gets `Predicted`, everyone else sees that player as
    /// `Interpolated` (see auth.rs's PredictionTarget/InterpolationTarget
    /// split). This regression-tests the actual production shape: without
    /// matching `Predicted` in `UnvisualizedPlayers`, the local player's own
    /// entity — Predicted, never Interpolated — silently never gets a mesh,
    /// a Transform, or the LocalPlayer tag the camera needs.
    #[test]
    fn attach_player_visuals_marks_local_player_when_predicted() {
        let mut app = attach_app(Some(PlayerId::new(1)));

        let entity = app
            .world_mut()
            .spawn((
                Player {
                    id: PlayerId::new(1),
                },
                PlayerColor(0),
                Predicted,
                PlayerPosition(Vec3::ZERO),
            ))
            .id();
        app.update();

        let children = app.world().get::<Children>(entity).unwrap();
        assert!(
            children
                .iter()
                .any(|child| app.world().get::<WorldAssetRoot>(child).is_some()),
            "predicted local player should get the fox scene on a pivot child"
        );
        assert!(
            app.world().get::<LocalPlayer>(entity).is_some(),
            "predicted local player should get LocalPlayer marker"
        );
    }

    #[test]
    fn attach_player_visuals_waits_for_player_color() {
        let mut app = attach_app(Some(PlayerId::new(1)));

        let entity = app
            .world_mut()
            .spawn((
                Player {
                    id: PlayerId::new(2),
                },
                Interpolated,
                PlayerPosition(Vec3::ZERO),
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Children>(entity).is_none(),
            "scene must wait for the replicated PlayerColor"
        );

        app.world_mut().entity_mut(entity).insert(PlayerColor(5));
        app.update();

        let children = app.world().get::<Children>(entity).unwrap();
        assert!(
            children
                .iter()
                .any(|child| app.world().get::<WorldAssetRoot>(child).is_some()),
            "scene attaches once PlayerColor arrives"
        );
    }

    #[test]
    fn setup_fox_animators_links_descendant_animator() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.add_systems(Update, setup_fox_animators);

        let animator = app.world_mut().spawn(AnimationPlayer::default()).id();
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: None,
                    wave_timer: None,
                },
            ))
            .add_child(animator)
            .id();
        app.update();

        let state = app.world().get::<FoxAnimation>(entity).unwrap();
        assert_eq!(
            state.animator,
            Some(animator),
            "descendant AnimationPlayer must be linked to the owning player"
        );
        assert!(
            app.world().get::<AnimationGraphHandle>(animator).is_some(),
            "animator must receive the shared animation graph"
        );
    }

    // ── Wave animation tests ─────────────────────────────────────────────

    #[test]
    fn wave_timer_triggers_wave_clip() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.init_resource::<Time>();
        app.add_systems(Update, animate_foxes);

        let animator = app.world_mut().spawn(AnimationPlayer::default()).id();
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: Some(animator),
                    wave_timer: Some(WAVE_DURATION),
                },
            ))
            .id();

        // First frame: wave timer active → clip should be Wave
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.016));
        app.update();
        let state = app.world().get::<FoxAnimation>(entity).unwrap();
        assert_eq!(
            state.current,
            FoxClip::Wave,
            "wave timer active → wave clip"
        );
    }

    #[test]
    fn wave_transitions_to_locomotion_after_timer() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.init_resource::<Time>();
        app.add_systems(Update, animate_foxes);

        let animator = app.world_mut().spawn(AnimationPlayer::default()).id();
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: Some(animator),
                    wave_timer: Some(0.1), // very short wave
                },
            ))
            .id();

        // Advance past the timer
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(0.05));
            app.update();
        }

        let state = app.world().get::<FoxAnimation>(entity).unwrap();
        assert!(
            state.wave_timer.is_none(),
            "wave timer should be cleared after duration"
        );
        assert_ne!(
            state.current,
            FoxClip::Wave,
            "clip should not be Wave after timer expires"
        );
    }

    #[test]
    fn wave_mid_blend_returns_to_idle_when_standing() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.init_resource::<Time>();
        app.add_systems(Update, animate_foxes);

        let animator = app.world_mut().spawn(AnimationPlayer::default()).id();
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: Some(animator),
                    wave_timer: Some(0.1),
                },
            ))
            .id();

        // Run past the wave timer
        for _ in 0..10 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(0.05));
            app.update();
        }

        // Fox is standing still (speed ~0) → should return to Idle
        let state = app.world().get::<FoxAnimation>(entity).unwrap();
        assert_eq!(state.current, FoxClip::Idle, "standing after wave → idle");
    }

    #[test]
    fn trigger_wave_from_emote_sets_wave_timer() {
        use crate::ui::social::ClientEmote;
        use game_core::actions::EmoteKind;
        use game_protocol::EmoteBroadcast;

        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.init_resource::<ClientEmote>();
        app.add_systems(Update, trigger_wave_from_emote);

        let entity = app
            .world_mut()
            .spawn((
                player::Player {
                    id: game_core::id::PlayerId::new(7),
                },
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: None,
                    wave_timer: None,
                },
            ))
            .id();

        // Inject a wave broadcast for player 7
        app.world_mut().resource_mut::<ClientEmote>().pending = Some(EmoteBroadcast {
            from_player: 7,
            emote: EmoteKind::Wave,
        });

        app.update();

        let state = app.world().get::<FoxAnimation>(entity).unwrap();
        assert!(
            state.wave_timer.is_some(),
            "emote broadcast should set wave_timer"
        );
        let remaining = state.wave_timer.unwrap();
        assert!(
            (remaining - WAVE_DURATION).abs() < 1e-5,
            "wave_timer should be WAVE_DURATION, got {remaining}"
        );
    }

    #[test]
    fn trigger_wave_from_emote_only_matches_correct_player() {
        use crate::ui::social::ClientEmote;
        use game_core::actions::EmoteKind;
        use game_protocol::EmoteBroadcast;

        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.init_resource::<ClientEmote>();
        app.add_systems(Update, trigger_wave_from_emote);

        app.world_mut()
            .spawn((
                player::Player {
                    id: game_core::id::PlayerId::new(1),
                },
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: None,
                    wave_timer: None,
                },
            ))
            .id();
        app.world_mut()
            .spawn((
                player::Player {
                    id: game_core::id::PlayerId::new(2),
                },
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: None,
                    wave_timer: None,
                },
            ))
            .id();

        // Broadcast wave for player 1 only
        app.world_mut().resource_mut::<ClientEmote>().pending = Some(EmoteBroadcast {
            from_player: 1,
            emote: EmoteKind::Wave,
        });

        app.update();

        let mut states: Vec<Option<f32>> = app
            .world_mut()
            .query::<&FoxAnimation>()
            .iter(app.world())
            .map(|a| a.wave_timer)
            .collect();
        states.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        assert_eq!(
            states,
            vec![None, Some(WAVE_DURATION)],
            "only player 1 should have wave_timer set"
        );
    }

    #[test]
    fn trigger_wave_ignores_non_wave_emotes() {
        use crate::ui::social::ClientEmote;
        use game_core::actions::EmoteKind;
        use game_protocol::EmoteBroadcast;

        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_fox_assets(&mut app);
        app.init_resource::<ClientEmote>();
        app.add_systems(Update, trigger_wave_from_emote);

        app.world_mut()
            .spawn((
                player::Player {
                    id: game_core::id::PlayerId::new(3),
                },
                FoxAnimation {
                    current: FoxClip::Idle,
                    speed: 0.0,
                    prev_translation: Vec3::ZERO,
                    animator: None,
                    wave_timer: None,
                },
            ))
            .id();

        // Send a Dance emote (not Wave)
        app.world_mut().resource_mut::<ClientEmote>().pending = Some(EmoteBroadcast {
            from_player: 3,
            emote: EmoteKind::Dance,
        });

        app.update();

        let state = app
            .world_mut()
            .query::<&FoxAnimation>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!(
            state.wave_timer.is_none(),
            "non-wave emotes must not set wave_timer"
        );
    }

    #[test]
    fn sync_position_converges_monotonically_for_local_player() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, sync_position_to_transform);

        // Local player has both Interpolated (from replication) and LocalPlayer.
        // PlayerPosition is at target (10, 0, 0), Transform starts at origin.
        let target = Vec3::new(10.0, 0.0, 0.0);
        let entity = app
            .world_mut()
            .spawn((
                LocalPlayer,
                PlayerPosition(target),
                Transform::from_translation(Vec3::ZERO),
                Interpolated,
            ))
            .id();

        let mut prev_x = 0.0f32;
        // Advance 20 frames at 0.1s each — should converge monotonically.
        for _ in 0..20 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(0.1));
            app.update();

            let t = app.world().get::<Transform>(entity).unwrap();
            let x = t.translation.x;
            // Must always move toward target without overshooting.
            assert!(
                x >= prev_x,
                "translation must increase monotonically toward {}: prev={}, cur={}",
                target.x,
                prev_x,
                x
            );
            assert!(
                x <= target.x,
                "translation must not overshoot target {}: got {}",
                target.x,
                x
            );
            prev_x = x;
        }
        // After 20 frames it should be within 1% of target.
        let t = app.world().get::<Transform>(entity).unwrap();
        assert!(
            (t.translation.x - target.x).abs() < 0.1,
            "expected ~{}, got {:?}",
            target.x,
            t.translation
        );
    }
}
