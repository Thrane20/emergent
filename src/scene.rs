//! Bounded authored primitive/light scenes. File I/O occurs only during loading.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use crate::{
    Camera, CameraProjection, Color, EntityId, Geometry, LightSource, Primitive, Project, Quat,
    ShadowCasting, Transform, Vec3, World,
};

const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_ENTITIES: usize = 1024;

/// A diagnostic identifying the authored file and invalid contract.
#[derive(Debug)]
pub struct SceneError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}
impl std::error::Error for SceneError {}

fn error(path: &str, message: impl fmt::Display) -> SceneError {
    SceneError {
        path: path.into(),
        message: message.to_string(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    schema_version: u32,
    id: String,
    name: Option<String>,
    builder: Option<String>,
    // Metadata written by the IDE's blank-scene template.
    kind: Option<String>,
    template: Option<String>,
    runtime_status: Option<String>,
    active_camera: Option<String>,
    entities: Vec<InstanceFile>,
    #[serde(default)]
    environment: crate::EnvironmentDefinition,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstanceFile {
    id: String,
    name: Option<String>,
    asset: String,
    parent: Option<String>,
    transform: Option<TransformOverride>,
    physics: Option<PhysicsFile>,
    material: Option<crate::MaterialDefinition>,
    camera: Option<CameraFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransformOverride {
    translation: Option<[f32; 3]>,
    rotation_degrees: Option<[f32; 3]>,
    scale: Option<[f32; 3]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntityFile {
    schema_version: u32,
    id: String,
    name: Option<String>,
    kind: String,
    template: Option<String>,
    runtime_status: Option<String>,
    #[serde(default)]
    transform: TransformFile,
    geometry: Option<GeometryFile>,
    terrain: Option<crate::TerrainSettings>,
    raymarched_terrain: Option<crate::RaymarchedTerrain>,
    terrain_heights: Option<String>,
    terrain_paint: Option<String>,
    structure: Option<String>,
    particles: Option<String>,
    light: Option<LightFile>,
    camera: Option<CameraFile>,
    physics: Option<PhysicsFile>,
    material: Option<crate::MaterialDefinition>,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TransformFile {
    translation: [f32; 3],
    rotation_degrees: [f32; 3],
    scale: [f32; 3],
}
impl Default for TransformFile {
    fn default() -> Self {
        Self {
            translation: [0.; 3],
            rotation_degrees: [0.; 3],
            scale: [1.; 3],
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "primitive", rename_all = "lowercase", deny_unknown_fields)]
enum GeometryFile {
    Cube {
        half_extent: f32,
        color: [f32; 4],
    },
    Plane {
        #[serde(default = "unit_half_extent")]
        half_width: f32,
        #[serde(default = "unit_half_extent")]
        half_depth: f32,
        color: [f32; 4],
    },
    Pyramid {
        half_extent: f32,
        height: f32,
        color: [f32; 4],
    },
    Sphere {
        radius: f32,
        sectors: u32,
        stacks: u32,
        color: [f32; 4],
    },
    Cylinder {
        radius: f32,
        half_height: f32,
        segments: u32,
        color: [f32; 4],
    },
    Cone {
        radius: f32,
        height: f32,
        segments: u32,
        color: [f32; 4],
    },
    Capsule {
        radius: f32,
        half_segment: f32,
        segments: u32,
        hemisphere_rings: u32,
        color: [f32; 4],
    },
    Torus {
        major_radius: f32,
        minor_radius: f32,
        major_segments: u32,
        minor_segments: u32,
        color: [f32; 4],
    },
}

fn unit_half_extent() -> f32 {
    0.5
}

impl GeometryFile {
    fn primitive(self) -> Result<Primitive, Box<dyn std::error::Error>> {
        let color = |[r, g, b, a]: [f32; 4]| Color::new(r, g, b, a);
        // Bound authored dimensions before tessellation can overflow its vertex math.
        let dimension = |n: f32| {
            if n.is_finite() && n <= 1_000_000.0 {
                Ok(n)
            } else {
                Err("scene dimensions must be finite and at most 1000000")
            }
        };
        // shortcut: at most 32 subdivisions per axis, raise only with a measured scene loading budget.
        let bounded = |n| {
            if n <= 32 {
                Ok(n)
            } else {
                Err("scene subdivisions must be at most 32")
            }
        };
        Ok(match self {
            Self::Cube {
                half_extent,
                color: c,
            } => Primitive::cube(dimension(half_extent)?, color(c)?)?,
            Self::Plane {
                half_width,
                half_depth,
                color: c,
            } => Primitive::plane(dimension(half_width)?, dimension(half_depth)?, color(c)?)?,
            Self::Pyramid {
                half_extent,
                height,
                color: c,
            } => Primitive::pyramid(dimension(half_extent)?, dimension(height)?, color(c)?)?,
            Self::Sphere {
                radius,
                sectors,
                stacks,
                color: c,
            } => {
                if sectors > 128 || stacks > 64 {
                    return Err(
                        "sphere subdivisions must be at most 128 sectors and 64 stacks".into(),
                    );
                }
                Primitive::sphere(dimension(radius)?, sectors, stacks, color(c)?)?
            }
            Self::Cylinder {
                radius,
                half_height,
                segments,
                color: c,
            } => Primitive::cylinder(
                dimension(radius)?,
                dimension(half_height)?,
                bounded(segments)?,
                color(c)?,
            )?,
            Self::Cone {
                radius,
                height,
                segments,
                color: c,
            } => Primitive::cone(
                dimension(radius)?,
                dimension(height)?,
                bounded(segments)?,
                color(c)?,
            )?,
            Self::Capsule {
                radius,
                half_segment,
                segments,
                hemisphere_rings,
                color: c,
            } => Primitive::capsule(
                dimension(radius)?,
                dimension(half_segment)?,
                bounded(segments)?,
                bounded(hemisphere_rings)?,
                color(c)?,
            )?,
            Self::Torus {
                major_radius,
                minor_radius,
                major_segments,
                minor_segments,
                color: c,
            } => Primitive::torus(
                dimension(major_radius)?,
                dimension(minor_radius)?,
                bounded(major_segments)?,
                bounded(minor_segments)?,
                color(c)?,
            )?,
        })
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PhysicsFile {
    enabled: bool,
    body: crate::BodyKind,
    gravity_scale: f32,
    mass: f32,
    friction: f32,
    restitution: f32,
    collider: ColliderFile,
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ColliderFile {
    shape: Option<String>,
    center: Option<[f32; 3]>,
    half_extents: Option<[f32; 3]>,
}
impl Default for PhysicsFile {
    fn default() -> Self {
        Self {
            enabled: true,
            body: crate::BodyKind::Dynamic,
            gravity_scale: 1.0,
            mass: 1.0,
            friction: 0.5,
            restitution: 0.0,
            collider: ColliderFile::default(),
        }
    }
}
impl PhysicsFile {
    fn resolve(
        self,
        geometry: Option<&Geometry>,
        terrain: Option<crate::TerrainSettings>,
    ) -> Result<Option<crate::PhysicsBody>, Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(None);
        }
        if !cfg!(feature = "physics") {
            return Err("enable the Emergent physics Cargo feature to load physics bodies".into());
        }
        if self.collider.shape.as_deref() == Some("terrain") {
            if self.collider.center.is_some() || self.collider.half_extents.is_some() {
                return Err(
                    "Terrain colliders use the terrain surface, without box dimensions.".into(),
                );
            }
            let settings = terrain.ok_or("Terrain collider requires a terrain entity asset.")?;
            let body = crate::PhysicsBody {
                body: self.body,
                gravity_scale: self.gravity_scale,
                mass: self.mass,
                friction: self.friction,
                restitution: self.restitution,
                collider: crate::TerrainCollider { settings }.into(),
            };
            body.validate()?;
            return Ok(Some(body));
        }
        if self
            .collider
            .shape
            .as_deref()
            .is_some_and(|shape| shape != "box")
        {
            return Err("Choose box or terrain collider.".into());
        }
        let fitted = if self.collider.half_extents.is_none() {
            let mesh = match geometry {
                Some(Geometry::Primitive(p)) => p.to_mesh()?,
                Some(Geometry::Mesh(m)) => m.clone(),
                _ => return Err("automatic box fitting requires geometry; supply explicit half_extents for empty entities".into()),
            };
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for vertex in mesh.vertices() {
                min = min.min(vertex.position());
                max = max.max(vertex.position());
            }
            Some((
                (min + max) * 0.5,
                ((max - min) * 0.5).max(Vec3::splat(0.01)),
            ))
        } else {
            None
        };
        let body = crate::PhysicsBody {
            body: self.body,
            gravity_scale: self.gravity_scale,
            mass: self.mass,
            friction: self.friction,
            restitution: self.restitution,
            collider: crate::BoxCollider {
                center: self
                    .collider
                    .center
                    .unwrap_or_else(|| fitted.map_or([0.0; 3], |(center, _)| center.to_array())),
                half_extents: self
                    .collider
                    .half_extents
                    .unwrap_or_else(|| fitted.expect("bounds fitted").1.to_array()),
            }
            .into(),
        };
        body.validate()?;
        Ok(Some(body))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LightFile {
    #[serde(rename = "type")]
    kind: String,
    color: [f32; 3],
    intensity: f32,
    range: Option<f32>,
    inner_half_angle: Option<f32>,
    outer_half_angle: Option<f32>,
    #[serde(default)]
    casts_shadows: bool,
}

impl LightFile {
    fn source(self) -> Result<LightSource, Box<dyn std::error::Error>> {
        let [r, g, b] = self.color;
        let color = Color::new(r, g, b, 1.)?;
        if self.kind != "spot"
            && (self.inner_half_angle.is_some() || self.outer_half_angle.is_some())
        {
            return Err("only spot lights accept cone angles".into());
        }
        if matches!(self.kind.as_str(), "ambient" | "directional") && self.range.is_some() {
            return Err("ambient/directional lights do not accept range".into());
        }
        let light = match self.kind.as_str() {
            "ambient" => LightSource::ambient(color, self.intensity)?,
            "directional" => LightSource::directional(color, self.intensity)?,
            "point" => LightSource::point(
                color,
                self.intensity,
                self.range.ok_or("point light requires range")?,
            )?,
            "spot" => LightSource::spot(
                color,
                self.intensity,
                self.range.ok_or("spot light requires range")?,
                self.inner_half_angle
                    .ok_or("spot requires inner_half_angle")?,
                self.outer_half_angle
                    .ok_or("spot requires outer_half_angle")?,
            )?,
            _ => return Err(format!("unsupported light type '{}'", self.kind).into()),
        };
        Ok(light.with_shadow_casting(if self.casts_shadows {
            ShadowCasting::Enabled
        } else {
            ShadowCasting::Disabled
        })?)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CameraFile {
    projection: String,
    field_of_view_degrees: Option<f32>,
    view_size: Option<f32>,
    near: f32,
    far: f32,
}

impl CameraFile {
    fn camera(self) -> Result<Camera, Box<dyn std::error::Error>> {
        let projection = match self.projection.as_str() {
            "perspective" => CameraProjection::perspective(
                self.field_of_view_degrees
                    .ok_or("perspective camera requires field_of_view_degrees")?,
                self.near,
                self.far,
            )?,
            "isometric" | "orthographic" => CameraProjection::isometric(
                self.view_size
                    .ok_or("isometric camera requires view_size")?,
                self.near,
                self.far,
            )?,
            other => return Err(format!("unsupported camera projection '{other}'").into()),
        };
        if self.projection != "perspective" && self.field_of_view_degrees.is_some() {
            return Err("only perspective cameras accept field_of_view_degrees".into());
        }
        if !matches!(self.projection.as_str(), "isometric" | "orthographic")
            && self.view_size.is_some()
        {
            return Err("only isometric cameras accept view_size".into());
        }
        Ok(Camera::new(projection))
    }
}

struct ValidatedEntity {
    raymarched_terrain: Option<crate::RaymarchedTerrain>,
    id: String,
    parent: Option<String>,
    name: String,
    transform: Transform,
    geometry: Option<Geometry>,
    light: Option<LightSource>,
    camera: Option<Camera>,
    physics: Option<crate::PhysicsBody>,
    material: Option<crate::Material>,
    terrain_field: Option<crate::TerrainField>,
    particles: Option<crate::ParticleEffect>,
}

/// Fully validated candidate; loading never mutates the active world.
pub struct SceneDefinition {
    path: String,
    name: String,
    builder: Option<String>,
    active_camera: Option<String>,
    entities: Vec<ValidatedEntity>,
    environment: crate::SceneEnvironment,
}

/// Construction results. Authored IDs resolve to fresh, world-scoped runtime keys.
pub struct LoadedScene {
    pub root: EntityId,
    pub environment: crate::SceneEnvironment,
    pub entities: HashMap<String, EntityId>,
    pub renderables: Vec<(EntityId, Geometry)>,
    pub lights: Vec<EntityId>,
    pub cameras: Vec<EntityId>,
    pub active_camera: Option<EntityId>,
}

fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

fn read<T: serde::de::DeserializeOwned>(project: &Project, path: &str) -> Result<T, SceneError> {
    read_bounded(project, path, MAX_FILE_BYTES)
}

fn read_bounded<T: serde::de::DeserializeOwned>(
    project: &Project,
    path: &str,
    limit: u64,
) -> Result<T, SceneError> {
    let result = (|| -> Result<T, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(project.asset_path(Path::new(path))?)?;
        let mut source = String::new();
        file.take(limit + 1).read_to_string(&mut source)?;
        if source.len() as u64 > limit {
            return Err(format!("asset file exceeds {} KiB", limit / 1024).into());
        }
        Ok(toml::from_str(&source)?)
    })();
    result.map_err(|e| error(path, e))
}

impl SceneDefinition {
    pub fn load(project: &Project, path: &str) -> Result<Self, SceneError> {
        let scene: SceneFile = read(project, path)?;
        Self::validate(project, path, scene)
    }

    /// Validates an editor candidate against the real project's asset definitions.
    /// No scene or runtime world is modified.
    pub fn from_source(project: &Project, path: &str, source: &str) -> Result<Self, SceneError> {
        project.asset_path(path).map_err(|e| error(path, e))?;
        if source.len() as u64 > MAX_FILE_BYTES {
            return Err(error(path, "scene file exceeds 256 KiB"));
        }
        let scene = toml::from_str(source).map_err(|e| error(path, e))?;
        Self::validate(project, path, scene)
    }

    fn validate(project: &Project, path: &str, scene: SceneFile) -> Result<Self, SceneError> {
        if scene.schema_version != 1 || !identifier(&scene.id) {
            return Err(error(
                path,
                "requires schema_version = 1 and a valid scene id",
            ));
        }
        if scene.entities.len() > MAX_ENTITIES {
            return Err(error(path, "scene exceeds 1024 instances"));
        }
        if scene.kind.as_deref().is_some_and(|k| k != "scene") {
            return Err(error(path, "scene kind must be scene"));
        }
        let _metadata = (scene.template, scene.runtime_status);
        let environment = scene
            .environment
            .resolve(project)
            .map_err(|e| error(path, e))?;
        let active_camera = scene.active_camera;
        let mut entities = Vec::with_capacity(scene.entities.len());
        let mut ids = HashSet::new();
        for instance in scene.entities {
            if !identifier(&instance.id) || !ids.insert(instance.id.clone()) {
                return Err(error(
                    path,
                    format!("invalid or duplicate instance id '{}'", instance.id),
                ));
            }
            if instance.name.as_ref().is_some_and(|name| {
                name.trim().is_empty()
                    || name.chars().count() > 64
                    || name.chars().any(char::is_control)
            }) {
                return Err(error(
                    path,
                    "instance name must contain 1–64 printable characters",
                ));
            }
            let asset = &instance.asset;
            let definition: EntityFile = read(project, asset)?;
            let result = (|| -> Result<ValidatedEntity, Box<dyn std::error::Error>> {
                if definition.schema_version != 1 || !identifier(&definition.id) {
                    return Err("requires schema_version = 1 and a valid asset id".into());
                }
                let _metadata = (definition.template, definition.runtime_status);
                let mut t = definition.transform;
                if let Some(overrides) = instance.transform {
                    if let Some(value) = overrides.translation {
                        t.translation = value;
                    }
                    if let Some(value) = overrides.rotation_degrees {
                        t.rotation_degrees = value;
                    }
                    if let Some(value) = overrides.scale {
                        t.scale = value;
                    }
                }
                if !t
                    .translation
                    .iter()
                    .chain(t.rotation_degrees.iter())
                    .chain(t.scale.iter())
                    .all(|v| v.is_finite())
                    || t.scale.iter().any(|v| v.abs() < 1e-4)
                {
                    return Err("transform values must be finite and scale axes non-zero".into());
                }
                let [x, y, z] = t.rotation_degrees.map(f32::to_radians);
                let transform = Transform::new(
                    Vec3::from_array(t.translation),
                    Quat::from_euler(glam::EulerRot::XYZ, x, y, z),
                    Vec3::from_array(t.scale),
                );
                let raymarched_terrain = definition.raymarched_terrain;
                if let Some(field) = raymarched_terrain {
                    field.validate()?;
                    if definition.kind != "raymarched-terrain"
                        || definition.terrain.is_some()
                        || definition.terrain_heights.is_some()
                        || definition.terrain_paint.is_some()
                        || definition.geometry.is_some()
                        || definition.structure.is_some()
                        || definition.particles.is_some()
                        || definition.physics.is_some()
                        || instance.physics.is_some()
                        || definition.material.is_some()
                        || instance.material.is_some()
                    {
                        return Err("Ray-marched terrain cannot contain heightfields, other geometry, materials or physics; collision is not implemented.".into());
                    }
                }
                if definition.structure.is_some()
                    && (definition.terrain.is_some()
                        || definition.geometry.is_some()
                        || definition.kind != "structure")
                {
                    return Err("Structure assets require kind=structure without terrain or primitive geometry.".into());
                }
                if definition.terrain.is_some() && definition.geometry.is_some() {
                    return Err("Terrain assets cannot also define primitive geometry.".into());
                }
                let terrain = definition.terrain;
                let geometry = definition
                    .geometry
                    .map(GeometryFile::primitive)
                    .transpose()?
                    .map(Geometry::from);
                let sculpted = definition.terrain_heights.is_some();
                let terrain_field = match (terrain, definition.terrain_heights) {
                    (Some(settings), Some(path)) => {
                        #[derive(Deserialize)]
                        #[serde(deny_unknown_fields)]
                        struct Heights {
                            schema_version: u32,
                            values: Vec<f32>,
                        }
                        let saved: Heights =
                            read_bounded(project, &path, crate::terrain::TERRAIN_DATA_MAX_BYTES)?;
                        if saved.schema_version != 1 {
                            return Err("Unsupported terrain heightfield version.".into());
                        }
                        Some(crate::TerrainField::from_heights(settings, saved.values)?)
                    }
                    (Some(settings), None) => Some(settings.generate()?),
                    (None, Some(_)) => {
                        return Err("Terrain heights require a terrain recipe.".into())
                    }
                    (None, None) => None,
                };
                let geometry = match &terrain_field {
                    Some(field) => Some(Geometry::Mesh(field.mesh(Color::WHITE)?)),
                    None => geometry,
                };
                let geometry = match &definition.structure {
                    Some(path) => Some(Geometry::Mesh(
                        crate::StructureDefinition::load(project, path)?.mesh()?,
                    )),
                    None => geometry,
                };
                let particles = definition
                    .particles
                    .as_deref()
                    .map(|p| crate::ParticleDefinition::load(project, p)?.resolve(project))
                    .transpose()?;
                if particles.is_some()
                    && (definition.kind != "particle-effect"
                        || geometry.is_some()
                        || definition.light.is_some()
                        || definition.camera.is_some()
                        || definition.physics.is_some()
                        || instance.physics.is_some())
                {
                    return Err(
                        "Particle effects cannot contain geometry, lights, cameras or physics."
                            .into(),
                    );
                }
                let light = definition.light.map(LightFile::source).transpose()?;
                if instance.camera.is_some() && definition.kind != "camera" {
                    return Err("Camera overrides require a camera asset.".into());
                }
                let camera = instance
                    .camera
                    .or(definition.camera)
                    .map(CameraFile::camera)
                    .transpose()?;
                match definition.kind.as_str() {
                    "raymarched-terrain"
                        if raymarched_terrain.is_some()
                            && geometry.is_none()
                            && light.is_none()
                            && camera.is_none() => {}
                    "particle-effect"
                        if particles.is_some()
                            && geometry.is_none()
                            && light.is_none()
                            && camera.is_none() => {}
                    "terrain"
                        if terrain.is_some()
                            && geometry.is_some()
                            && light.is_none()
                            && camera.is_none() => {}
                    "structure"
                        if definition.structure.is_some()
                            && geometry.is_some()
                            && light.is_none()
                            && camera.is_none() => {}
                    "primitive"
                        if terrain.is_none()
                            && geometry.is_some()
                            && light.is_none()
                            && camera.is_none() => {}
                    "entity" if geometry.is_none() && light.is_none() && camera.is_none() => (),
                    "camera" if geometry.is_none() && light.is_none() && camera.is_some() => (),
                    kind if kind.ends_with("-light")
                        && light.is_some()
                        && geometry.is_none()
                        && camera.is_none() =>
                    {
                        let expected = format!(
                            "{}-light",
                            format!("{:?}", light.unwrap().kind()).to_lowercase()
                        );
                        if kind != expected {
                            return Err("entity kind and light type disagree".into());
                        }
                    }
                    _ => {
                        return Err(format!(
                            "unsupported or inconsistent entity kind '{}'",
                            definition.kind
                        )
                        .into())
                    }
                }
                let physics = instance
                    .physics
                    .or(definition.physics)
                    .map(|p| p.resolve(geometry.as_ref(), terrain))
                    .transpose()?
                    .flatten();
                let surface = if let Some(path) = definition.terrain_paint {
                    Some(crate::MaterialDefinition {
                        terrain_paint: Some(path),
                        shading: crate::MaterialShading::Smooth,
                        ..Default::default()
                    })
                } else {
                    instance.material.or(definition.material)
                };
                let material = surface
                    .map(|definition| {
                        if geometry.is_none() {
                            return Err(crate::MaterialError(
                                "Materials require a renderable geometry asset.".into(),
                            ));
                        }
                        let material = definition.resolve(project)?;
                        if let Some(paint) = &material.terrain {
                            if terrain.is_none_or(|settings| {
                                settings.resolution != paint.paint.resolution
                            }) {
                                return Err(crate::MaterialError(
                                    "Terrain paint requires a terrain with matching resolution."
                                        .into(),
                                ));
                            }
                        }
                        Ok(material)
                    })
                    .transpose()?;
                Ok(ValidatedEntity {
                    raymarched_terrain,
                    id: instance.id,
                    parent: instance.parent,
                    name: instance.name.or(definition.name).unwrap_or(definition.id),
                    transform,
                    geometry,
                    light,
                    camera,
                    physics,
                    material,
                    particles,
                    terrain_field: if sculpted { terrain_field } else { None },
                })
            })();
            entities.push(result.map_err(|e| error(asset, e))?);
        }
        if entities
            .iter()
            .filter(|e| e.raymarched_terrain.is_some())
            .count()
            > 8
        {
            return Err(error(
                path,
                "Scene supports at most eight ray-marched terrain regions.",
            ));
        }
        let particle_capacity: u64 = entities
            .iter()
            .filter_map(|e| e.particles.as_ref())
            .flat_map(|effect| &effect.definition().emitters)
            .map(|e| u64::from(e.max_particles))
            .sum();
        if particle_capacity > crate::PARTICLE_MAX_LIVE as u64 {
            return Err(error(path, "Scene particle capacity exceeds 100,000."));
        }
        let parents: HashMap<_, _> = entities
            .iter()
            .map(|e| (e.id.as_str(), e.parent.as_deref()))
            .collect();
        for entity in &entities {
            if entity
                .physics
                .is_some_and(|p| p.body == crate::BodyKind::Dynamic)
                && entity.parent.is_some()
            {
                return Err(error(
                    path,
                    format!("dynamic instance '{}' must be at the scene root", entity.id),
                ));
            }
            let mut cursor = entity.parent.as_deref();
            for depth in 0..=entities.len() {
                let Some(parent) = cursor else { break };
                if parent == entity.id || depth == entities.len() {
                    return Err(error(path, format!("hierarchy cycle at '{}'", entity.id)));
                }
                cursor = *parents.get(parent).ok_or_else(|| {
                    error(
                        path,
                        format!("instance '{}' has missing parent '{parent}'", entity.id),
                    )
                })?;
            }
        }
        if entities.iter().filter(|e| e.light.is_some()).count() > 8 {
            return Err(error(path, "scene supports at most eight lights"));
        }
        // Check composed matrices before construction, including overflow from deep scaling.
        let by_id: HashMap<_, _> = entities.iter().map(|e| (e.id.as_str(), e)).collect();
        if let Some(id) = active_camera.as_deref() {
            let camera = by_id
                .get(id)
                .ok_or_else(|| error(path, format!("active camera '{id}' is missing")))?;
            if camera.camera.is_none() {
                return Err(error(
                    path,
                    format!("active camera '{id}' does not reference a camera entity"),
                ));
            }
        }
        for entity in &entities {
            let matrix_for = |t: Transform| {
                crate::Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation)
            };
            let mut matrix = matrix_for(entity.transform);
            let mut parent = entity.parent.as_deref();
            while let Some(id) = parent {
                let ancestor = by_id[id];
                matrix = matrix_for(ancestor.transform) * matrix;
                parent = ancestor.parent.as_deref();
            }
            if let Some(physics) = entity.physics {
                if physics.body == crate::BodyKind::Dynamic
                    && entity.transform.scale.min_element() <= 0.0
                {
                    return Err(error(
                        path,
                        format!(
                            "dynamic instance {} requires positive local scale",
                            entity.id
                        ),
                    ));
                }
                crate::physics::validate_pose(physics, matrix)
                    .map_err(|e| error(path, format!("instance {}: {e}", entity.id)))?;
            }
            if !matrix.is_finite() || !matrix.inverse().is_finite() {
                return Err(error(
                    path,
                    format!(
                        "instance '{}' has a non-invertible or overflowing world transform",
                        entity.id
                    ),
                ));
            }
        }
        Ok(Self {
            path: path.into(),
            name: scene.name.unwrap_or(scene.id),
            builder: scene.builder,
            environment,
            active_camera,
            entities,
        })
    }

    /// Optional project-owned extension. The engine never executes this identifier.
    pub fn builder(&self) -> Option<&str> {
        self.builder.as_deref()
    }

    pub fn instantiate(self, world: &mut World) -> Result<LoadedScene, SceneError> {
        let root = world.spawn(self.name);
        let active_camera_name = self.active_camera.clone();
        let result = (|| -> Result<LoadedScene, crate::WorldError> {
            let mut entities = HashMap::with_capacity(self.entities.len());
            let mut renderables = Vec::new();
            let mut lights = Vec::new();
            let mut cameras = Vec::new();
            for entity in &self.entities {
                let id = world.spawn_child(root, entity.name.clone())?;
                world.set_local_transform(id, entity.transform)?;
                if let Some(field) = entity.raymarched_terrain {
                    world.insert_component(id, field)?;
                }
                if let Some(effect) = &entity.particles {
                    world.insert_component(id, effect.clone())?;
                }
                if let Some(material) = &entity.material {
                    world.insert_component(id, material.clone())?;
                }
                if let Some(physics) = entity.physics {
                    world.insert_component(id, physics)?;
                }
                if let Some(light) = entity.light {
                    world.insert_component(id, light)?;
                    lights.push(id);
                }
                if let Some(camera) = entity.camera {
                    world.insert_component(id, camera)?;
                    cameras.push(id);
                }
                entities.insert(entity.id.clone(), id);
            }
            for entity in self.entities {
                let id = entities[&entity.id];
                if let Some(parent) = entity.parent {
                    world.reparent(id, Some(entities[&parent]))?;
                }
                if let Some(field) = entity.terrain_field {
                    #[cfg(feature = "physics")]
                    world.insert_component(id, crate::terrain::AuthoredTerrain(field))?;
                    #[cfg(not(feature = "physics"))]
                    drop(field);
                }
                if let Some(geometry) = entity.geometry {
                    renderables.push((id, geometry));
                }
            }
            let active_camera = active_camera_name.as_ref().map(|name| entities[name]);
            Ok(LoadedScene {
                root,
                environment: self.environment.clone(),
                entities,
                renderables,
                lights,
                cameras,
                active_camera,
            })
        })();
        match result {
            Ok(scene) => Ok(scene),
            Err(e) => {
                let _ = world.remove_subtree(root);
                Err(error(&self.path, e))
            }
        }
    }
}
