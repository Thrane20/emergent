//! Headless math for inspection tools; handles still belong to the caller's World.
use crate::{EntityId, Transform, World, WorldError};
use glam::{Mat4, Quat, Vec2, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformSpace {
    Local,
    Global,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GizmoTool {
    Translate,
    Rotate,
    Scale,
}

/// Normalized viewport coordinates: top-left (0,0), bottom-right (1,1).
pub fn project_point(view_projection: Mat4, point: Vec3) -> Option<Vec3> {
    let clip = view_projection * point.extend(1.0);
    if !clip.is_finite() || clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !(0.0..=1.0).contains(&ndc.z) {
        return None;
    }
    Some(Vec3::new((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5, ndc.z))
}

#[derive(Clone, Copy, Debug)]
pub struct InspectionRay {
    pub origin: Vec3,
    pub direction: Vec3,
    pub length: f32,
}
impl InspectionRay {
    pub fn from_viewport(view_projection: Mat4, cursor: Vec2) -> Option<Self> {
        if !cursor.is_finite() || cursor.min_element() < 0.0 || cursor.max_element() > 1.0 {
            return None;
        }
        let inverse = view_projection.inverse();
        let p = cursor * Vec2::new(2.0, -2.0) + Vec2::new(-1.0, 1.0);
        let near = inverse.project_point3(p.extend(0.0));
        let far = inverse.project_point3(p.extend(1.0));
        let delta = far - near;
        if !near.is_finite() || !far.is_finite() || delta.length() < 1e-8 {
            return None;
        }
        Some(Self {
            origin: near,
            direction: delta.normalize(),
            length: delta.length(),
        })
    }
}

/// Optional CPU cache made from the same triangles as rendering; not a second identity store.
pub struct PickingMesh {
    positions: Vec<Vec3>,
    indices: Vec<u32>,
    min: Vec3,
    max: Vec3,
}
impl PickingMesh {
    /// Cached local-space bounds; no geometry rebuild or vertex scan.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        (self.min, self.max)
    }
    #[cfg(any(feature = "graphics", test))]
    pub(crate) fn new(positions: Vec<Vec3>, indices: Vec<u32>) -> Self {
        let min = positions
            .iter()
            .copied()
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let max = positions
            .iter()
            .copied()
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        Self {
            positions,
            indices,
            min,
            max,
        }
    }
    /// Distance along the world ray. Tests both triangle sides, nearest first by distance.
    pub fn intersect(&self, ray: InspectionRay, model: Mat4) -> Option<f32> {
        let inv = model.inverse();
        if !inv.is_finite() {
            return None;
        }
        let origin = inv.transform_point3(ray.origin);
        let direction = inv.transform_vector3(ray.direction);
        let mut near = 0.0_f32;
        let mut far = ray.length;
        for axis in 0..3 {
            if direction[axis].abs() < 1e-20 {
                if origin[axis] < self.min[axis] || origin[axis] > self.max[axis] {
                    return None;
                }
            } else {
                let a = (self.min[axis] - origin[axis]) / direction[axis];
                let b = (self.max[axis] - origin[axis]) / direction[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far {
                    return None;
                }
            }
        }
        let mut best = ray.length;
        let mut hit = false;
        for tri in self.indices.as_chunks::<3>().0 {
            let a = self.positions[tri[0] as usize];
            let e1 = self.positions[tri[1] as usize] - a;
            let e2 = self.positions[tri[2] as usize] - a;
            let p = direction.cross(e2);
            let determinant = e1.dot(p);
            if determinant.abs() <= f32::EPSILON * e1.length() * p.length() {
                continue;
            }
            let t = origin - a;
            let u = t.dot(p) / determinant;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = t.cross(e1);
            let v = direction.dot(q) / determinant;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let distance = e2.dot(q) / determinant;
            if distance >= 0.0 && distance <= best {
                best = distance;
                hit = true;
            }
        }
        hit.then_some(best)
    }
}

/// Axis frame for tools, deliberately distinct from a decomposed global TRS.
pub fn gizmo_frame(
    world: &World,
    id: EntityId,
    space: TransformSpace,
    tool: GizmoTool,
) -> Result<Mat4, WorldError> {
    let global = world.global_transform(id)?.matrix();
    let parent = world
        .parent(id)?
        .map(|p| world.global_transform(p).map(|t| t.matrix()))
        .transpose()?
        .unwrap_or(Mat4::IDENTITY);
    let linear = match (space, tool) {
        (TransformSpace::Global, _) => Mat4::IDENTITY,
        (TransformSpace::Local, GizmoTool::Translate) => parent,
        (TransformSpace::Local, GizmoTool::Rotate) => {
            parent * Mat4::from_quat(world.local_transform(id)?.rotation)
        }
        _ => global,
    };
    Ok(Mat4::from_cols(
        linear.x_axis.with_w(0.0),
        linear.y_axis.with_w(0.0),
        linear.z_axis.with_w(0.0),
        global.w_axis,
    ))
}

/// Immutable start snapshot: every pointer update is computed from the original transform.
pub struct TransformGizmo {
    original: Transform,
    parent: Mat4,
    global: Option<Transform>,
    frame: Mat4,
    view_projection: Mat4,
    start: Vec2,
    axis: usize,
    space: TransformSpace,
    tool: GizmoTool,
    axis_screen: Vec2,
    world_length: f32,
}
impl TransformGizmo {
    pub fn begin(
        world: &World,
        id: EntityId,
        space: TransformSpace,
        tool: GizmoTool,
        axis: usize,
        view_projection: Mat4,
        start: Vec2,
    ) -> Result<Self, WorldError> {
        if axis > 2 || InspectionRay::from_viewport(view_projection, start).is_none() {
            return Err(WorldError::InvalidTransform);
        }
        let frame = gizmo_frame(world, id, space, tool)?;
        let origin = frame.w_axis.truncate();
        let distance = view_projection
            .inverse()
            .project_point3(Vec3::ZERO)
            .distance(origin);
        let world_length = (distance * 0.12).max(0.01);
        let axis_world = frame.col(axis).truncate().normalize();
        let screen =
            project_point(view_projection, origin).ok_or(WorldError::UnrepresentableTransform)?;
        let tip = project_point(view_projection, origin + axis_world * world_length)
            .ok_or(WorldError::UnrepresentableTransform)?;
        let axis_screen = (tip - screen).truncate();
        if tool != GizmoTool::Rotate && (axis_screen * Vec2::new(960.0, 540.0)).length() < 8.0 {
            return Err(WorldError::UnrepresentableTransform);
        }
        let parent = world
            .parent(id)?
            .map(|p| world.global_transform(p).map(|t| t.matrix()))
            .transpose()?
            .unwrap_or(Mat4::IDENTITY);
        let global = if space == TransformSpace::Global && tool != GizmoTool::Translate {
            Some(Transform::try_from_matrix(
                world.global_transform(id)?.matrix(),
            )?)
        } else {
            None
        };
        let result = Self {
            original: *world.local_transform(id)?,
            parent,
            global,
            frame,
            view_projection,
            start,
            axis,
            space,
            tool,
            axis_screen,
            world_length,
        };
        if tool == GizmoTool::Rotate {
            result.plane_vector(start)?;
        }
        Ok(result)
    }
    pub fn original(&self) -> Transform {
        self.original
    }
    fn plane_vector(&self, cursor: Vec2) -> Result<Vec3, WorldError> {
        let ray = InspectionRay::from_viewport(self.view_projection, cursor)
            .ok_or(WorldError::InvalidTransform)?;
        let inverse = self.frame.inverse();
        let origin = inverse.transform_point3(ray.origin);
        let direction = inverse.transform_vector3(ray.direction);
        if !direction.is_finite() || direction[self.axis].abs() < 1e-6 * direction.length() {
            return Err(WorldError::UnrepresentableTransform);
        }
        let distance = -origin[self.axis] / direction[self.axis];
        let point = origin + direction * distance;
        if distance < 0.0 || !point.is_finite() || point.length() < 1e-6 {
            return Err(WorldError::UnrepresentableTransform);
        }
        Ok(point.normalize())
    }
    pub fn update(&self, cursor: Vec2) -> Result<Transform, WorldError> {
        if InspectionRay::from_viewport(self.view_projection, cursor).is_none() {
            return Err(WorldError::InvalidTransform);
        }
        if cursor == self.start {
            return Ok(self.original);
        }
        let mut local = self.original;
        let pixels = Vec2::new(960.0, 540.0);
        let delta = (cursor - self.start) * pixels;
        let screen = self.axis_screen * pixels;
        let along = delta.dot(screen) / screen.length_squared();
        let axis = Vec3::AXES[self.axis];
        match self.tool {
            GizmoTool::Translate => {
                let amount = along * self.world_length;
                if self.space == TransformSpace::Local {
                    local.translation +=
                        axis * amount / self.frame.col(self.axis).truncate().length();
                } else {
                    local.translation = self
                        .parent
                        .inverse()
                        .transform_point3(self.frame.w_axis.truncate() + axis * amount);
                }
            }
            GizmoTool::Rotate => {
                let a = self.plane_vector(self.start)?;
                let b = self.plane_vector(cursor)?;
                let angle = axis.dot(a.cross(b)).atan2(a.dot(b));
                let rotation = Quat::from_axis_angle(axis, angle);
                if self.space == TransformSpace::Local {
                    local.rotation = (local.rotation * rotation).normalize();
                } else {
                    let mut desired = self.global.ok_or(WorldError::UnrepresentableTransform)?;
                    desired.rotation = (rotation * desired.rotation).normalize();
                    local = Transform::try_from_matrix(
                        self.parent.inverse()
                            * Mat4::from_scale_rotation_translation(
                                desired.scale,
                                desired.rotation,
                                desired.translation,
                            ),
                    )?;
                }
            }
            GizmoTool::Scale => {
                let factor = along.clamp(-4.0, 4.0).exp();
                if self.space == TransformSpace::Local {
                    local.scale[self.axis] *= factor;
                } else {
                    let before = self.parent
                        * Mat4::from_scale_rotation_translation(
                            self.original.scale,
                            self.original.rotation,
                            self.original.translation,
                        );
                    let mut scale = Vec3::ONE;
                    scale[self.axis] = factor;
                    let mut desired = Mat4::from_scale(scale) * before;
                    desired.w_axis = before.w_axis;
                    local = Transform::try_from_matrix(self.parent.inverse() * desired)?;
                }
            }
        }
        if !local.translation.is_finite()
            || !local.scale.is_finite()
            || local.scale.abs().min_element() < 0.0001
        {
            return Err(WorldError::InvalidTransform);
        }
        Ok(local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> Mat4 {
        glam::camera::rh::proj::directx::perspective(45f32.to_radians(), 960.0 / 540.0, 0.1, 100.0)
            * glam::camera::rh::view::look_at_mat4(Vec3::new(7.0, 5.0, 9.0), Vec3::ZERO, Vec3::Y)
    }
    #[test]
    fn projection_ray_and_nearest_triangles_respect_depth_and_nonuniform_models() {
        let vp = view();
        let p = project_point(vp, Vec3::ZERO).unwrap();
        assert!((p.truncate() - Vec2::splat(0.5)).length() < 1e-5);
        assert!(project_point(vp, Vec3::new(70.0, 50.0, 90.0)).is_none());
        let ray = InspectionRay::from_viewport(vp, Vec2::splat(0.5)).unwrap();
        let mesh = PickingMesh::new(
            vec![
                Vec3::new(-5.0, -5.0, 0.0),
                Vec3::new(5.0, -5.0, 0.0),
                Vec3::new(0.0, 5.0, 0.0),
            ],
            vec![0, 1, 2],
        );
        let near = mesh
            .intersect(ray, Mat4::from_scale(Vec3::new(2.0, 1.0, 0.5)))
            .unwrap();
        let far = mesh
            .intersect(ray, Mat4::from_translation(ray.direction * 2.0))
            .unwrap();
        assert!(far > near);
        assert!(mesh
            .intersect(ray, Mat4::from_translation(Vec3::splat(50.0)))
            .is_none());
        assert!(InspectionRay::from_viewport(vp, Vec2::new(-0.1, 0.5)).is_none());
    }
    #[test]
    fn gizmo_updates_are_absolute_cancelable_and_parent_aware() {
        let mut world = World::new();
        let parent = world.spawn("parent");
        let child = world.spawn_child(parent, "child").unwrap();
        world
            .set_local_transform(
                parent,
                Transform::new(
                    Vec3::ZERO,
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                    Vec3::splat(2.0),
                ),
            )
            .unwrap();
        world.propagate_transforms();
        for space in [TransformSpace::Local, TransformSpace::Global] {
            for tool in [GizmoTool::Translate, GizmoTool::Scale] {
                let drag =
                    TransformGizmo::begin(&world, child, space, tool, 0, view(), Vec2::splat(0.5))
                        .unwrap();
                let result = drag.update(Vec2::new(0.6, 0.4)).unwrap();
                assert_ne!(result, drag.original());
                assert_eq!(drag.update(Vec2::new(0.6, 0.4)).unwrap(), result);
                assert_eq!(drag.update(Vec2::splat(0.5)).unwrap(), drag.original());
            }
        }
        world
            .set_local_transform(parent, Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)))
            .unwrap();
        world
            .set_local_transform(child, Transform::from_rotation(Quat::from_rotation_z(0.5)))
            .unwrap();
        world.propagate_transforms();
        assert!(TransformGizmo::begin(
            &world,
            child,
            TransformSpace::Global,
            GizmoTool::Scale,
            0,
            view(),
            Vec2::splat(0.5)
        )
        .is_err());
        let rotation = TransformGizmo::begin(
            &world,
            child,
            TransformSpace::Local,
            GizmoTool::Rotate,
            2,
            view(),
            Vec2::new(0.6, 0.4),
        )
        .unwrap();
        assert_ne!(
            rotation.update(Vec2::new(0.5, 0.4)).unwrap().rotation,
            rotation.original().rotation
        );
    }
    #[test]
    fn global_rotation_reconstructs_world_orientation_under_a_rotated_parent() {
        let mut world = World::new();
        let parent = world.spawn("parent");
        let child = world.spawn_child(parent, "child").unwrap();
        world
            .set_local_transform(
                parent,
                Transform::new(Vec3::ZERO, Quat::from_rotation_x(0.3), Vec3::splat(2.0)),
            )
            .unwrap();
        world
            .set_local_transform(child, Transform::from_rotation(Quat::from_rotation_y(0.2)))
            .unwrap();
        world.propagate_transforms();
        let before = world.global_transform(child).unwrap().matrix();
        let vp = view();
        let start = project_point(vp, Vec3::X).unwrap().truncate();
        let end = project_point(vp, Vec3::Y).unwrap().truncate();
        let drag = TransformGizmo::begin(
            &world,
            child,
            TransformSpace::Global,
            GizmoTool::Rotate,
            2,
            vp,
            start,
        )
        .unwrap();
        let next = drag.update(end).unwrap();
        world.set_local_transform(child, next).unwrap();
        world.propagate_transforms();
        let expected = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2) * before;
        assert!(world
            .global_transform(child)
            .unwrap()
            .matrix()
            .abs_diff_eq(expected, 1e-4));
        world.remove_subtree(parent).unwrap();
        assert!(TransformGizmo::begin(
            &world,
            child,
            TransformSpace::Local,
            GizmoTool::Translate,
            0,
            vp,
            start
        )
        .is_err());
    }
    #[test]
    fn global_scale_uses_world_axes_and_rejects_unrepresentable_shear() {
        let mut world = World::new();
        let id = world.spawn("object");
        world
            .set_local_transform(
                id,
                Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            )
            .unwrap();
        world.propagate_transforms();
        let drag = TransformGizmo::begin(
            &world,
            id,
            TransformSpace::Global,
            GizmoTool::Scale,
            0,
            view(),
            Vec2::splat(0.5),
        )
        .unwrap();
        let scaled = drag.update(Vec2::new(0.6, 0.5)).unwrap();
        assert!((scaled.scale.x - 1.0).abs() < 1e-4);
        assert!(
            scaled.scale.y > 1.0,
            "world X must map to local Y after a 90-degree rotation"
        );
        assert_eq!(scaled.translation, drag.original().translation);
        world
            .set_local_transform(
                id,
                Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
            )
            .unwrap();
        world.propagate_transforms();
        let drag = TransformGizmo::begin(
            &world,
            id,
            TransformSpace::Global,
            GizmoTool::Scale,
            0,
            view(),
            Vec2::splat(0.5),
        )
        .unwrap();
        assert_eq!(
            drag.update(Vec2::new(0.6, 0.5)),
            Err(WorldError::UnrepresentableTransform)
        );
        assert_eq!(*world.local_transform(id).unwrap(), drag.original());
    }
}
