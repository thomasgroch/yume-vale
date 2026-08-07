use avian3d::prelude::*;
use bevy::prelude::*;
use game_core::arena::{ArenaColliderShape, arena_layout};
use game_core::constants::{RUN_SPEED, WALK_SPEED};
use game_core::decorations::{DecorationKind, decoration_layout};
use game_core::math::Direction;
use game_core::player_state::PlayerInput;
use game_protocol::{MovementInput, ReplicatedPlayerInput};
use lightyear::prelude::input::native::ActionState;

use crate::PlayerMovement;

const PLAYER_RADIUS: f32 = 0.35;
const PLAYER_SEGMENT_HEIGHT: f32 = 0.5;
const PLAYER_HALF_HEIGHT: f32 = PLAYER_RADIUS + PLAYER_SEGMENT_HEIGHT * 0.5;
const WALK_ACCELERATION: f32 = 40.0;
const AIR_ACCELERATION: f32 = 10.0;
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

pub fn decode_movement(input: MovementInput) -> Direction {
    let x = input.move_x as f32 / 127.0;
    let z = input.move_z as f32 / 127.0;
    Direction::from_xz(x, z).unwrap_or(Direction::zero())
}

pub fn horizontal_velocity(
    current: Vec3,
    direction: Direction,
    running: bool,
    grounded: bool,
    dt: f32,
) -> Vec3 {
    let speed = if running { RUN_SPEED } else { WALK_SPEED };
    let target = direction.0 * speed;
    let current_horizontal = Vec3::new(current.x, 0.0, current.z);
    let acceleration = if grounded {
        WALK_ACCELERATION
    } else {
        AIR_ACCELERATION
    };
    current_horizontal.move_towards(target, acceleration * dt)
}

type MovingPlayers<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ActionState<MovementInput>,
        &'static Position,
        &'static mut LinearVelocity,
        &'static mut PlayerMovement,
        &'static mut ReplicatedPlayerInput,
        &'static mut JumpLatch,
    ),
    With<RigidBody>,
>;

pub fn apply_predicted_movement(
    time: Res<Time>,
    spatial_query: SpatialQuery,
    mut players: MovingPlayers,
) {
    let dt = time.delta_secs();
    for (entity, input, position, mut velocity, mut movement, mut replicated, mut latch) in
        &mut players
    {
        let direction = decode_movement(input.0);
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
        let horizontal = horizontal_velocity(velocity.0, direction, input.run, grounded, dt);
        velocity.x = horizontal.x;
        velocity.z = horizontal.z;
        if input.jump && !latch.0 && grounded {
            velocity.y = (2.0 * GRAVITY * JUMP_HEIGHT).sqrt();
        }
        latch.0 = input.jump;
        movement.direction = direction;
        movement.running = input.run;
        movement.jump = input.jump;
        replicated.0 = PlayerInput {
            movement: direction,
            run: input.run,
            interact: None,
            action: None,
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_input_decodes_and_normalizes() {
        let direction = decode_movement(MovementInput {
            move_x: 127,
            move_z: 127,
            ..default()
        });
        assert!((direction.0.length() - 1.0).abs() < 1e-5);
        assert!(direction.0.x > 0.0 && direction.0.z > 0.0);
    }

    #[test]
    fn grounded_acceleration_reaches_walk_target() {
        let direction = Direction::from_xz(1.0, 0.0).unwrap();
        let velocity = horizontal_velocity(Vec3::ZERO, direction, false, true, 1.0);
        assert_eq!(velocity, Vec3::X * WALK_SPEED);
    }

    #[test]
    fn air_acceleration_is_lower_than_ground() {
        let direction = Direction::from_xz(1.0, 0.0).unwrap();
        let ground = horizontal_velocity(Vec3::ZERO, direction, true, true, 0.1);
        let air = horizontal_velocity(Vec3::ZERO, direction, true, false, 0.1);
        assert!(ground.length() > air.length());
    }
}
