use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use game_core::arena::{ArenaModel, arena_layout};

/// Handles to all six arena GLB models, loaded once at startup.
#[derive(Resource, Clone)]
pub struct ArenaAssets {
    /// The portal model handle.
    portal: Handle<WorldAsset>,
    /// The wall model handle.
    wall: Handle<WorldAsset>,
    /// The pillar model handle.
    pillar: Handle<WorldAsset>,
    /// The large crystal model handle.
    crystal_big: Handle<WorldAsset>,
    /// The small crystal model handle.
    pub crystal_small: Handle<WorldAsset>,
    /// The rock model handle.
    rock: Handle<WorldAsset>,
}

impl ArenaAssets {
    /// Returns the scene handle for the given model variant.
    fn handle(&self, model: ArenaModel) -> Handle<WorldAsset> {
        match model {
            ArenaModel::Portal => self.portal.clone(),
            ArenaModel::Wall => self.wall.clone(),
            ArenaModel::Pillar => self.pillar.clone(),
            ArenaModel::CrystalBig => self.crystal_big.clone(),
            ArenaModel::CrystalSmall => self.crystal_small.clone(),
            ArenaModel::Rock => self.rock.clone(),
        }
    }
}

/// Vertical lift so each model's base rests on the ground. The Meshy GLBs
/// have a centered origin (base at -half_height), so without this the props
/// would sink halfway into the floor. Values: measured min_y × layout scale.
fn y_offset(model: ArenaModel) -> f32 {
    match model {
        ArenaModel::Portal => 2.48,       // 0.99 × 2.5
        ArenaModel::Wall => 1.47,         // 0.42 × 3.5
        ArenaModel::Pillar => 2.0,        // 1.00 × 2.0
        ArenaModel::CrystalBig => 1.6,    // 1.00 × 1.6
        ArenaModel::CrystalSmall => 0.44, // 0.80 × 0.55
        ArenaModel::Rock => 0.73,         // 0.91 × 0.8
    }
}

/// Loads the six arena GLB scene models via the asset server and inserts
/// [`ArenaAssets`] as a resource. Should run before [`spawn_arena`] in the
/// Startup schedule (chain them to flush deferred commands in between).
pub fn load_arena_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |model: ArenaModel| -> Handle<WorldAsset> {
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(model.asset_path()))
    };

    commands.insert_resource(ArenaAssets {
        portal: load(ArenaModel::Portal),
        wall: load(ArenaModel::Wall),
        pillar: load(ArenaModel::Pillar),
        crystal_big: load(ArenaModel::CrystalBig),
        crystal_small: load(ArenaModel::CrystalSmall),
        rock: load(ArenaModel::Rock),
    });
}

/// Spawns the arena floor disc and all props from [`arena_layout`].
///
/// The floor is a flat stone-coloured cylinder centred at the origin.
/// Each prop receives its model's scene handle, position, yaw rotation,
/// and uniform scale from the layout definition.
///
/// Must run after [`load_arena_assets`] so [`ArenaAssets`] is available.
pub fn spawn_arena(
    mut commands: Commands,
    assets: Res<ArenaAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Flat stone disc floor under the arena. Its top face sits 2cm below the
    // green ground plane (y=0) — coplanar surfaces would z-fight (flicker).
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(23.5, 0.08))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.60, 0.68),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_translation(Vec3::new(0.0, -0.06, 0.0)),
    ));

    for prop in arena_layout() {
        commands.spawn((
            WorldAssetRoot(assets.handle(prop.model)),
            Transform::from_translation(prop.translation + Vec3::Y * y_offset(prop.model))
                .with_rotation(Quat::from_rotation_y(prop.yaw))
                .with_scale(Vec3::splat(prop.scale)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inserts [`ArenaAssets`] with default (null) handles, suitable for
    /// tests that check entity counts and transforms without loading real
    /// models.
    fn insert_arena_assets(app: &mut App) {
        app.init_asset::<WorldAsset>();
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.world_mut().insert_resource(ArenaAssets {
            portal: Handle::default(),
            wall: Handle::default(),
            pillar: Handle::default(),
            crystal_big: Handle::default(),
            crystal_small: Handle::default(),
            rock: Handle::default(),
        });
    }

    #[test]
    fn spawn_arena_matches_layout_count_and_transforms() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_arena_assets(&mut app);
        app.add_systems(Startup, spawn_arena);
        app.update();

        let layout = arena_layout();
        let mut query = app.world_mut().query::<(&WorldAssetRoot, &Transform)>();
        let spawned: Vec<_> = query.iter(app.world()).collect();

        assert_eq!(
            spawned.len(),
            layout.len(),
            "spawn_arena must create one WorldAssetRoot entity per layout entry"
        );

        for ((_handle, transform), prop) in spawned.iter().zip(layout.iter()) {
            let t = &transform.translation;
            assert!(
                (t.x - prop.translation.x).abs() < 1e-5,
                "x mismatch: got {} expected {}",
                t.x,
                prop.translation.x,
            );
            assert!(
                (t.y - (prop.translation.y + y_offset(prop.model))).abs() < 1e-5,
                "y mismatch: got {} expected {}",
                t.y,
                prop.translation.y + y_offset(prop.model),
            );
            assert!(
                (t.z - prop.translation.z).abs() < 1e-5,
                "z mismatch: got {} expected {}",
                t.z,
                prop.translation.z,
            );
        }
    }

    #[test]
    fn spawn_arena_includes_floor_disc() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        insert_arena_assets(&mut app);
        app.add_systems(Startup, spawn_arena);
        app.update();

        let mut query = app.world_mut().query::<(&Mesh3d, &Transform)>();
        let floors: Vec<_> = query
            .iter(app.world())
            .filter(|(mesh, _)| {
                // The floor disc uses a Cylinder mesh; no other mesh in this
                // system is a cylinder (props use WorldAssetRoot, not Mesh3d).
                let _ = mesh;
                true
            })
            .collect();

        assert_eq!(
            floors.len(),
            1,
            "spawn_arena must spawn exactly one floor disc"
        );
        assert!(
            (floors[0].1.translation.y - (-0.06)).abs() < 1e-5,
            "floor disc must sit at y = -0.06 (2cm below the ground plane avoids z-fighting)",
        );
    }
}
