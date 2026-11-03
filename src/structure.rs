//! Editable authoring graphs compiled into ordinary mesh resources before rendering.
use crate::{Color, Mat4, Mesh, MeshVertex, Project, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::Read,
};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StructureTransform {
    pub translation: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub scale: [f32; 3],
}
impl Default for StructureTransform {
    fn default() -> Self {
        Self {
            translation: [0.; 3],
            rotation_degrees: [0.; 3],
            scale: [1.; 3],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolygonMesh {
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<Vec<u32>>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructurePart {
    pub id: String,
    pub name: String,
    pub parent: Option<String>,
    #[serde(default)]
    pub transform: StructureTransform,
    pub mesh: PolygonMesh,
}
/// A durable authoring hierarchy, not a runtime object store. Parts compile into one mesh.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructureDefinition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub parts: Vec<StructurePart>,
}
#[derive(Debug)]
pub struct StructureError(pub String);
impl fmt::Display for StructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for StructureError {}
fn invalid(message: &str) -> StructureError {
    StructureError(message.into())
}
fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}
fn name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 64 && !name.chars().any(char::is_control)
}
const EPS: f64 = 1e-8;
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn on(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> bool {
    cross(a, b, p).abs() <= EPS
        && (0..2).all(|i| p[i] >= a[i].min(b[i]) - EPS && p[i] <= a[i].max(b[i]) + EPS)
}
impl PolygonMesh {
    /// Dominant-plane ear clipping, preserving polygon winding and authored indices.
    pub fn triangulate_face(&self, face: usize) -> Result<Vec<u32>, StructureError> {
        let indices = self
            .faces
            .get(face)
            .ok_or_else(|| invalid("Face is missing."))?;
        if !(3..=64).contains(&indices.len())
            || indices.iter().any(|i| *i as usize >= self.vertices.len())
            || indices.iter().collect::<HashSet<_>>().len() != indices.len()
        {
            return Err(invalid("Faces require 3–64 distinct valid vertex indices."));
        }
        let mut normal = [0f64; 3];
        for i in 0..indices.len() {
            let a = self.vertices[indices[i] as usize].map(f64::from);
            let b = self.vertices[indices[(i + 1) % indices.len()] as usize].map(f64::from);
            normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
            normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
            normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        if normal.iter().map(|n| n * n).sum::<f64>().sqrt() < EPS {
            return Err(invalid("Face is collapsed."));
        }
        let mut axis = 0;
        for i in 1..3 {
            if normal[i].abs() > normal[axis].abs() {
                axis = i;
            }
        }
        let axes: Vec<_> = (0..3).filter(|i| *i != axis).collect();
        let points: Vec<_> = indices
            .iter()
            .map(|i| {
                [
                    f64::from(self.vertices[*i as usize][axes[0]]),
                    f64::from(self.vertices[*i as usize][axes[1]]),
                ]
            })
            .collect();
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                if j == i + 1 || (i == 0 && j == points.len() - 1) {
                    continue;
                }
                let (a, b, c, d) = (
                    points[i],
                    points[(i + 1) % points.len()],
                    points[j],
                    points[(j + 1) % points.len()],
                );
                if (cross(a, b, c) * cross(a, b, d) < -EPS
                    && cross(c, d, a) * cross(c, d, b) < -EPS)
                    || on(a, b, c)
                    || on(a, b, d)
                    || on(c, d, a)
                    || on(c, d, b)
                {
                    return Err(invalid("Face edges intersect."));
                }
            }
        }
        let area = points
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let b = points[(i + 1) % points.len()];
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>();
        if area.abs() <= EPS {
            return Err(invalid("Face is collapsed."));
        }
        let sign = area.signum();
        let mut remaining: Vec<_> = (0..indices.len()).collect();
        let mut triangles = Vec::with_capacity((indices.len() - 2) * 3);
        for i in (0..remaining.len()).rev() {
            if remaining.len() <= 3 {
                break;
            }
            let n = remaining.len();
            if cross(
                points[remaining[(i + n - 1) % n]],
                points[remaining[i]],
                points[remaining[(i + 1) % n]],
            )
            .abs()
                <= EPS
            {
                remaining.remove(i);
            }
        }
        while remaining.len() > 3 {
            let mut clipped = false;
            for i in 0..remaining.len() {
                let a = remaining[(i + remaining.len() - 1) % remaining.len()];
                let b = remaining[i];
                let c = remaining[(i + 1) % remaining.len()];
                if cross(points[a], points[b], points[c]) * sign <= EPS {
                    continue;
                }
                if remaining.iter().any(|p| {
                    *p != a
                        && *p != b
                        && *p != c
                        && cross(points[a], points[b], points[*p]) * sign >= -EPS
                        && cross(points[b], points[c], points[*p]) * sign >= -EPS
                        && cross(points[c], points[a], points[*p]) * sign >= -EPS
                }) {
                    continue;
                }
                triangles.extend([indices[a], indices[b], indices[c]]);
                remaining.remove(i);
                clipped = true;
                break;
            }
            if !clipped {
                return Err(invalid("Face cannot be triangulated."));
            }
        }
        triangles.extend(remaining.into_iter().map(|i| indices[i]));
        Ok(triangles)
    }
}
impl StructureDefinition {
    pub fn load(project: &Project, asset: &str) -> Result<Self, StructureError> {
        let path = project
            .asset_path(asset)
            .map_err(|e| StructureError(e.to_string()))?;
        let mut source = String::new();
        std::fs::File::open(path)
            .and_then(|f| f.take(2 * 1024 * 1024 + 1).read_to_string(&mut source))
            .map_err(|e| StructureError(e.to_string()))?;
        if source.len() > 2 * 1024 * 1024 {
            return Err(invalid("Structure asset exceeds 2 MiB."));
        }
        let definition: Self =
            toml::from_str(&source).map_err(|e| StructureError(e.to_string()))?;
        definition.mesh()?;
        Ok(definition)
    }
    /// Validates and assembles part transforms into a single ordinary mesh resource.
    pub fn mesh(&self) -> Result<Mesh, StructureError> {
        if self.schema_version != 1
            || !identifier(&self.id)
            || !name(&self.name)
            || self.parts.is_empty()
            || self.parts.len() > 128
        {
            return Err(invalid("Use schema 1, a valid ID/name and 1–128 parts."));
        }
        let mut by_id = HashMap::with_capacity(self.parts.len());
        let mut children: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
        let mut count = 0usize;
        for (i, p) in self.parts.iter().enumerate() {
            if !identifier(&p.id) || !name(&p.name) || by_id.insert(p.id.as_str(), i).is_some() {
                return Err(invalid("Part IDs must be unique, with valid names."));
            }
            if p.transform
                .translation
                .iter()
                .chain(&p.transform.rotation_degrees)
                .chain(&p.transform.scale)
                .any(|n| !n.is_finite() || n.abs() > 10000.)
                || p.transform.scale.iter().any(|n| n.abs() < 0.0001)
            {
                return Err(invalid(
                    "Part transforms must be finite with nonzero scale.",
                ));
            }
            if !(p.mesh.vertices.is_empty() && p.mesh.faces.is_empty())
                && (!(3..=4096).contains(&p.mesh.vertices.len())
                    || !(1..=8192).contains(&p.mesh.faces.len()))
                || p.mesh
                    .vertices
                    .iter()
                    .flatten()
                    .any(|n| !n.is_finite() || n.abs() > 10000.)
            {
                return Err(invalid(
                    "Part meshes exceed vertex/face limits or contain invalid positions.",
                ));
            }
            count += p.mesh.vertices.len();
            children.entry(p.parent.as_deref()).or_default().push(i);
        }
        if count > 65536 {
            return Err(invalid("Structure exceeds 65536 vertices."));
        }
        if self.parts.iter().any(|p| {
            p.parent
                .as_ref()
                .is_some_and(|id| !by_id.contains_key(id.as_str()))
        }) {
            return Err(invalid("Part parent is missing."));
        }
        let mut matrices = vec![Mat4::IDENTITY; self.parts.len()];
        let mut stack = children.get(&None).cloned().unwrap_or_default();
        let mut visited = 0;
        while let Some(i) = stack.pop() {
            let p = &self.parts[i];
            let [x, y, z] = p.transform.rotation_degrees.map(f32::to_radians);
            let mut matrix = Mat4::from_scale_rotation_translation(
                Vec3::from_array(p.transform.scale),
                Quat::from_euler(glam::EulerRot::XYZ, x, y, z),
                Vec3::from_array(p.transform.translation),
            );
            if let Some(parent) = p.parent.as_deref() {
                matrix = matrices[by_id[parent]] * matrix;
            }
            if !matrix.is_finite() || !matrix.inverse().is_finite() {
                return Err(invalid("Part world transform is singular or overflowing."));
            }
            matrices[i] = matrix;
            visited += 1;
            if let Some(group) = children.get(&Some(p.id.as_str())) {
                stack.extend(group);
            }
        }
        if visited != self.parts.len() {
            return Err(invalid("Part parenting creates a cycle."));
        }
        let mut vertices = Vec::with_capacity(count);
        let mut indices = Vec::new();
        for (part, matrix) in self.parts.iter().zip(matrices) {
            let offset = vertices.len() as u32;
            for point in &part.mesh.vertices {
                let point = matrix.transform_point3(Vec3::from_array(*point));
                if !point.is_finite() || point.abs().max_element() > 1000000. {
                    return Err(invalid("Assembled structure exceeds finite metre bounds."));
                }
                vertices.push(
                    MeshVertex::new(point, Color::WHITE)
                        .map_err(|e| StructureError(e.to_string()))?,
                );
            }
            for face in 0..part.mesh.faces.len() {
                let mut triangles = part.mesh.triangulate_face(face)?;
                if matrix.determinant() < 0. {
                    for tri in triangles.as_chunks_mut::<3>().0 {
                        tri.swap(1, 2);
                    }
                }
                indices.extend(triangles.into_iter().map(|index| offset + index));
                if indices.len() > 131072 * 3 {
                    return Err(invalid("Structure exceeds 131072 triangles."));
                }
            }
        }
        Mesh::new(vertices, indices).map_err(|e| StructureError(e.to_string()))
    }
}
