//! Headless rigid-body contracts; backend handles never escape this module.
#[cfg(feature = "physics")]
use crate::{EntityId, World};
use crate::{Transform, Vec3, WorldError};
use bevy_ecs::component::Component;
use serde::{Deserialize, Serialize};
use std::fmt;
#[cfg(feature = "physics")]
use std::time::Duration;

/// The physics world's current fixed gravity, in metres per second squared.
pub const PHYSICS_GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

/// Copied, committed dynamic-body observation; never a backend handle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsBodyState {
    pub transform: Transform,
    pub linear_velocity: Vec3,
    /// World-space radians per second.
    pub angular_velocity: Vec3,
    pub mass: f32,
    pub gravity: Vec3,
    /// World-local reconstruction change token, not durable identity. Compare only
    /// for the same EntityId in the same World; reconstruction resets control history.
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BodyKind {
    Static,
    #[default]
    Dynamic,
}

/// Local-space cuboid. Scale is baked into its dimensions during synchronization.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BoxCollider {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
}

/// Static height-field collision from the same versioned recipe as its render mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainCollider {
    pub settings: crate::TerrainSettings,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Collider {
    Box(BoxCollider),
    Terrain(TerrainCollider),
}
impl From<BoxCollider> for Collider {
    fn from(value: BoxCollider) -> Self {
        Self::Box(value)
    }
}
impl From<TerrainCollider> for Collider {
    fn from(value: TerrainCollider) -> Self {
        Self::Terrain(value)
    }
}
impl Collider {
    pub fn as_box(self) -> Option<BoxCollider> {
        match self {
            Self::Box(value) => Some(value),
            Self::Terrain(_) => None,
        }
    }
    fn bounds(self) -> BoxCollider {
        match self {
            Self::Box(value) => value,
            Self::Terrain(value) => BoxCollider {
                center: [0., value.settings.height as f32 / 2., 0.],
                half_extents: [
                    value.settings.size as f32 / 2.,
                    (value.settings.height as f32 / 2.).max(0.01),
                    value.settings.size as f32 / 2.,
                ],
            },
        }
    }
}

