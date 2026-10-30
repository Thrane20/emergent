use crate::game::FirstProjectGame;
use emergent::{
    gizmo_frame, project_point, Engine, EntityId, GeometryRenderer, GizmoTool, InspectionRay, Mat4,
    PickingMesh, Transform, TransformGizmo, TransformSpace, Vec2, Vec3,
};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum ToolCommand {
    Focus {
        seq: u32,
        id: String,
    },
    Align {
        seq: u32,
        id: String,
    },
    Pick {
        seq: u32,
        x: f32,
        y: f32,
    },
    Select {
        seq: u32,
        id: Option<String>,
        space: String,
        tool: String,
    },
    Begin {
        seq: u32,
        token: String,
        id: String,
        space: String,
        tool: String,
        axis: usize,
        x: f32,
        y: f32,
    },
    Drag {
        seq: u32,
        token: String,
        x: f32,
        y: f32,
    },
    Finish {
        seq: u32,
        token: String,
    },
    Cancel {
        seq: u32,
        token: String,
    },
}
impl ToolCommand {
    pub fn seq(&self) -> u32 {
        match self {
            Self::Focus { seq, .. }
            | Self::Align { seq, .. }
            | Self::Pick { seq, .. }
            | Self::Select { seq, .. }
            | Self::Begin { seq, .. }
            | Self::Drag { seq, .. }
            | Self::Finish { seq, .. }
            | Self::Cancel { seq, .. } => *seq,
        }
    }
}
#[derive(Serialize)]
pub struct Marker {
    id: String,
    point: [f32; 2],
}
#[derive(Serialize)]
pub struct Handle {
    axis: usize,
    points: Vec<[f32; 2]>,
}
#[derive(Serialize)]
pub struct Frustum {
    id: String,
    segments: Vec<[[f32; 2]; 2]>,
}
#[derive(Serialize)]
pub struct ToolResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    bounds: Option<Bounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    view: Option<View>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<Values>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frustum: Option<Frustum>,
    selected: String,
    tool: String,
    space: String,
    handles: Vec<Handle>,
    markers: Vec<Marker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transform: Option<Values>,
}
#[derive(Serialize)]
struct Bounds {
    center: [f32; 3],
    radius: f32,
}
#[derive(Serialize)]
struct View {
    eye: [f32; 3],
    forward: [f32; 3],
    projection: &'static str,
    field_of_view_degrees: f32,
    view_size: f32,
}
#[derive(Serialize)]
pub struct Values {
    translation: [f32; 3],
    rotation_degrees: [f32; 3],
    scale: [f32; 3],
}
impl From<Transform> for Values {
    fn from(t: Transform) -> Self {
        let (x, y, z) = t.rotation.to_euler(emergent::EulerRot::XYZ);
        Self {
            translation: t.translation.to_array(),
            rotation_degrees: [x, y, z].map(f32::to_degrees),
            scale: t.scale.to_array(),
        }
    }
}
pub struct Tools {
    selected: Option<String>,
    space: TransformSpace,
    tool: GizmoTool,
    drag: Option<(EntityId, TransformGizmo, String)>,
    meshes: Vec<(EntityId, PickingMesh)>,
}
impl Tools {
    pub fn new(game: &FirstProjectGame) -> Self {
        Self {
            selected: None,
            space: TransformSpace::Local,
            tool: GizmoTool::Translate,
            drag: None,
            meshes: game
                .loaded
                .renderables
                .iter()
                .map(|(id, g)| (*id, PickingMesh::from_geometry(g)))
                .collect(),
        }
    }
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub fn apply(
        &mut self,
        command: &ToolCommand,
        engine: &mut Engine,
        game: &FirstProjectGame,
        renderer: &GeometryRenderer,
    ) -> Result<Option<Transform>, Box<dyn Error>> {
        if self.drag.is_some()
            && !matches!(
                command,
                ToolCommand::Drag { .. } | ToolCommand::Finish { .. } | ToolCommand::Cancel { .. }
            )
        {
            return Err("Finish or cancel the active gizmo drag first".into());
        }
        if let Some((_, _, active)) = &self.drag {
            if let ToolCommand::Drag { token, .. }
            | ToolCommand::Finish { token, .. }
            | ToolCommand::Cancel { token, .. } = command
            {
                if token != active {
                    return Err("Stale gizmo gesture".into());
                }
            }
        }
        match command {
            ToolCommand::Focus { id, .. } | ToolCommand::Align { id, .. } => {
                let entity = *game
                    .loaded
                    .entities
                    .get(id)
                    .ok_or("Selected instance is not in this scene")?;
                if matches!(command, ToolCommand::Align { .. }) {
                    let proposed =
                        aligned_transform(engine.world(), entity, renderer.view_transform())?;
                    self.selected = Some(id.clone());
                    return Ok(Some(proposed));
                }
                self.selected = Some(id.clone());
            }
            ToolCommand::Select {
                id, space, tool, ..
            } => {
                if let Some(id) = id {
                    if !game.loaded.entities.contains_key(id) {
                        return Err("Selected instance is not in this scene".into());
                    }
                }
                let space = match space.as_str() {
                    "local" => TransformSpace::Local,
                    "global" => TransformSpace::Global,
                    _ => return Err("Invalid gizmo space".into()),
                };
                let tool = match tool.as_str() {
                    "translate" => GizmoTool::Translate,
                    "rotate" => GizmoTool::Rotate,
                    "scale" => GizmoTool::Scale,
                    _ => return Err("Invalid gizmo tool".into()),
                };
                self.selected = id.clone();
                self.space = space;
                self.tool = tool;
            }
            ToolCommand::Pick { x, y, .. } => {
                let cursor = Vec2::new(*x, *y);
                let vp = renderer.view_projection();
                let ray =
                    InspectionRay::from_viewport(vp, cursor).ok_or("Pick outside viewport")?;
                let mut best = ray.length;
                let mut selected = None;
                for (id, mesh) in &self.meshes {
                    if let Some(distance) =
                        mesh.intersect(ray, engine.world().global_transform(*id)?.matrix())
                    {
                        if distance < best {
                            best = distance;
                            selected = Some(*id);
                        }
                    }
                }
                for &id in game.loaded.entities.values() {
                    if let Some(field) = engine
                        .world()
                        .component::<emergent::RaymarchedTerrain>(id)?
                    {
                        let model = engine.world().global_transform(id)?.matrix();
                        let inverse = model.inverse();
                        let local_origin = inverse.transform_point3(ray.origin);
                        let local_direction = inverse.transform_vector3(ray.direction);
                        if let Some(t) = field.trace(local_origin, local_direction)? {
                            let point = model
                                .transform_point3(local_origin + local_direction.normalize() * t);
                            let distance = point.distance(ray.origin);
                            if distance < best {
                                best = distance;
                                selected = Some(id);
                            }
                        }
                    }
                }
                // Light/empty-node origins are editor icons, visible above scene geometry.
                let mut icon_distance = 12.0;
                for marker in self.markers(engine, game, vp)? {
                    let distance = ((Vec2::from_array(marker.point) - cursor)
                        * Vec2::new(960.0, 540.0))
                    .length();
                    if distance < icon_distance {
                        icon_distance = distance;
                        selected = Some(game.loaded.entities[&marker.id]);
                    }
                }
                self.selected = selected.and_then(|id| {
                    game.loaded
                        .entities
                        .iter()
                        .find(|(_, entity)| **entity == id)
                        .map(|(name, _)| name.clone())
                });
            }
            ToolCommand::Begin {
                axis,
                x,
                y,
                token,
                id: expected,
                space,
                tool,
                ..
            } => {
                if token.len() > 64 || token.is_empty() {
                    return Err("Invalid gesture token".into());
                }
                let current_tool = match self.tool {
                    GizmoTool::Translate => "translate",
                    GizmoTool::Rotate => "rotate",
                    GizmoTool::Scale => "scale",
                };
                let current_space = match self.space {
                    TransformSpace::Local => "local",
                    TransformSpace::Global => "global",
                };
                if self.selected.as_ref() != Some(expected)
                    || space != current_space
                    || tool != current_tool
                {
                    return Err("Gizmo selection changed. Select the instance again.".into());
                }
                let id = self.entity(game)?;
                let drag = TransformGizmo::begin(
                    engine.world(),
                    id,
                    self.space,
                    self.tool,
                    *axis,
                    renderer.view_projection(),
                    Vec2::new(*x, *y),
                )
                .map_err(|e| {
                    format!("Cannot drag this axis: {e}. Change view or use Local mode.")
                })?;
                self.drag = Some((id, drag, token.clone()));
            }
            ToolCommand::Drag { x, y, .. } => {
                let (id, drag, _) = self.drag.as_ref().ok_or("No active gizmo drag")?;
                let before = *engine.world().local_transform(*id)?;
                let next = drag.update(Vec2::new(*x, *y)).map_err(|e| e.to_string())?;
                engine.world_mut().set_local_transform(*id, next)?;
                engine.world_mut().propagate_transforms();
                if !game.loaded.entities.values().all(|entity| {
                    engine
                        .world()
                        .global_transform(*entity)
                        .is_ok_and(|t| t.matrix().is_finite() && t.matrix().inverse().is_finite())
                }) {
                    engine.world_mut().set_local_transform(*id, before)?;
                    engine.world_mut().propagate_transforms();
                    return Err("Drag would overflow descendant transforms".into());
                }
            }
            ToolCommand::Finish { .. } => {
                if self.drag.take().is_none() {
                    return Err("No active gizmo drag".into());
                }
            }
            ToolCommand::Cancel { .. } => {
                if let Some((id, drag, _)) = self.drag.take() {
                    engine
                        .world_mut()
                        .set_local_transform(id, drag.original())?;
                    engine.world_mut().propagate_transforms();
                }
            }
        }
        Ok(None)
    }
    fn entity(&self, game: &FirstProjectGame) -> Result<EntityId, Box<dyn Error>> {
        self.selected
            .as_ref()
            .and_then(|id| game.loaded.entities.get(id))
            .copied()
            .ok_or_else(|| "Select a scene instance first".into())
    }
    fn markers(
        &self,
        engine: &Engine,
        game: &FirstProjectGame,
        vp: Mat4,
    ) -> Result<Vec<Marker>, Box<dyn Error>> {
        let mut markers = Vec::new();
        for (name, id) in &game.loaded.entities {
            if !self.meshes.iter().any(|(entity, _)| entity == id) {
                if let Some(point) =
                    project_point(vp, engine.world().global_transform(*id)?.translation())
                {
                    markers.push(Marker {
                        id: name.clone(),
                        point: [point.x, point.y],
                    });
                }
            }
        }
        Ok(markers)
    }
    pub fn result(
        &self,
        engine: &Engine,
        game: &FirstProjectGame,
        renderer: &GeometryRenderer,
    ) -> Result<ToolResult, Box<dyn Error>> {
        let vp = renderer.view_projection();
        let mut result = ToolResult {
            bounds: None,
            view: None,
            placement: None,
            frustum: None,
            selected: self.selected.clone().unwrap_or_default(),
            tool: match self.tool {
                GizmoTool::Translate => "translate",
                GizmoTool::Rotate => "rotate",
                GizmoTool::Scale => "scale",
            }
            .into(),
            space: match self.space {
                TransformSpace::Local => "local",
                TransformSpace::Global => "global",
            }
            .into(),
            handles: Vec::new(),
            markers: self.markers(engine, game, vp)?,
            origin: None,
            transform: None,
        };
        let Ok(id) = self.entity(game) else {
            return Ok(result);
        };
        let matrix = engine.world().global_transform(id)?.matrix();
        let center = matrix.transform_point3(Vec3::ZERO);
        let mut bounds = Bounds {
            center: center.to_array(),
            radius: 0.5,
        };
        if let Some(field) = engine
            .world()
            .component::<emergent::RaymarchedTerrain>(id)?
        {
            let (lower, upper) = field.bounds();
            let extent = lower.abs().max(upper.abs());
            bounds.radius = matrix.x_axis.truncate().length() * extent.x
                + matrix.y_axis.truncate().length() * extent.y
                + matrix.z_axis.truncate().length() * extent.z;
        }
        if let Some((_, mesh)) = self.meshes.iter().find(|(entity, _)| *entity == id) {
            let (min, max) = mesh.bounds();
            let center = matrix.transform_point3((min + max) * 0.5);
            let mut radius = 0.0_f32;
            for i in 0..8 {
                let point = Vec3::new(
                    if i & 1 == 0 { min.x } else { max.x },
                    if i & 2 == 0 { min.y } else { max.y },
                    if i & 4 == 0 { min.z } else { max.z },
                );
                radius = radius.max(matrix.transform_point3(point).distance(center));
            }
            bounds = Bounds {
                center: center.to_array(),
                radius: radius.max(0.01),
            };
        }
        if !Vec3::from_array(bounds.center).is_finite() || !bounds.radius.is_finite() {
            return Err("Selected object's bounds exceed finite inspection coordinates".into());
        }
        result.bounds = Some(bounds);
        let view = renderer.view_transform();
        let (projection, field_of_view_degrees, view_size) = match renderer.projection() {
            emergent::CameraProjection::Perspective {
                vertical_fov_radians,
                ..
            } => ("perspective", vertical_fov_radians.to_degrees(), 12.),
            emergent::CameraProjection::Isometric { vertical_size, .. } => {
                ("isometric", 45., vertical_size)
            }
        };
        result.view = Some(View {
            eye: view.w_axis.truncate().to_array(),
            forward: (-view.z_axis.truncate()).normalize().to_array(),
            projection,
            field_of_view_degrees,
            view_size,
        });
        if let Some(camera) = engine.world().component::<emergent::Camera>(id)? {
            let matrix = engine.world().global_transform(id)?.matrix();
            let origin = matrix.transform_point3(Vec3::ZERO);
            let forward = matrix.transform_vector3(-Vec3::Z).normalize();
            let right = forward.cross(matrix.transform_vector3(Vec3::Y)).normalize();
            let up = right.cross(forward);
            let corners = camera
                .projection()
                .frustum_corners(960. / 540.)?
                .map(|p| origin + right * p.x + up * p.y - forward * p.z);
            let mut segments = Vec::with_capacity(12);
            for (a, b) in [
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 0),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ] {
                if let Some(segment) = clip_segment(vp, corners[a], corners[b]) {
                    segments.push(segment);
                }
            }
            result.frustum = Some(Frustum {
                id: self.selected.clone().unwrap_or_default(),
                segments,
            });
        }
        result.transform = Some((*engine.world().local_transform(id)?).into());
        let frame = gizmo_frame(engine.world(), id, self.space, self.tool)?;
        let origin = frame.w_axis.truncate();
        let Some(screen) = project_point(vp, origin) else {
            return Ok(result);
        };
        result.origin = Some([screen.x, screen.y]);
        if self.space == TransformSpace::Global
            && self.tool != GizmoTool::Translate
            && Transform::try_from_matrix(engine.world().global_transform(id)?.matrix()).is_err()
        {
            return Ok(result);
        }
        let length = (vp.inverse().project_point3(Vec3::ZERO).distance(origin) * 0.12).max(0.01);
        for axis in 0..3 {
            let mut points = Vec::new();
            if self.tool == GizmoTool::Rotate {
                let radius = length
                    / (0..3)
                        .map(|i| frame.col(i).truncate().length())
                        .fold(0.0_f32, f32::max);
                let a = frame.col((axis + 1) % 3).truncate() * radius;
                let b = frame.col((axis + 2) % 3).truncate() * radius;
                for n in 0..=64 {
                    let angle = n as f32 / 64.0 * std::f32::consts::TAU;
                    if let Some(p) = project_point(vp, origin + a * angle.cos() + b * angle.sin()) {
                        points.push([p.x, p.y]);
                    }
                }
                let normal = frame
                    .col((axis + 1) % 3)
                    .truncate()
                    .cross(frame.col((axis + 2) % 3).truncate())
                    .normalize();
                let eye_ray = InspectionRay::from_viewport(vp, screen.truncate())
                    .ok_or("Invalid gizmo projection")?;
                if normal.dot(eye_ray.direction).abs() < 0.08 {
                    continue;
                }
            } else {
                if let Some(p) =
                    project_point(vp, origin + frame.col(axis).truncate().normalize() * length)
                {
                    if ((p - screen).truncate() * Vec2::new(960.0, 540.0)).length() >= 8.0 {
                        points.extend([[screen.x, screen.y], [p.x, p.y]]);
                    }
                }
            }
            if points.len() >= 2 {
                result.handles.push(Handle { axis, points });
            }
        }
        Ok(result)
    }
}

