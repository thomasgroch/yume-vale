use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use game_core::world_config::CreatureKind;
use game_protocol::components::CreatureState;
use lightyear::prelude::Interpolated;

// ---------------------------------------------------------------------------
// Constants — ground offsets from GLB measurement
// ---------------------------------------------------------------------------

/// Vertical offset so each creature GLB's feet rest on the ground.
/// Measured as -min_y of the model bounding box (positive = lift upward).
const FLUFFBALL_GROUND_Y: f32 = 0.85;
const GLIMMERWING_GROUND_Y: f32 = 0.80;

fn ground_offset(kind: CreatureKind) -> f32 {
    match kind {
        CreatureKind::Fluffball => FLUFFBALL_GROUND_Y,
        CreatureKind::Glimmerwing => GLIMMERWING_GROUND_Y,
    }
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Handles for the two creature GLB models, loaded once at startup.
#[derive(Resource, Clone)]
pub struct CreatureAssets {
    pub(crate) fluffball: Handle<WorldAsset>,
    pub(crate) glimmerwing: Handle<WorldAsset>,
}

impl CreatureAssets {
    fn handle(&self, kind: CreatureKind) -> Handle<WorldAsset> {
        match kind {
            CreatureKind::Fluffball => self.fluffball.clone(),
            CreatureKind::Glimmerwing => self.glimmerwing.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Marker component
// ---------------------------------------------------------------------------

/// Marks an entity as a rendered creature (attached to the replicated entity).
#[derive(Component)]
pub struct CreatureVisual;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

type UnvisualizedCreatures<'w, 's> =
    Query<'w, 's, (Entity, &'static CreatureState), (With<Interpolated>, Without<CreatureVisual>)>;

/// Attaches creature scene pivots to newly replicated entities with
/// [`CreatureState`]. The pivot child carries the GLB scene offset so the
/// model's feet rest on the ground.
pub fn attach_creature_visuals(
    mut commands: Commands,
    assets: Option<Res<CreatureAssets>>,
    creatures: UnvisualizedCreatures,
) {
    let Some(assets) = assets else {
        return;
    };
    for (entity, state) in creatures.iter() {
        let offset = ground_offset(state.kind);
        let pivot = commands
            .spawn((
                Transform::from_xyz(0.0, offset, 0.0),
                Visibility::Inherited,
                WorldAssetRoot(assets.handle(state.kind)),
            ))
            .id();
        commands.entity(entity).add_child(pivot).insert((
            Transform::from_xyz(state.position_x, state.position_y, state.position_z),
            CreatureVisual,
        ));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::world_config::CreatureKind;

    fn creature_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<WorldAsset>();
        app.world_mut().insert_resource(CreatureAssets {
            fluffball: Handle::default(),
            glimmerwing: Handle::default(),
        });
        app.add_systems(Update, attach_creature_visuals);
        app
    }

    #[test]
    fn creature_assets_handle_maps_kinds() {
        let assets = CreatureAssets {
            fluffball: Handle::default(),
            glimmerwing: Handle::default(),
        };
        // Both handles are default (equal), but the method should not panic.
        let _h = assets.handle(CreatureKind::Fluffball);
        let _h = assets.handle(CreatureKind::Glimmerwing);
    }

    #[test]
    fn fluffball_attaches_scene_entity() {
        let mut app = creature_app();
        let entity = app
            .world_mut()
            .spawn((
                CreatureState {
                    creature_id: 1,
                    kind: CreatureKind::Fluffball,
                    position_x: 10.0,
                    position_y: 0.0,
                    position_z: 5.0,
                    target_x: 12.0,
                    target_z: 3.0,
                },
                Interpolated,
            ))
            .id();
        app.update();

        let children = app.world().get::<Children>(entity);
        assert!(
            children.is_some(),
            "creature entity should have a pivot child"
        );
        let has_scene = children
            .unwrap()
            .iter()
            .any(|child| app.world().get::<WorldAssetRoot>(child).is_some());
        assert!(has_scene, "pivot should carry the WorldAssetRoot");
        assert!(
            app.world().get::<CreatureVisual>(entity).is_some(),
            "creature should have CreatureVisual marker"
        );
    }

    #[test]
    fn glimmerwing_attaches_scene_entity() {
        let mut app = creature_app();
        let entity = app
            .world_mut()
            .spawn((
                CreatureState {
                    creature_id: 2,
                    kind: CreatureKind::Glimmerwing,
                    position_x: -5.0,
                    position_y: 0.0,
                    position_z: 15.0,
                    target_x: -3.0,
                    target_z: 13.0,
                },
                Interpolated,
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Children>(entity).is_some(),
            "glimmerwing should get a pivot child"
        );
    }

    #[test]
    fn creature_waits_for_assets() {
        // Without CreatureAssets resource, nothing should attach.
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.add_systems(Update, attach_creature_visuals);
        app.init_asset::<WorldAsset>();
        let entity = app
            .world_mut()
            .spawn((
                CreatureState {
                    creature_id: 1,
                    kind: CreatureKind::Fluffball,
                    position_x: 0.0,
                    position_y: 0.0,
                    position_z: 0.0,
                    target_x: 0.0,
                    target_z: 0.0,
                },
                Interpolated,
            ))
            .id();
        app.update();
        assert!(
            app.world().get::<Children>(entity).is_none(),
            "no visual without CreatureAssets"
        );
    }

    #[test]
    fn ground_offsets_are_positive() {
        for kind in &[CreatureKind::Fluffball, CreatureKind::Glimmerwing] {
            let o = ground_offset(*kind);
            assert!(o > 0.0, "{kind:?} ground offset should lift, got {o}");
        }
    }
}