#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct PhysicsBody {
    pub body: BodyKind,
    pub collider: Collider,
    pub gravity_scale: f32,
    pub mass: f32,
    pub friction: f32,
    pub restitution: f32,
}
impl PhysicsBody {
    pub fn new(body: BodyKind, collider: impl Into<Collider>) -> Result<Self, PhysicsError> {
        let result = Self {
            body,
            collider: collider.into(),
            gravity_scale: 1.0,
            mass: 1.0,
            friction: 0.5,
            restitution: 0.0,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn validate(self) -> Result<(), PhysicsError> {
        if let Collider::Terrain(terrain) = self.collider {
            if self.body != BodyKind::Static || terrain.settings.validate().is_err() {
                return Err(PhysicsError::InvalidSettings);
            }
        }
        let bounds = self.collider.bounds();
        if !bounds
            .center
            .iter()
            .all(|n| n.is_finite() && n.abs() <= 1_000_000.)
            || !bounds
                .half_extents
                .iter()
                .all(|n| n.is_finite() && *n >= 0.0001 && *n <= 1_000_000.)
            || !self.gravity_scale.is_finite()
            || !(0.0..=100.0).contains(&self.gravity_scale)
            || !self.mass.is_finite()
            || !(0.0001..=1_000_000.0).contains(&self.mass)
            || !self.friction.is_finite()
            || !(0.0..=10.0).contains(&self.friction)
            || !self.restitution.is_finite()
            || !(0.0..=1.0).contains(&self.restitution)
        {
            return Err(PhysicsError::InvalidSettings);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicsError {
    InvalidSettings,
    InvalidForce,
    InvalidTorque,
    BodyRevisionExhausted,
    LocomotionConflict,
    PhysicalLocomotionRequiresDynamicBody,
    AircraftBody,
    NotDynamicBody,
    FeatureDisabled,
    UnsupportedTransform,
    UnsupportedParent,
    TransformConflict,
    InvalidTimestep,
    World(WorldError),
}
impl From<WorldError> for PhysicsError {
    fn from(e: WorldError) -> Self {
        Self::World(e)
    }
}
impl fmt::Display for PhysicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSettings => "physics requires finite bounded settings, positive dimensions/mass, and static terrain colliders",
            Self::InvalidForce => "physics force must be finite and its accumulated value must remain finite",
            Self::InvalidTorque => "physics torque must be finite and its accumulated value must remain finite",
            Self::BodyRevisionExhausted => "physics body reconstruction revision exhausted",
            Self::LocomotionConflict => "locomotion conflicts with another pose or velocity writer; physical bodies are moved only by physics",
            Self::PhysicalLocomotionRequiresDynamicBody => "physical locomotion (drone_physics, fixed_wing_physics) requires an enabled dynamic PhysicsBody",
            Self::AircraftBody => "fixed_wing_physics needs a box collider with friction 0: the pilot models the wheels",
            Self::NotDynamicBody => "forces and torques can only be applied to entities with a dynamic physics body",
            Self::FeatureDisabled => "physics force and torque commands require the optional physics feature",
            Self::UnsupportedTransform => "physics requires finite positive-scale TRS without shear",
            Self::UnsupportedParent => "dynamic bodies must be scene roots, optionally under an identity scene container without physics",
            Self::TransformConflict => "gameplay changed a dynamic body's transform; physics owns its pose",
            Self::InvalidTimestep => "physics timestep must be finite and in (0, 0.1] seconds",
            Self::World(_) => "physics entity/world mapping is invalid",
        })
    }
}
impl std::error::Error for PhysicsError {}

/// Shared validation for authored candidates and runtime synchronization.
pub(crate) fn validate_pose(
    body: PhysicsBody,
    matrix: crate::Mat4,
) -> Result<Transform, PhysicsError> {
    body.validate()?;
    let t = Transform::try_from_matrix(matrix).map_err(|_| PhysicsError::UnsupportedTransform)?;
    let half = Vec3::from_array(body.collider.bounds().half_extents) * t.scale;
    let center = Vec3::from_array(body.collider.bounds().center) * t.scale;
    if t.scale.min_element() <= 0.0
        || !half.is_finite()
        || half.min_element() < 0.0001
        || half.max_element() > 1_000_000.0
        || !center.is_finite()
        || center.abs().max_element() > 1_000_000.0
        || t.translation.abs().max_element() > 1_000_000.0
    {
        return Err(PhysicsError::UnsupportedTransform);
    }
    Ok(t)
}

#[cfg(feature = "physics")]
pub(crate) mod runtime {
    use super::*;
    use rapier3d::math::{Pose, Rotation, Vector};
    use rapier3d::prelude::{ColliderBuilder, PhysicsWorld, RigidBodyBuilder, RigidBodyHandle};
    use std::collections::HashMap;

    struct Entry {
        handle: RigidBodyHandle,
        settings: PhysicsBody,
        scale: Vec3,
        last_local: Transform,
        revision: u64,
    }
    #[derive(Default)]
    pub(crate) struct PhysicsState {
        backend: PhysicsWorld,
        entries: HashMap<EntityId, Entry>,
        pub(crate) inputs: Vec<(EntityId, PhysicsBody)>,
        pending_forces: HashMap<EntityId, Vec3>,
        pending_torques: HashMap<EntityId, Vec3>,
        last_revision: u64,
        /// Target horizontal velocity, jump speed and drive force in newtons (0 sets velocity directly).
        pending_drives: HashMap<EntityId, (Vec3, Option<f32>, f32)>,
    }
    impl PhysicsState {
        pub(crate) fn grounded(&self, id: EntityId) -> bool {
            let Some(entry) = self.entries.get(&id) else {
                return false;
            };
            let body = &self.backend.bodies[entry.handle];
            body.colliders().iter().any(|&collider| {
                self.backend.contact_pairs_with(collider).any(|pair| {
                    pair.has_any_active_contact()
                        && pair.manifolds().iter().any(|m| {
                            let up = if pair.collider1 == collider {
                                -m.data.normal.y
                            } else {
                                m.data.normal.y
                            };
                            up >= 0.5 && m.points.iter().any(|p| p.dist <= 0.02)
                        })
                })
            })
        }