fn aligned_transform(
    world: &emergent::World,
    id: EntityId,
    view: Mat4,
) -> Result<Transform, Box<dyn Error>> {
    let current = Transform::try_from_matrix(world.global_transform(id)?.matrix())?;
    let camera = Transform::try_from_matrix(view)?;
    Ok(world.local_from_global(
        id,
        Transform::new(camera.translation, camera.rotation, current.scale),
    )?)
}

/// Clip in homogeneous space so edges crossing the editor's near plane remain visible.
fn clip_segment(vp: Mat4, a: Vec3, b: Vec3) -> Option<[[f32; 2]; 2]> {
    let a = vp * a.extend(1.);
    let b = vp * b.extend(1.);
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    let mut low = 0.0_f32;
    let mut high = 1.0_f32;
    for (start, end) in [
        (a.x + a.w, b.x + b.w),
        (a.w - a.x, b.w - b.x),
        (a.y + a.w, b.y + b.w),
        (a.w - a.y, b.w - b.y),
        (a.z, b.z),
        (a.w - a.z, b.w - b.z),
    ] {
        if !start.is_finite() || !end.is_finite() {
            return None;
        }
        if start < 0. && end < 0. {
            return None;
        }
        if start < 0. {
            low = low.max(start / (start - end));
        } else if end < 0. {
            high = high.min(start / (start - end));
        }
    }
    if low > high {
        return None;
    }
    let clipped = [a.lerp(b, low), a.lerp(b, high)];
    if clipped.iter().any(|p| p.w <= 0.) {
        return None;
    }
    Some(clipped.map(|p| {
        [
            ((p.x / p.w + 1.) * 0.5).clamp(0., 1.),
            ((1. - p.y / p.w) * 0.5).clamp(0., 1.),
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alignment_preserves_scale_resolves_parent_and_does_not_mutate_the_world() {
        use emergent::{Quat, World};
        let mut world = World::new();
        let parent = world.spawn("parent");
        world
            .set_local_transform(
                parent,
                Transform::new(
                    Vec3::new(4., 1., 2.),
                    Quat::from_rotation_y(0.6),
                    Vec3::splat(2.),
                ),
            )
            .unwrap();
        let child = world.spawn_child(parent, "child").unwrap();
        world
            .set_local_transform(child, Transform::from_scale(Vec3::splat(3.)))
            .unwrap();
        world.propagate_transforms();
        let original = *world.local_transform(child).unwrap();
        let view = Transform::new(
            Vec3::new(7., 5., 9.),
            Quat::from_euler(emergent::EulerRot::XYZ, 0.2, 0.8, 0.),
            Vec3::ONE,
        );
        let matrix =
            Mat4::from_scale_rotation_translation(view.scale, view.rotation, view.translation);
        let aligned = aligned_transform(&world, child, matrix).unwrap();
        assert_eq!(original, *world.local_transform(child).unwrap());
        world.set_local_transform(child, aligned).unwrap();
        world.propagate_transforms();
        let global = world.global_transform(child).unwrap();
        assert!(global.translation().abs_diff_eq(view.translation, 1e-4));
        assert!(global
            .look_direction()
            .unwrap()
            .abs_diff_eq(view.rotation * Vec3::NEG_Z, 1e-4));
        assert!(Transform::try_from_matrix(global.matrix())
            .unwrap()
            .scale
            .abs_diff_eq(Vec3::splat(6.), 1e-4));
        world
            .set_local_transform(parent, Transform::from_scale(Vec3::new(2., 1., 3.)))
            .unwrap();
        world.propagate_transforms();
        let before = *world.local_transform(child).unwrap();
        assert!(aligned_transform(&world, child, matrix).is_err());
        assert_eq!(before, *world.local_transform(child).unwrap());
    }
    #[test]
    fn frustum_edges_clip_against_near_far_and_viewport_boundaries() {
        assert_eq!(
            clip_segment(
                Mat4::IDENTITY,
                Vec3::new(-2., 0., 0.5),
                Vec3::new(2., 0., 0.5)
            ),
            Some([[0., 0.5], [1., 0.5]])
        );
        assert_eq!(
            clip_segment(
                Mat4::IDENTITY,
                Vec3::new(0., 0., -1.),
                Vec3::new(0., 0., 2.)
            ),
            Some([[0.5, 0.5]; 2])
        );
        assert!(clip_segment(
            Mat4::IDENTITY,
            Vec3::new(0., 0., -2.),
            Vec3::new(0., 0., -1.)
        )
        .is_none());
        assert!(clip_segment(Mat4::IDENTITY, Vec3::splat(f32::NAN), Vec3::ZERO).is_none());
        let vp = Mat4::from_cols_array_2d(&[
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., -1., -1.],
            [0., 0., -0.1, 0.],
        ]);
        let edge = clip_segment(vp, Vec3::new(0., 0., 1.), Vec3::new(1., 0., -5.)).unwrap();
        assert!(edge
            .iter()
            .flatten()
            .all(|n| n.is_finite() && (0.0..=1.0).contains(n)));
    }
}
