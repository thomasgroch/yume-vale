use bevy::prelude::*;
use game_protocol::{PlayerColor, PlayerPosition, palette_color};
use lightyear::prelude::Interpolated;
use player::{LocalPlayer, Player};

use crate::connection::LocalPlayerId;

type UnmeshedInterpolatedPlayers<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Player, &'static PlayerColor),
    (With<Interpolated>, Without<Mesh3d>),
>;

fn player_material(base_color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color,
        metallic: 0.1,
        perceptual_roughness: 0.8,
        emissive: LinearRgba::from(base_color) * 0.3,
        ..Default::default()
    }
}

/// Attaches mesh visuals to any player entity that arrives via Lightyear
/// replication (marked with `Interpolated`) but does not yet have a `Mesh3d`.
/// Color comes from the server-assigned `PlayerColor` so every client renders
/// the same player identically; if it has not replicated yet, the entity is
/// retried on later frames via the `Without<Mesh3d>` filter.
pub fn attach_player_visuals(
    mut commands: Commands,
    local_id: Res<LocalPlayerId>,
    players: UnmeshedInterpolatedPlayers,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, player, color) in players.iter() {
        let is_local = local_id.id == Some(player.id);
        info!(
            "player visuals attached for {:?} (local={}, color={})",
            player.id, is_local, color.0
        );

        let base: Color = palette_color(color.0).into();

        let mut entity_cmds = commands.entity(entity);
        entity_cmds.insert((
            Mesh3d(meshes.add(Capsule3d::new(0.4, 1.2))),
            MeshMaterial3d(materials.add(player_material(base))),
        ));

        if is_local {
            entity_cmds.insert(LocalPlayer);
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

/// Copies `PlayerPosition` → `Transform.translation` for interpolated entities.
/// Runs in PostUpdate, chained before `follow_local_player`.
pub fn sync_position_to_transform(
    mut query: Query<(&PlayerPosition, &mut Transform), With<Interpolated>>,
) {
    for (pos, mut transform) in query.iter_mut() {
        transform.translation = pos.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::id::PlayerId;

    #[test]
    fn sync_position_to_transform_copies_translation() {
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
    fn attach_player_visuals_adds_mesh_to_interpolated() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.init_resource::<LocalPlayerId>();
        app.add_systems(Update, attach_player_visuals);

        // Non-local player (no LocalPlayerId match)
        let remote_id = local_player_id(Some(PlayerId::new(1)));
        app.world_mut().insert_resource(remote_id);

        let entity = app
            .world_mut()
            .spawn((
                Player {
                    id: PlayerId::new(2),
                },
                PlayerColor(3),
                Interpolated,
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Mesh3d>(entity).is_some(),
            "remote player should get a mesh"
        );
        assert!(
            app.world().get::<LocalPlayer>(entity).is_none(),
            "non-local player should not get LocalPlayer marker"
        );

        let material = app.world().get::<MeshMaterial3d<StandardMaterial>>(entity);
        let handle = material.expect("player should have a material");
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let mat = materials.get(&handle.0).unwrap();
        let expected: Color = palette_color(3).into();
        assert_eq!(
            mat.base_color, expected,
            "color must come from the replicated PlayerColor"
        );
    }

    #[test]
    fn attach_player_visuals_marks_local_player() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.init_resource::<LocalPlayerId>();
        app.add_systems(Update, attach_player_visuals);

        // Local player (LocalPlayerId matches)
        let local_id = local_player_id(Some(PlayerId::new(1)));
        app.world_mut().insert_resource(local_id);

        let entity = app
            .world_mut()
            .spawn((
                Player {
                    id: PlayerId::new(1),
                },
                PlayerColor(0),
                Interpolated,
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Mesh3d>(entity).is_some(),
            "local player should get a mesh"
        );
        assert!(
            app.world().get::<LocalPlayer>(entity).is_some(),
            "local player should get LocalPlayer marker"
        );
    }

    #[test]
    fn attach_player_visuals_waits_for_player_color() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.init_resource::<LocalPlayerId>();
        app.add_systems(Update, attach_player_visuals);

        let entity = app
            .world_mut()
            .spawn((
                Player {
                    id: PlayerId::new(2),
                },
                Interpolated,
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Mesh3d>(entity).is_none(),
            "mesh must wait for the replicated PlayerColor"
        );

        app.world_mut().entity_mut(entity).insert(PlayerColor(5));
        app.update();

        assert!(
            app.world().get::<Mesh3d>(entity).is_some(),
            "mesh attaches once PlayerColor arrives"
        );
    }

    fn local_player_id(id: Option<PlayerId>) -> LocalPlayerId {
        LocalPlayerId { id }
    }
}