        pub(crate) fn queue_force(
            &mut self,
            id: EntityId,
            force: Vec3,
        ) -> Result<(), PhysicsError> {
            if !force.is_finite() {
                return Err(PhysicsError::InvalidForce);
            }
            let accumulated = self.pending_forces.get(&id).copied().unwrap_or(Vec3::ZERO) + force;
            if !accumulated.is_finite() {
                return Err(PhysicsError::InvalidForce);
            }
            self.pending_forces.insert(id, accumulated);
            Ok(())
        }
        pub(crate) fn queue_drive(
            &mut self,
            id: EntityId,
            velocity: Vec3,
            jump: Option<f32>,
            force: f32,
        ) -> Result<(), PhysicsError> {
            if !velocity.is_finite()
                || jump.is_some_and(|j| !j.is_finite())
                || !force.is_finite()
                || force < 0.0
            {
                return Err(PhysicsError::InvalidForce);
            }
            self.pending_drives.insert(id, (velocity, jump, force));
            Ok(())
        }
        pub(crate) fn queue_torque(
            &mut self,
            id: EntityId,
            torque: Vec3,
        ) -> Result<(), PhysicsError> {
            if !torque.is_finite() {
                return Err(PhysicsError::InvalidTorque);
            }
            let accumulated = self.pending_torques.get(&id).copied().unwrap_or(Vec3::ZERO) + torque;
            if !accumulated.is_finite() {
                return Err(PhysicsError::InvalidTorque);
            }
            self.pending_torques.insert(id, accumulated);
            Ok(())
        }
        /// First static surface straight below `origin` within `length`: point and normal.
        /// Dynamic bodies (including the character's own) are ignored.
        pub(crate) fn ground_ray(&self, origin: Vec3, length: f32) -> Option<(Vec3, Vec3)> {
            let ray = rapier3d::prelude::Ray::new(
                Vector::from_array(origin.to_array()),
                Vector::new(0.0, -1.0, 0.0),
            );
            let (_, hit) = self.backend.cast_ray_and_get_normal(
                &ray,
                length,
                true,
                rapier3d::prelude::QueryFilter::exclude_dynamic(),
            )?;
            Some((
                origin - Vec3::Y * hit.time_of_impact,
                Vec3::from_array(hit.normal.to_array()),
            ))
        }
        pub(crate) fn linear_velocity(&self, id: EntityId) -> Option<Vec3> {
            let entry = self.entries.get(&id)?;
            Some(Vec3::from_array(
                self.backend.bodies[entry.handle].linvel().to_array(),
            ))
        }
        pub(crate) fn angular_velocity(&self, id: EntityId) -> Option<Vec3> {
            let entry = self.entries.get(&id)?;
            Some(Vec3::from_array(
                self.backend.bodies[entry.handle].angvel().to_array(),
            ))
        }
        pub(crate) fn body_state(
            &self,
            id: EntityId,
            settings: PhysicsBody,
            local: Transform,
        ) -> Option<PhysicsBodyState> {
            let entry = self.entries.get(&id)?;
            if settings.body != BodyKind::Dynamic
                || entry.settings != settings
                || entry.last_local != local
            {
                return None;
            }
            let body = &self.backend.bodies[entry.handle];
            Some(PhysicsBodyState {
                transform: entry.last_local,
                linear_velocity: Vec3::from_array(body.linvel().to_array()),
                angular_velocity: Vec3::from_array(body.angvel().to_array()),
                mass: body.mass(),
                gravity: PHYSICS_GRAVITY * body.gravity_scale(),
                revision: entry.revision,
            })
        }
    }
    fn pose(t: Transform) -> Pose {
        Pose::from_parts(
            Vector::from_array(t.translation.to_array()),
            Rotation::from_xyzw(t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w),
        )
    }
    pub(super) fn terrain_builder(
        terrain: TerrainCollider,
        scale: Vec3,
    ) -> Result<ColliderBuilder, PhysicsError> {
        let field = terrain
            .settings
            .generate()
            .map_err(|_| PhysicsError::InvalidSettings)?;
        terrain_field_builder(terrain, scale, &field)
    }
    pub(super) fn terrain_field_builder(
        terrain: TerrainCollider,
        scale: Vec3,
        field: &crate::TerrainField,
    ) -> Result<ColliderBuilder, PhysicsError> {
        if field.settings() != &terrain.settings {
            return Err(PhysicsError::InvalidSettings);
        }
        let n = terrain.settings.resolution as usize;
        // Rapier indexes (z,x) in column-major order; engine heights are row-major (z,x).
        let heights = (0..n)
            .flat_map(|x| (0..n).map(move |z| (x, z)))
            .map(|(x, z)| field.values()[z * n + x])
            .collect();
        let heights = rapier3d::parry::utils::Array2::new(n, n, heights);
        Ok(ColliderBuilder::heightfield_with_flags(
            heights,
            Vector::new(
                terrain.settings.size as f32 * scale.x,
                scale.y,
                terrain.settings.size as f32 * scale.z,
            ),
            rapier3d::parry::shape::HeightFieldFlags::FIX_INTERNAL_EDGES,
        ))
    }
    impl PhysicsState {
        pub(crate) fn step(&mut self, world: &mut World, dt: Duration) -> Result<(), PhysicsError> {
            if self.inputs.is_empty() && self.entries.is_empty() {
                self.pending_forces.clear();
                self.pending_torques.clear();
                self.pending_drives.clear();
                return Ok(());
            }
            let dt = dt.as_secs_f32();
            if !dt.is_finite() || dt <= 0.0 || dt > 0.1 {
                return Err(PhysicsError::InvalidTimestep);
            }
            // Validate the whole candidate before changing backend state or stepping.
            for &(id, settings) in &self.inputs {
                let global = validate_pose(settings, world.global_transform(id)?.matrix())?;
                // Preserve authored scale exactly; matrix decomposition can introduce rounding.
                let t = if settings.body == BodyKind::Dynamic {
                    *world.local_transform(id)?
                } else {
                    global
                };
                if settings.body == BodyKind::Dynamic {
                    if t.scale.min_element() <= 0.0 {
                        return Err(PhysicsError::UnsupportedTransform);
                    }
                    if let Some(parent) = world.parent(id)? {
                        if world.parent(parent)?.is_some()
                            || world.component::<PhysicsBody>(parent)?.is_some()
                            || world.global_transform(parent)?.matrix() != crate::Mat4::IDENTITY
                        {
                            return Err(PhysicsError::UnsupportedParent);
                        }
                    }
                    if let Some(entry) = self.entries.get(&id) {
                        if entry.settings == settings
                            && entry.scale == t.scale
                            && *world.local_transform(id)? != entry.last_local
                        {
                            return Err(PhysicsError::TransformConflict);
                        }
                    }
                }
            }
            self.entries.retain(|id, entry| {
                let keep = world
                    .component::<PhysicsBody>(*id)
                    .is_ok_and(|c| c.is_some());
                if !keep {
                    self.backend.remove_body_with_colliders(entry.handle, true);
                }
                keep
            });
            for &(id, settings) in &self.inputs {
                let global = validate_pose(settings, world.global_transform(id)?.matrix())?;
                // Preserve authored scale exactly; matrix decomposition can introduce rounding.
                let t = if settings.body == BodyKind::Dynamic {
                    *world.local_transform(id)?
                } else {
                    global
                };
                let rebuild = self
                    .entries
                    .get(&id)
                    .is_none_or(|e| e.settings != settings || e.scale != t.scale);
                if rebuild {
                    let revision = self
                        .last_revision
                        .checked_add(1)
                        .ok_or(PhysicsError::BodyRevisionExhausted)?;
                    if let Some(old) = self.entries.remove(&id) {
                        self.backend.remove_body_with_colliders(old.handle, true);
                    }
                    let builder = match settings.body {
                        BodyKind::Static => RigidBodyBuilder::fixed(),
                        BodyKind::Dynamic => RigidBodyBuilder::dynamic(),
                    };
                    let collider = match settings.collider {
                        Collider::Box(bounds) => {
                            let half = Vec3::from_array(bounds.half_extents) * t.scale;
                            let offset = Vec3::from_array(bounds.center) * t.scale;
                            ColliderBuilder::cuboid(half.x, half.y, half.z)
                                .translation(Vector::from_array(offset.to_array()))
                        }
                        Collider::Terrain(terrain) => {
                            match world.component::<crate::terrain::AuthoredTerrain>(id)? {
                                Some(field) => terrain_field_builder(terrain, t.scale, &field.0)?,
                                None => terrain_builder(terrain, t.scale)?,
                            }
                        }
                    }
                    .mass(settings.mass)
                    .friction(settings.friction)
                    // Friction 0 means frictionless against anything (Rapier would otherwise average it).
                    .friction_combine_rule(if settings.friction == 0.0 {
                        rapier3d::prelude::CoefficientCombineRule::Min
                    } else {
                        rapier3d::prelude::CoefficientCombineRule::Average
                    })
                    .restitution(settings.restitution);
                    let (handle, _) = self.backend.insert(
                        builder
                            .pose(pose(t))
                            .gravity_scale(settings.gravity_scale)
                            .ccd_enabled(true),
                        collider,
                    );
                    self.entries.insert(
                        id,
                        Entry {
                            handle,
                            settings,
                            scale: t.scale,
                            last_local: *world.local_transform(id)?,
                            revision,
                        },
                    );
                    self.last_revision = revision;
                } else if settings.body == BodyKind::Static {
                    let entry = &self.entries[&id];
                    let body = &mut self.backend.bodies[entry.handle];
                    let next = pose(t);
                    if *body.position() != next {
                        body.set_position(next, true);
                    }
                }
            }
            for entry in self.entries.values() {
                if entry.settings.body == BodyKind::Dynamic {
                    self.backend.bodies[entry.handle].reset_forces(false);
                    self.backend.bodies[entry.handle].reset_torques(false);
                }
            }
            for (&id, &force) in &self.pending_forces {
                if let Some(entry) = self.entries.get(&id) {
                    if entry.settings.body == BodyKind::Dynamic {
                        self.backend.bodies[entry.handle]
                            .add_force(Vector::from_array(force.to_array()), true);
                    }
                }
            }
            self.pending_forces.clear();
            for (&id, &torque) in &self.pending_torques {
                if let Some(entry) = self.entries.get(&id) {
                    if entry.settings.body == BodyKind::Dynamic {
                        self.backend.bodies[entry.handle]
                            .add_torque(Vector::from_array(torque.to_array()), true);
                    }
                }
            }
            self.pending_torques.clear();
            for (&id, &(velocity, jump, force)) in &self.pending_drives {
                if let Some(entry) = self.entries.get(&id) {
                    if entry.settings.body == BodyKind::Dynamic {
                        let grounded = self.grounded(id);
                        let body = &mut self.backend.bodies[entry.handle];
                        body.lock_rotations(true, true);
                        let current = body.linvel();
                        let vertical = match jump {
                            Some(speed) if grounded && current.y <= 0.1 => speed,
                            _ => current.y,
                        };
                        let target = Vector::new(velocity.x, 0.0, velocity.z);
                        let horizontal = if force <= 0.0 {
                            target
                        } else {
                            let now = Vector::new(current.x, 0.0, current.z);
                            if target == Vector::ZERO {
                                // No input: contact friction (μ·m·g, so a = μ·g) coasts the body to rest.
                                now
                            } else {
                                // a = F/m toward the target; friction still acts in the step, so net a = F/m − μ·g.
                                let gap = target - now;
                                let step = force / body.mass().max(f32::EPSILON) * dt;
                                if gap.length() <= step {
                                    target
                                } else {
                                    now + gap.normalize() * step
                                }
                            }
                        };
                        body.set_linvel(Vector::new(horizontal.x, vertical, horizontal.z), true);
                    }
                }
            }
            self.pending_drives.clear();
            self.backend.gravity = Vector::from_array(PHYSICS_GRAVITY.to_array());
            self.backend.integration_parameters.dt = dt;
            self.backend.step();
            for &(id, settings) in &self.inputs {
                if settings.body != BodyKind::Dynamic {
                    continue;
                }
                let entry = self.entries.get_mut(&id).expect("validated body inserted");
                let p = self.backend.bodies[entry.handle].position();
                let t = Transform::new(
                    Vec3::from_array(p.translation.to_array()),
                    crate::Quat::from_xyzw(p.rotation.x, p.rotation.y, p.rotation.z, p.rotation.w),
                    entry.scale,
                );
                validate_pose(
                    settings,
                    crate::Mat4::from_scale_rotation_translation(
                        t.scale,
                        t.rotation,
                        t.translation,
                    ),
                )?;
                if *world.local_transform(id)? != t {
                    world.set_local_transform(id, t)?;
                }
                entry.last_local = t;
            }
            Ok(())
        }
    }
}

#[cfg(all(test, feature = "physics"))]
mod terrain_tests {
    use super::*;
    #[test]
    fn maximum_terrain_grid_builds_a_complete_rapier_heightfield() {
        let settings = crate::TerrainSettings {
            resolution: 1024,
            size: 270.,
            ..Default::default()
        };
        let field = crate::TerrainField::from_heights(settings, vec![1.; 1024 * 1024]).unwrap();
        let collider =
            runtime::terrain_field_builder(TerrainCollider { settings }, Vec3::ONE, &field)
                .unwrap()
                .build();
        let heightfield = collider.shape().as_heightfield().unwrap();
        for (z, x) in [(0, 0), (1022, 1022)] {
            let (a, b) = heightfield.triangles_at(z, x);
            for triangle in [a.unwrap(), b.unwrap()] {
                for point in [triangle.a, triangle.b, triangle.c] {
                    assert_eq!(point.y, 1.);
                    assert!(point.x.abs() <= 135. && point.z.abs() <= 135.);
                }
            }
        }
        assert_eq!(heightfield.triangles_at(1023, 1023), (None, None));
    }
    #[test]
    fn heightfield_triangles_match_render_grid_orientation_and_diagonals() {
        let settings = crate::TerrainSettings {
            resolution: 32,
            size: 20.,
            height: 7.,
            ..Default::default()
        };
        // A frozen sculpted ramp must drive both rendering and collision.
        let field = crate::TerrainField::from_heights(
            settings,
            (0..1024).map(|i| (i % 32) as f32 / 5.).collect(),
        )
        .unwrap();
        let mesh = field.mesh(crate::Color::WHITE).unwrap();
        let scale = Vec3::new(2., 1.5, 0.75);
        let collider = runtime::terrain_field_builder(TerrainCollider { settings }, scale, &field)
            .unwrap()
            .build();
        let heightfield = collider.shape().as_heightfield().unwrap();
        for z in 0..31 {
            for x in 0..31 {
                let (a, b) = heightfield.triangles_at(z, x);
                let triangle = a.unwrap();
                let other = b.unwrap();
                let i = z * 32 + x;
                for (point, index) in [
                    (triangle.a, i),
                    (triangle.b, i + 32),
                    (triangle.c, i + 1),
                    (other.a, i + 32),
                    (other.b, i + 33),
                    (other.c, i + 1),
                ] {
                    assert!(
                        (Vec3::from_array(point.to_array())
                            - mesh.vertices()[index].position() * scale)
                            .length()
                            < 0.0001
                    );
                }
            }
        }
    }
}
