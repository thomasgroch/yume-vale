use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use game_core::constants::{RUN_SPEED, WALK_SPEED};
use game_protocol::{PlayerColor, PlayerPosition};
use lightyear::prelude::{ConfirmedHistory, Interpolated};
use player::{LocalPlayer, Player};

use crate::connection::LocalPlayerId;

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
}

/// Handles and graph nodes for the fox character, built once at startup.
#[derive(Resource, Clone)]
pub struct FoxAssets {
    scene: Handle<WorldAsset>,
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
}

impl FoxAssets {
    fn node(&self, clip: FoxClip) -> AnimationNodeIndex {
        match clip {
            FoxClip::Idle => self.idle,
            FoxClip::Walk => self.walk,
            FoxClip::Run => self.run,
        }
    }

    fn nodes(&self) -> [AnimationNodeIndex; 3] {
        [self.idle, self.walk, self.run]
    }
}

/// Loads the rigged fox scene and builds the idle/walk/run animation graph.
/// The idle clip comes from the Animation Library (`idle.glb`); walk and run
/// come from the rigging's bundled animations (same skeleton).
pub fn load_fox_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/fox/rigged.glb"));
    let idle_clip =
        asset_server.load(GltfAssetLabel::Animation(0).from_asset("models/fox/idle.glb"));
    let walk_clip =
        asset_server.load(GltfAssetLabel::Animation(0).from_asset("models/fox/walking.glb"));
    let run_clip =
        asset_server.load(GltfAssetLabel::Animation(0).from_asset("models/fox/running.glb"));

    let (graph, indices) = AnimationGraph::from_clips([idle_clip, walk_clip, run_clip]);
    info!("fox assets loaded");
    commands.insert_resource(FoxAssets {
        scene,
        graph: graphs.add(graph),
        idle: indices[0],
        walk: indices[1],
        run: indices[2],
    });
}

/// Per-player animation state: the clip being blended toward, the
/// EMA-smoothed speed, where the entity was last frame, and the
/// scene-spawned `AnimationPlayer`. Weights blend continuously every frame —
/// no crossfade restarts, so rapid clip changes never snap.
#[derive(Component)]
pub struct FoxAnimation {
    current: FoxClip,
    speed: f32,
    prev_translation: Vec3,
    animator: Option<Entity>,
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
    (With<Interpolated>, Without<FoxAnimation>),
>;

/// Attaches the fox scene to any player entity that arrives via Lightyear
/// replication (marked with `Interpolated`) but does not yet have
/// `FoxAnimation`. The scene goes on a pivot child offset down to the ground
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
        }
        if let Ok(mut state) = foxes.get_mut(owner) {
            state.animator = Some(animator);
        }
    }
}

