use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::arena::{ArenaColliderShape, arena_layout};
use game_core::decorations::{DecorationKind, decoration_layout};
pub use game_core::math::horizontal_velocity;
use game_core::player_state::PlayerInput;
use game_protocol::ReplicatedPlayerInput;

use crate::PlayerMovement;

const PLAYER_RADIUS: f32 = 0.35;
const PLAYER_SEGMENT_HEIGHT: f32 = 0.5;
const PLAYER_HALF_HEIGHT: f32 = PLAYER_RADIUS + PLAYER_SEGMENT_HEIGHT * 0.5;
const JUMP_HEIGHT: f32 = 1.5;
const GRAVITY: f32 = 9.81;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JumpLatch(pub bool);

#[derive(Bundle)]
pub struct PlayerPhysicsBundle {
    rigid_body: RigidBody,
    collider: Collider,
    locked_axes: LockedAxes,
    linear_velocity: LinearVelocity,
    angular_velocity: AngularVelocity,
    friction: Friction,
    movement: PlayerMovement,
    replicated_input: ReplicatedPlayerInput,
    jump_latch: JumpLatch,
}

impl Default for PlayerPhysicsBundle {
    fn default() -> Self {
        Self {
            rigid_body: RigidBody::Dynamic,
            collider: Collider::capsule(PLAYER_RADIUS, PLAYER_SEGMENT_HEIGHT),
            locked_axes: LockedAxes::ROTATION_LOCKED,
            linear_velocity: LinearVelocity::ZERO,
            angular_velocity: AngularVelocity::ZERO,
            friction: Friction::new(0.0),
            movement: PlayerMovement::default(),
            replicated_input: ReplicatedPlayerInput(PlayerInput::default()),
            jump_latch: JumpLatch::default(),
        }
    }
}

type MovingPlayers<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static PlayerMovement,
        &'static Position,
        &'static mut LinearVelocity,
        &'static mut JumpLatch,
    ),
    With<RigidBody>,
>;

/// Applies the already-decoded `PlayerMovement` (populated from the client's
/// `ClientInput` message by `apply_client_input`, which runs earlier in
/// `ServerSystems`) to the player's physics velocity.
pub fn apply_predicted_movement(
    time: Res<Time>,
    spatial_query: SpatialQuery,
    mut players: MovingPlayers,
) {
    let dt = time.delta_secs();
    for (entity, movement, position, mut velocity, mut latch) in &mut players {
        let ray_origin = position.0 - Vec3::Y * (PLAYER_HALF_HEIGHT - 0.02);
        let grounded = spatial_query
            .cast_ray(
                ray_origin,
                Dir3::NEG_Y,
                0.08,
                true,
                &SpatialQueryFilter::from_excluded_entities([entity]),
            )
            .is_some();
        let horizontal = horizontal_velocity(
            velocity.0,
            movement.direction,
            movement.running,
            grounded,
            dt,
        );
        velocity.x = horizontal.x;
        velocity.z = horizontal.z;
        if movement.jump && !latch.0 && grounded {
            velocity.y = (2.0 * GRAVITY * JUMP_HEIGHT).sqrt();
        }
        latch.0 = movement.jump;
    }
}

pub fn spawn_static_world_colliders(commands: &mut Commands) {
    commands.spawn((RigidBody::Static, Collider::half_space(Vec3::Y)));
    for prop in arena_layout() {
        let rotation = Quat::from_rotation_y(prop.yaw);
        for collider in prop.colliders {
            let shape = match collider.shape {
                ArenaColliderShape::Cuboid { half_extents } => {
                    Collider::cuboid(half_extents.x, half_extents.y, half_extents.z)
                }
                ArenaColliderShape::Cylinder {
                    radius,
                    half_height,
                } => Collider::cylinder(radius, half_height),
            };
            commands.spawn((
                RigidBody::Static,
                shape,
                Transform::from_translation(prop.translation + rotation * collider.offset)
                    .with_rotation(rotation),
            ));
        }
    }
    for prop in decoration_layout() {
        let (x, z) = (prop.position.x, prop.position.z);
        match prop.kind {
            DecorationKind::Tree => {
                commands.spawn((
                    RigidBody::Static,
                    Collider::cylinder(0.25, 0.8),
                    Transform::from_translation(Vec3::new(x, 0.8, z)),
                ));
            }
            DecorationKind::Rock(scale) => {
                commands.spawn((
                    RigidBody::Static,
                    Collider::sphere(0.6 * scale),
                    Transform::from_translation(Vec3::new(x, 0.3 * scale, z)),
                ));
            }
            DecorationKind::Flower => {}
        }
    }
}
