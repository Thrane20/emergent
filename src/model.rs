//! Imported glTF binary (`.glb`) models: the default scene's triangle meshes, with node
//! transforms applied, merged into one engine [`Mesh`] carrying normals and texture
//! coordinates, plus the base-colour image. Parsed once at scene load, never per frame.
use std::fmt;
use std::io::Read;

use glam::{Mat4, Vec2, Vec3};

use crate::{Color, Mesh, MeshVertex, Project, TextureImage};

/// Same ceiling as Object Creator downloads.
pub const MODEL_MAX_BYTES: usize = 100 * 1024 * 1024;
pub const MODEL_MAX_VERTICES: usize = 1_000_000;
pub const MODEL_MAX_TRIANGLES: usize = 500_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError(pub String);

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ModelError {}

fn invalid(message: impl Into<String>) -> ModelError {
    ModelError(message.into())
}

/// A loaded model: one mesh in the model's own space and its base-colour image, if any.
#[derive(Clone, Debug)]
pub struct ImportedModel {
    pub mesh: Mesh,
    pub texture: Option<TextureImage>,
}

impl ImportedModel {
    /// Reads a project-relative `.glb` within the size budget.
    pub fn load(project: &Project, asset: &str) -> Result<Self, ModelError> {
        if !asset.ends_with(".glb") {
            return Err(invalid("Models must be glTF binary (.glb) files."));
        }
        let path = project
            .asset_path(asset)
            .map_err(|e| invalid(e.to_string()))?;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|f| f.take(MODEL_MAX_BYTES as u64 + 1).read_to_end(&mut bytes))
            .map_err(|e| invalid(e.to_string()))?;
        Self::from_glb(&bytes)
    }

    pub fn from_glb(bytes: &[u8]) -> Result<Self, ModelError> {
        if bytes.len() > MODEL_MAX_BYTES {
            return Err(invalid("Model exceeds 100 MiB."));
        }
        let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
        let blob = gltf.blob.as_deref();
        // Only the GLB's own binary chunk is read; external or data-URI buffers are refused.
        let buffer = |buffer: gltf::Buffer| match buffer.source() {
            gltf::buffer::Source::Bin => blob,
            gltf::buffer::Source::Uri(_) => None,
        };
        let scene = gltf
            .default_scene()
            .or_else(|| gltf.scenes().next())
            .ok_or_else(|| invalid("Model has no scene."))?;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut image = None;
        // Explicit stack: node depth comes from the file, so no recursion.
        let mut stack: Vec<_> = scene.nodes().map(|node| (node, Mat4::IDENTITY)).collect();
        while let Some((node, parent)) = stack.pop() {
            let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
            stack.extend(node.children().map(|child| (child, world)));
            let Some(mesh) = node.mesh() else { continue };
            let normal_matrix = world.inverse().transpose();
            for primitive in mesh.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(invalid("Only triangle meshes are supported."));
                }
                let pbr = primitive.material().pbr_metallic_roughness();
                let tint = pbr.base_color_factor();
                if let Some(info) = pbr.base_color_texture() {
                    let index = info.texture().source().index();
                    match image {
                        None => image = Some(index),
                        Some(existing) if existing != index => return Err(invalid(
                            "Models with more than one base-colour texture are not supported yet.",
                        )),
                        Some(_) => {}
                    }
                }
                let reader = primitive.reader(buffer);
                let positions: Vec<[f32; 3]> = reader
                    .read_positions()
                    .ok_or_else(|| invalid("Mesh primitive has no positions."))?
                    .collect();
                let normals: Option<Vec<[f32; 3]>> = reader.read_normals().map(Iterator::collect);
                let uvs: Option<Vec<[f32; 2]>> =
                    reader.read_tex_coords(0).map(|t| t.into_f32().collect());
                let colors: Option<Vec<[f32; 4]>> =
                    reader.read_colors(0).map(|c| c.into_rgba_f32().collect());
                let base = vertices.len();
                if base + positions.len() > MODEL_MAX_VERTICES {
                    return Err(invalid("Model exceeds 1,000,000 vertices."));
                }
                for (i, position) in positions.iter().enumerate() {
                    let rgba = colors
                        .as_ref()
                        .and_then(|c| c.get(i))
                        .copied()
                        .unwrap_or([1.; 4]);
                    let color = Color::new(
                        rgba[0] * tint[0],
                        rgba[1] * tint[1],
                        rgba[2] * tint[2],
                        rgba[3] * tint[3],
                    )
                    .map_err(|e| invalid(e.to_string()))?;
                    let point = world.transform_point3(Vec3::from_array(*position));
                    let normal = normals
                        .as_ref()
                        .and_then(|n| n.get(i))
                        .map(|n| normal_matrix.transform_vector3(Vec3::from_array(*n)))
                        .filter(|n| n.length_squared() > 0.);
                    let vertex = match normal {
                        Some(normal) => MeshVertex::with_normal(point, normal, color),
                        None => MeshVertex::new(point, color),
                    }
                    .map_err(|e| invalid(e.to_string()))?;
                    let vertex = match uvs.as_ref().and_then(|t| t.get(i)) {
                        Some(uv) => vertex.with_uv(Vec2::from_array(*uv)),
                        None => Ok(vertex),
                    }
                    .map_err(|e| invalid(e.to_string()))?;
                    vertices.push(vertex);
                }
                match reader.read_indices() {
                    Some(read) => {
                        for index in read.into_u32() {
                            if index as usize >= positions.len() {
                                return Err(invalid("Mesh index is out of range."));
                            }
                            indices.push(base as u32 + index);
                        }
                    }
                    None => indices.extend((0..positions.len() as u32).map(|i| base as u32 + i)),
                }
                if indices.len() / 3 > MODEL_MAX_TRIANGLES {
                    return Err(invalid("Model exceeds 500,000 triangles."));
                }
            }
        }
        if indices.len() % 3 != 0 {
            return Err(invalid("Mesh index count is not a multiple of three."));
        }
        let texture = match image {
            None => None,
            Some(index) => {
                let image = gltf
                    .images()
                    .nth(index)
                    .ok_or_else(|| invalid("Missing texture image."))?;
                let gltf::image::Source::View { view, .. } = image.source() else {
                    return Err(invalid("Model textures must be embedded in the GLB."));
                };
                let data = blob.ok_or_else(|| invalid("Model has no binary chunk."))?;
                let bytes = data
                    .get(view.offset()..view.offset() + view.length())
                    .ok_or_else(|| invalid("Texture data is out of range."))?;
                Some(TextureImage::from_bytes(bytes).map_err(|e| invalid(e.to_string()))?)
            }
        };
        let mesh = Mesh::new(vertices, indices).map_err(|e| invalid(e.to_string()))?;
        Ok(Self { mesh, texture })
    }
}