/// Drives each fox from its replicated movement: estimates planar speed from
/// the per-frame translation delta (EMA-smoothed), picks idle/walk/run with
/// hysteresis, blends every clip's weight toward its target each frame (so
/// interrupted fades and rapid changes never snap), and rotates the entity to
/// face its movement direction.
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

        let clip = match state.current {
            FoxClip::Idle if speed > IDLE_SPEED_THRESHOLD + HYSTERESIS => FoxClip::Walk,
            FoxClip::Walk if speed < IDLE_SPEED_THRESHOLD - HYSTERESIS => FoxClip::Idle,
            FoxClip::Walk if speed > RUN_SPEED_THRESHOLD + HYSTERESIS => FoxClip::Run,
            FoxClip::Run if speed < RUN_SPEED_THRESHOLD - HYSTERESIS => FoxClip::Walk,
            c => c,
        };
        state.current = clip;

        if let Some(animator) = state.animator {
            if let Ok(mut player) = anim_players.get_mut(animator) {
                let step = dt / CROSSFADE_SECS;
                for node in fox.nodes() {
                    let target = if node == fox.node(clip) { 1.0 } else { 0.0 };
                    // A never-played clip starts at weight 0 and blends in;
                    // already-playing clips slide toward their target.
                    if player.animation(node).is_none() {
                        player.play(node).repeat().set_weight(0.0);
                    }
                    if let Some(anim) = player.animation_mut(node) {
                        let w = anim.weight();
                        anim.set_weight(w + (target - w).clamp(-step, step));
                    }
                }
            }
        }

        if speed >= IDLE_SPEED_THRESHOLD {
            let yaw = planar.x.atan2(planar.z);
            let target = Quat::from_rotation_y(yaw);
            let t = 1.0 - (-TURN_SMOOTHING * dt).exp();
            transform.rotation = transform.rotation.slerp(target, t);
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

type InterpolatedPlayers<'w, 's> = Query<
    'w,
    's,
    (
        &'static PlayerPosition,
        &'static mut Transform,
        Option<&'static ConfirmedHistory<PlayerPosition>>,
    ),
    With<Interpolated>,
>;

/// Copies `PlayerPosition` → `Transform.translation` for interpolated entities.
/// Right after spawn, lightyear's interpolation timeline is still syncing and
/// the component stays frozen (then teleports when interpolation engages).
/// Detect that state — component unchanged from last frame while newer
/// confirmed data keeps arriving — and snap to the newest confirmed snapshot
/// instead: the fox steps at 30Hz for a few frames rather than freezing.
/// Runs in PostUpdate, chained before `animate_foxes`/`follow_local_player`.
pub fn sync_position_to_transform(mut query: InterpolatedPlayers) {
    for (pos, mut transform, history) in query.iter_mut() {
        let newest = history.and_then(|h| h.newest_present()).map(|(_, p)| p.0);
        transform.translation = match newest {
            Some(n) if pos.0 == transform.translation && (n - pos.0).length() > 1e-4 => n,
            _ => pos.0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;
    use game_core::id::PlayerId;

    fn insert_fox_assets(app: &mut App) {
        app.init_asset::<WorldAsset>();
        app.init_asset::<AnimationClip>();
        app.init_asset::<AnimationGraph>();
        let (graph, indices) =
            AnimationGraph::from_clips([Handle::default(), Handle::default(), Handle::default()]);
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
        app.add_systems(Update, sync_position_to_transform);

        let entity = app
            .world_mut()
            .spawn((
                PlayerPosition(Vec3::new(10.0, 0.0, 20.0)),
                Transform::from_translation(Vec3::ZERO),
                Interpolated,
            ))
            .id();
        app.update();

        let t = app.world().get::<Transform>(entity).unwrap();
        assert!((t.translation.x - 10.0).abs() < 1e-5);
        assert!((t.translation.z - 20.0).abs() < 1e-5);
    }

    #[test]
    fn sync_position_snaps_to_newest_confirmed_while_component_stale() {
        use lightyear::prelude::Tick;

        let mut app = App::new();
        app.add_systems(Update, sync_position_to_transform);

        // Component frozen at spawn (0,0,0) while confirmed data flows in:
        // sync must snap to the newest confirmed position instead of freezing.
        let mut history = ConfirmedHistory::<PlayerPosition>::default();
        history.insert_present(Tick(1), PlayerPosition(Vec3::new(3.0, 0.0, 0.0)));
        let entity = app
            .world_mut()
            .spawn((
                PlayerPosition(Vec3::ZERO),
                Transform::from_translation(Vec3::ZERO),
                Interpolated,
                history,
            ))
            .id();
        app.update();

        let t = app.world().get::<Transform>(entity).unwrap();
        assert!(
            (t.translation.x - 3.0).abs() < 1e-5,
            "stale component should snap to newest confirmed, got {:?}",
            t.translation
        );
    }

    #[test]
    fn sync_position_prefers_component_when_interpolation_engaged() {
        use lightyear::prelude::Tick;

        let mut app = App::new();
        app.add_systems(Update, sync_position_to_transform);

        // Component moved this frame (interpolation engaged): it wins over
        // the newest confirmed anchor.
        let mut history = ConfirmedHistory::<PlayerPosition>::default();
        history.insert_present(Tick(1), PlayerPosition(Vec3::new(3.0, 0.0, 0.0)));
        let entity = app
            .world_mut()
            .spawn((
                PlayerPosition(Vec3::new(1.0, 0.0, 0.0)),
                Transform::from_translation(Vec3::ZERO),
                Interpolated,
                history,
            ))
            .id();
        app.update();

        let t = app.world().get::<Transform>(entity).unwrap();
        assert!(
            (t.translation.x - 1.0).abs() < 1e-5,
            "engaged interpolation value should win, got {:?}",
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
}
