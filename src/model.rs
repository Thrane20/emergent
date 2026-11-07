//! Imported glTF binary (`.glb`) models: the default scene's triangle meshes, with node
//! transforms applied, merged into one engine [`Mesh`] carrying normals and texture
//! coordinates, plus the base-colour image. Parsed once at scene load, never per frame.
use std::fmt;
use std::io::Read;

use glam::{Mat4, Vec2, Vec3};

use crate::{Color, Mesh, MeshSkin, MeshVertex, Project, Skeleton, TextureImage, Transform};

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
/// A model with a glTF skin also carries its skeleton, and its mesh carries the skin.
#[derive(Clone, Debug)]
pub struct ImportedModel {
    pub mesh: Mesh,
    pub texture: Option<TextureImage>,
    pub skeleton: Option<Skeleton>,
    /// Animation clips over the skeleton's joints; empty without a skeleton.
    pub clips: Vec<crate::AnimationClip>,
}

impl ImportedModel {
    /// Reads a project-relative `.glb` within the size budget.
    pub fn load(project: &Project, asset: &str) -> Result<Self, ModelError> {
        // VRM avatars are glTF binaries with extensions.
        if !(asset.ends_with(".glb") || asset.ends_with(".vrm")) {
            return Err(invalid("Models must be glTF binary (.glb or .vrm) files."));
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
        // Explicit stack: node depth comes from the file, so no recursion.
        let node_count = gltf.nodes().len();
        let mut parents = vec![None; node_count];
        let mut worlds = vec![None; node_count];
        let mut order = Vec::new();
        let mut stack: Vec<_> = scene.nodes().map(|node| (node, Mat4::IDENTITY)).collect();
        while let Some((node, parent)) = stack.pop() {
            let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
            if worlds[node.index()].replace(world).is_some() {
                return Err(invalid("Model nodes must form a tree."));
            }
            for child in node.children() {
                parents[child.index()] = Some(node.index());
                stack.push((child, world));
            }
            order.push(node);
        }
        let mut skins = order
            .iter()
            .filter(|n| n.mesh().is_some())
            .filter_map(|n| n.skin());
        let skin = skins.next();
        if skins.any(|other| Some(other.index()) != skin.as_ref().map(|s| s.index())) {
            return Err(invalid(
                "Models with more than one skin are not supported yet.",
            ));
        }
        let rig = skin
            .map(|skin| Rig::new(&skin, &parents, &worlds, blob))
            .transpose()?;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut skin_joints = Vec::new();
        let mut skin_weights = Vec::new();
        let mut image = None;
        for node in &order {
            let Some(mesh) = node.mesh() else { continue };
            let world = worlds[node.index()].expect("visited node has a transform");
            // glTF skinned vertices are already in bind (model) space; the node transform
            // is ignored. Unskinned meshes in a rig follow their nearest joint ancestor.
            let (matrix, rigid) = match &rig {
                Some(_) if node.skin().is_some() => (Mat4::IDENTITY, None),
                Some(rig) => match rig.joint_ancestor(node.index(), &parents) {
                    Some(j) => (
                        rig.rest_palette[j].inverse() * world,
                        Some(([j as u16, 0, 0, 0], [1., 0., 0., 0.])),
                    ),
                    None => (world, Some(([0; 4], [0.; 4]))),
                },
                None => (world, None),
            };
            let world = matrix;
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
                if let Some(rig) = &rig {
                    match rigid {
                        Some((joints, weights)) => {
                            skin_joints.extend(std::iter::repeat_n(joints, positions.len()));
                            skin_weights.extend(std::iter::repeat_n(weights, positions.len()));
                        }
                        None => rig.read_influences(
                            &reader,
                            positions.len(),
                            &mut skin_joints,
                            &mut skin_weights,
                        )?,
                    }
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
        let mut mesh = Mesh::new(vertices, indices).map_err(|e| invalid(e.to_string()))?;
        let clips = match &rig {
            Some(rig) => read_clips(&gltf, rig, buffer)?,
            None => Vec::new(),
        };
        let skeleton = match rig {
            Some(rig) => {
                mesh = mesh
                    .with_skin(MeshSkin {
                        joints: skin_joints,
                        weights: skin_weights,
                        rest_palette: rig.rest_palette,
                    })
                    .map_err(|e| invalid(e.to_string()))?;
                Some(rig.skeleton)
            }
            None => None,
        };
        Ok(Self {
            mesh,
            texture,
            skeleton,
            clips,
        })
    }
}

/// A glTF skin converted to a topologically sorted [`Skeleton`], with the mapping from
/// file joint order (used by vertex `JOINTS_0`) to skeleton order.
struct Rig {
    skeleton: Skeleton,
    rest_palette: Vec<glam::Mat4>,
    /// File joint index → skeleton joint index.
    remap: Vec<u16>,
    /// Node index → skeleton joint index.
    node_joint: Vec<Option<usize>>,
}

impl Rig {
    fn new(
        skin: &gltf::Skin,
        parents: &[Option<usize>],
        worlds: &[Option<Mat4>],
        blob: Option<&[u8]>,
    ) -> Result<Self, ModelError> {
        let joints: Vec<_> = skin.joints().collect();
        if joints.is_empty() || joints.len() > crate::SKELETON_MAX_JOINTS {
            return Err(invalid(format!(
                "Skins need 1 to {} joints.",
                crate::SKELETON_MAX_JOINTS
            )));
        }
        let mut file_joint = vec![None; parents.len()];
        for (k, joint) in joints.iter().enumerate() {
            if worlds[joint.index()].is_none() || file_joint[joint.index()].replace(k).is_some() {
                return Err(invalid(
                    "Skin joints must be distinct nodes in the model's scene.",
                ));
            }
        }
        let ancestor_joint = |node: usize| {
            let mut at = parents[node];
            while let Some(p) = at {
                if file_joint[p].is_some() {
                    return Some(p);
                }
                at = parents[p];
            }
            None
        };
        // Parents are joints directly; a plain node between two joints is not supported.
        let mut file_parent = Vec::with_capacity(joints.len());
        for joint in &joints {
            let direct = parents[joint.index()];
            let ancestor = ancestor_joint(joint.index());
            if ancestor.is_some() && ancestor != direct {
                return Err(invalid(
                    "Skin joints must be direct children of their parent joint.",
                ));
            }
            file_parent.push(ancestor.and_then(|p| file_joint[p]));
        }
        let depth = |mut k: usize| {
            let mut d = 0;
            while let Some(p) = file_parent[k] {
                (k, d) = (p, d + 1);
            }
            d
        };
        let mut sorted: Vec<usize> = (0..joints.len()).collect();
        sorted.sort_by_key(|&k| (depth(k), k));
        let mut remap = vec![0u16; joints.len()];
        for (new, &old) in sorted.iter().enumerate() {
            remap[old] = new as u16;
        }
        let inverse_bind: Vec<Mat4> = match skin
            .reader(|b| match b.source() {
                gltf::buffer::Source::Bin => blob,
                gltf::buffer::Source::Uri(_) => None,
            })
            .read_inverse_bind_matrices()
        {
            Some(read) => read.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
            None => vec![Mat4::IDENTITY; joints.len()],
        };
        if inverse_bind.len() != joints.len() {
            return Err(invalid(
                "Skin inverse bind matrices do not match its joints.",
            ));
        }
        let mut names = Vec::with_capacity(joints.len());
        let mut seen = std::collections::HashSet::new();
        let (mut parents_out, mut rest, mut binds) = (Vec::new(), Vec::new(), Vec::new());
        let mut node_joint = vec![None; parents.len()];
        for (new, &old) in sorted.iter().enumerate() {
            let joint = &joints[old];
            node_joint[joint.index()] = Some(new);
            let name = joint.name().unwrap_or("joint").to_string();
            names.push(if seen.insert(name.clone()) {
                name
            } else {
                format!("{name}#{old}")
            });
            parents_out.push(file_parent[old].map(|p| remap[p]));
            // Roots keep any non-joint ancestors' transforms, so the rest pose is in model space.
            let local = match file_parent[old] {
                Some(_) => Mat4::from_cols_array_2d(&joint.transform().matrix()),
                None => worlds[joint.index()].expect("checked in scene"),
            };
            rest.push(Transform::try_from_matrix(local).map_err(|_| {
                invalid(format!(
                    "Joint {:?} has a sheared or degenerate transform.",
                    joint.name()
                ))
            })?);
            binds.push(inverse_bind[old]);
        }
        let skeleton =
            Skeleton::new(names, parents_out, rest, binds).map_err(|e| invalid(e.to_string()))?;
        Ok(Self {
            rest_palette: skeleton.rest_palette(),
            skeleton,
            remap,
            node_joint,
        })
    }

    /// Nearest joint at or above `node`, as a skeleton index.
    fn joint_ancestor(&self, node: usize, parents: &[Option<usize>]) -> Option<usize> {
        let mut at = Some(node);
        while let Some(n) = at {
            if let Some(j) = self.node_joint[n] {
                return Some(j);
            }
            at = parents[n];
        }
        None
    }

    fn read_influences<'a, 's, F>(
        &self,
        reader: &gltf::mesh::Reader<'a, 's, F>,
        count: usize,
        joints: &mut Vec<[u16; 4]>,
        weights: &mut Vec<[f32; 4]>,
    ) -> Result<(), ModelError>
    where
        F: Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>,
    {
        if reader.read_joints(1).is_some() {
            return Err(invalid(
                "More than four joint influences per vertex are not supported.",
            ));
        }
        let (Some(read_joints), Some(read_weights)) =
            (reader.read_joints(0), reader.read_weights(0))
        else {
            return Err(invalid("Skinned meshes need JOINTS_0 and WEIGHTS_0."));
        };
        let start = joints.len();
        for (set, w) in read_joints.into_u16().zip(read_weights.into_f32()) {
            let mut mapped = [0u16; 4];
            for (out, &j) in mapped.iter_mut().zip(&set) {
                *out = *self
                    .remap
                    .get(j as usize)
                    .ok_or_else(|| invalid("Vertex joint index is outside its skin."))?;
            }
            let sum: f32 = w.iter().sum();
            if !sum.is_finite() || w.iter().any(|x| *x < 0.0) {
                return Err(invalid("Joint weights must be finite and non-negative."));
            }
            joints.push(mapped);
            weights.push(if sum > 0.0 {
                w.map(|x| x / sum)
            } else {
                [0.0; 4]
            });
        }
        if joints.len() - start != count {
            return Err(invalid("Skin attribute counts do not match positions."));
        }
        Ok(())
    }
}

/// Total keyframes across a model's clips, a bound on import work and memory.
const MAX_CLIP_KEYS: usize = 1_000_000;

/// glTF animations as clips over the rig's joints. Channels on non-joint nodes and
/// morph weights are skipped; cubic-spline keys keep their values, not their tangents.
fn read_clips<'a>(
    gltf: &'a gltf::Gltf,
    rig: &Rig,
    buffer: impl Clone + Fn(gltf::Buffer<'a>) -> Option<&'a [u8]>,
) -> Result<Vec<crate::AnimationClip>, ModelError> {
    use crate::{AnimationChannel, ChannelProperty, Interpolation};
    use gltf::animation::util::ReadOutputs;
    let mut clips = Vec::new();
    let mut names = std::collections::HashSet::new();
    let mut keys = 0usize;
    for animation in gltf.animations() {
        let mut channels = Vec::new();
        for channel in animation.channels() {
            let Some(joint) = rig.node_joint[channel.target().node().index()] else {
                continue;
            };
            let reader = channel.reader(buffer.clone());
            let times: Vec<f32> = reader
                .read_inputs()
                .ok_or_else(|| invalid("Animation channel has no key times."))?
                .collect();
            let (property, values): (_, Vec<glam::Vec4>) = match reader
                .read_outputs()
                .ok_or_else(|| invalid("Animation channel has no values."))?
            {
                ReadOutputs::Translations(v) => (
                    ChannelProperty::Translation,
                    v.map(|v| glam::Vec3::from_array(v).extend(0.)).collect(),
                ),
                ReadOutputs::Scales(v) => (
                    ChannelProperty::Scale,
                    v.map(|v| glam::Vec3::from_array(v).extend(0.)).collect(),
                ),
                ReadOutputs::Rotations(v) => (
                    ChannelProperty::Rotation,
                    v.into_f32().map(glam::Vec4::from_array).collect(),
                ),
                ReadOutputs::MorphTargetWeights(_) => continue,
            };
            let (interpolation, values) = match channel.sampler().interpolation() {
                gltf::animation::Interpolation::Linear => (Interpolation::Linear, values),
                gltf::animation::Interpolation::Step => (Interpolation::Step, values),
                // shortcut: in-tangent, value, out-tangent triplets keep only the value and
                // play linearly; add Hermite sampling if exported curves look wrong.
                gltf::animation::Interpolation::CubicSpline => (
                    Interpolation::Linear,
                    values.chunks(3).filter_map(|c| c.get(1).copied()).collect(),
                ),
            };
            keys += times.len();
            if keys > MAX_CLIP_KEYS {
                return Err(invalid("Model animations exceed 1,000,000 keyframes."));
            }
            channels.push(AnimationChannel {
                joint: joint as u16,
                property,
                interpolation,
                times,
                values,
            });
        }
        if channels.is_empty() {
            continue;
        }
        let name = animation.name().filter(|n| !n.is_empty()).map_or_else(
            || format!("animation {}", animation.index()),
            str::to_string,
        );
        let name = if names.insert(name.clone()) {
            name
        } else {
            format!("{name}#{}", animation.index())
        };
        clips.push(crate::AnimationClip::new(name, channels, rig.skeleton.len()).map_err(invalid)?);
    }
    Ok(clips)
}
