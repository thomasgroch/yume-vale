use bevy::prelude::*;

/// Marker for decorative world objects (trees, rocks, flowers).
#[derive(Component)]
pub struct Decoration;

/// Spawns decorative objects scattered around the world so the player can
/// perceive movement via parallax against fixed reference points.
pub fn spawn_decorations(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let trunk_mesh = meshes.add(Cylinder::new(0.25, 1.6));
    let canopy_mesh = meshes.add(Sphere::new(1.1));
    let rock_mesh = meshes.add(Sphere::new(0.6));
    let flower_mesh = meshes.add(Sphere::new(0.2));

    let trunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.22, 0.10),
        perceptual_roughness: 0.9,
        ..default()
    });
    let canopy_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.62, 0.20),
        perceptual_roughness: 0.8,
        ..default()
    });
    let rock_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.50, 0.50, 0.55),
        perceptual_roughness: 0.9,
        metallic: 0.05,
        ..default()
    });
    let flower_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.30, 0.60),
        emissive: Color::srgb(0.6, 0.1, 0.3).to_linear() * 0.6,
        perceptual_roughness: 0.5,
        ..default()
    });

    // Deterministic pseudo-random scatter (no rand dependency).
    let mut seed: u32 = 0x9E37_79B9;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / 16_777_216.0
    };

    for _ in 0..40 {
        let x = (next() - 0.5) * 80.0;
        let z = (next() - 0.5) * 80.0;
        // Keep spawn area clear.
        if x.abs() < 3.0 && z.abs() < 3.0 {
            continue;
        }

        match (next() * 10.0) as u32 {
            0..=4 => {
                // Tree: trunk + canopy.
                commands.spawn((
                    Mesh3d(trunk_mesh.clone()),
                    MeshMaterial3d(trunk_mat.clone()),
                    Transform::from_translation(Vec3::new(x, 0.8, z)),
                    Decoration,
                ));
                commands.spawn((
                    Mesh3d(canopy_mesh.clone()),
                    MeshMaterial3d(canopy_mat.clone()),
                    Transform::from_translation(Vec3::new(x, 2.4, z)),
                    Decoration,
                ));
            }
            5..=7 => {
                // Rock.
                let s = 0.6 + next() * 0.8;
                commands.spawn((
                    Mesh3d(rock_mesh.clone()),
                    MeshMaterial3d(rock_mat.clone()),
                    Transform::from_translation(Vec3::new(x, 0.3 * s, z))
                        .with_scale(Vec3::splat(s)),
                    Decoration,
                ));
            }
            _ => {
                // Flower.
                commands.spawn((
                    Mesh3d(flower_mesh.clone()),
                    MeshMaterial3d(flower_mat.clone()),
                    Transform::from_translation(Vec3::new(x, 0.4, z)),
                    Decoration,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorations_spawn() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_systems(Startup, spawn_decorations);
        app.update();

        let mut query = app.world_mut().query::<&Decoration>();
        assert!(query.iter(app.world()).count() > 10);
    }
}
