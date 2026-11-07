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
/// Instances per scene. Map editing (waypoints and zones are instances) needs room.
pub const SCENE_MAX_INSTANCES: usize = 4096;
/// Scene file size; 4,096 instances with transforms and components fit with margin.
pub const SCENE_MAX_BYTES: u64 = 2 * 1024 * 1024;

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
    wind: Option<crate::WindDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstanceFile {
    prefab_label: Option<String>,
    classification: Option<crate::ObjectClassification>,
    id: String,
    name: Option<String>,
    asset: String,
    parent: Option<String>,
    transform: Option<TransformOverride>,
    physics: Option<PhysicsFile>,
    material: Option<crate::MaterialDefinition>,
    camera: Option<CameraFile>,
    #[serde(default)]
    components: Vec<ComponentFile>,
    /// Brain-local authored IDs mapped to this instance's scene IDs.
    #[serde(default)]
    brain_bindings: HashMap<String, String>,
}

/// An authored `[[entities.components]]` entry; fields are checked against the registry schema.
#[derive(Deserialize)]
struct ComponentFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default = "enabled_default")]
    enabled: bool,
    #[serde(flatten)]
    fields: toml::Table,
}
fn enabled_default() -> bool {
    true
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
    classification: Option<crate::ObjectClassification>,
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
    /// A project-relative `.glb` for `kind = "model"` assets.
    model: Option<String>,
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

#[derive(Clone, Deserialize)]
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
#[derive(Clone, Default, Deserialize)]
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
    /// Overrides from a schema-validated `emergent.light` component.
    fn apply(&mut self, fields: &toml::Table) {
        let number = |value: &toml::Value| {
            value
                .as_float()
                .or(value.as_integer().map(|n| n as f64))
                .map(|n| n as f32)
        };
        for (name, value) in fields {
            match name.as_str() {
                "color" => {
                    if let Some(color) = value.as_array().and_then(|c| {
                        <[f32; 3]>::try_from(c.iter().map(number).collect::<Option<Vec<_>>>()?).ok()
                    }) {
                        self.color = color;
                    }
                }
                "intensity" => self.intensity = number(value).unwrap_or(self.intensity),
                "range" => self.range = number(value).or(self.range),
                "inner_half_angle" => {
                    self.inner_half_angle = number(value).or(self.inner_half_angle)
                }
                "outer_half_angle" => {
                    self.outer_half_angle = number(value).or(self.outer_half_angle)
                }
                "casts_shadows" => {
                    self.casts_shadows = value.as_bool().unwrap_or(self.casts_shadows)
                }
                _ => {}
            }
        }
    }

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

#[derive(Clone)]
struct ValidatedEntity {
    classification: crate::ObjectClassification,
    raymarched_terrain: Option<crate::RaymarchedTerrain>,
    id: String,
    parent: Option<String>,
    name: String,
    transform: Transform,
    geometry: Option<Geometry>,
    light: Option<LightSource>,
    camera: Option<Camera>,
    physics: Option<crate::PhysicsBody>,
    pending_physics: Option<PhysicsFile>,
    material: Option<crate::Material>,
    material_sections: Option<crate::MaterialSections>,
    terrain_field: Option<crate::TerrainField>,
    particles: Option<crate::ParticleEffect>,
    controller: Option<crate::CharacterController>,
    spin: Option<crate::Spin>,
    animator: Option<crate::Animator>,
    /// Instance ID the animator's head looks at; resolved at construction.
    animator_look: Option<String>,
    /// Target instance ID, speed and stop distance; resolved to a handle at construction.
    follow: Option<(String, f32, f32)>,
    group: Option<String>,
    /// The modes (legacy: the main brain then any stacked brains), with the component's settings.
    brain: Option<BrainSource>,
    locomotion: Option<crate::locomotion::Model>,
    /// Steering wheel IDs, rolling wheel IDs, radius; resolve to handles at construction.
    wheels: Option<(Vec<String>, Vec<String>, f32)>,
    /// Target ID, aim offset, turn rate (rad/s).
    look_at: Option<(String, Vec3, f32)>,
    /// Target ID, offset, behind, smoothing, look.
    follow_camera: Option<(String, Vec3, bool, f32, bool)>,
    project_components: Vec<(String, toml::Table)>,
    brain_bindings: HashMap<String, String>,
    audio_emitter: Option<crate::AudioEmitter>,
    audio_listener: Option<crate::AudioListener>,
    /// Link target IDs, two-way, arrival radius; targets resolve to handles at construction.
    waypoint: Option<(Vec<String>, crate::Waypoint)>,
    zone: Option<crate::Zone>,
    metadata: Option<crate::Metadata>,
    ui_surface: Option<crate::UiSurface>,
    mouse_look: Option<crate::MouseLook>,
}

impl ValidatedEntity {
    fn brain_target<'a>(&'a self, local: &'a str) -> Option<&'a str> {
        if self.brain_bindings.is_empty() {
            Some(local)
        } else {
            self.brain_bindings.get(local).map(String::as_str)
        }
    }
}

fn fit_child_physics(entities: &mut [ValidatedEntity], path: &str) -> Result<(), SceneError> {
    if !entities
        .iter()
        .any(|entity| entity.pending_physics.is_some())
    {
        return Ok(());
    }
    let mut children: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, entity) in entities.iter().enumerate() {
        if let Some(parent) = &entity.parent {
            children.entry(parent.clone()).or_default().push(index);
        }
    }
    let bounds = |mesh: &crate::Mesh| {
        mesh.vertices().iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), vertex| (min.min(vertex.position()), max.max(vertex.position())),
        )
    };
    let local_bounds: Vec<Option<(Vec3, Vec3)>> = entities
        .iter()
        .map(|entity| {
            Ok(match entity.geometry.as_ref() {
                Some(Geometry::Mesh(mesh)) => Some(bounds(mesh)),
                Some(Geometry::Primitive(primitive)) => {
                    Some(bounds(&primitive.to_mesh().map_err(|e| error(path, e))?))
                }
                _ => None,
            })
        })
        .collect::<Result<_, SceneError>>()?;
    let matrix_for = |transform: Transform| {
        crate::Mat4::from_scale_rotation_translation(
            transform.scale,
            transform.rotation,
            transform.translation,
        )
    };
    for root in 0..entities.len() {
        let Some(mut physics) = entities[root].pending_physics.take() else {
            continue;
        };
        let root_id = entities[root].id.clone();
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut stack: Vec<_> = children
            .get(&root_id)
            .into_iter()
            .flatten()
            .map(|&child| (child, matrix_for(entities[child].transform)))
            .collect();
        while let Some((child, relative)) = stack.pop() {
            if let Some((lower, upper)) = local_bounds[child] {
                for x in [lower.x, upper.x] {
                    for y in [lower.y, upper.y] {
                        for z in [lower.z, upper.z] {
                            let point = relative.transform_point3(Vec3::new(x, y, z));
                            min = min.min(point);
                            max = max.max(point);
                        }
                    }
                }
            }
            if let Some(next) = children.get(&entities[child].id) {
                stack.extend(
                    next.iter()
                        .map(|&index| (index, relative * matrix_for(entities[index].transform))),
                );
            }
        }
        if !min.is_finite() || !max.is_finite() {
            return Err(error(path, format!("instance '{root_id}': automatic box fitting requires geometry on this entity or its descendants; supply explicit half_extents")));
        }
        physics
            .collider
            .center
            .get_or_insert(((min + max) * 0.5).to_array());
        physics.collider.half_extents = Some(((max - min) * 0.5).max(Vec3::splat(0.01)).to_array());
        entities[root].physics = physics
            .resolve(None, None)
            .map_err(|e| error(path, format!("instance '{root_id}': {e}")))?;
    }
    Ok(())
}

/// An enabled project-defined component, handed to the game to attach its runtime.
#[derive(Clone, Debug)]
pub struct SceneComponent {
    pub entity: EntityId,
    pub type_id: String,
    pub fields: toml::Table,
}

/// Fully validated candidate; loading never mutates the active world.
#[derive(Clone)]
pub struct SceneDefinition {
    path: String,
    name: String,
    builder: Option<String>,
    active_camera: Option<String>,
    entities: Vec<ValidatedEntity>,
    environment: crate::SceneEnvironment,
    wind: Option<crate::WindDefinition>,
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
    /// Enabled project components, in scene order.
    pub components: Vec<SceneComponent>,
}

/// A brain component's modes with their loaded brains, and its settings.
type BrainSource = (
    Vec<(std::sync::Arc<crate::BrainDefinition>, ModeSource)>,
    crate::BrainSettings,
);

/// One brain mode as authored, before its brain is compiled for the scene.
#[derive(Clone, Debug)]
struct ModeSource {
    name: String,
    active: bool,
    on_next: Option<usize>,
    on_previous: Option<usize>,
    keep_running: bool,
}

impl ModeSource {
    fn new(name: String, active: bool) -> Self {
        Self {
            name,
            active,
            on_next: None,
            on_previous: None,
            keep_running: false,
        }
    }
}

pub(crate) fn identifier(id: &str) -> bool {
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
    #[cfg(feature = "graphics")]
    pub(crate) fn inspection_classification(
        &self,
        id: &str,
    ) -> Result<crate::ObjectClassification, Box<dyn std::error::Error>> {
        Ok(self
            .entities
            .iter()
            .find(|e| e.id == id)
            .ok_or("Object is not in saved scene")?
            .classification)
    }

    #[cfg(feature = "graphics")]
    pub(crate) fn inspection_material(
        &self,
        id: &str,
    ) -> Result<Option<crate::Material>, Box<dyn std::error::Error>> {
        Ok(self
            .entities
            .iter()
            .find(|e| e.id == id)
            .ok_or("Material instance is not in the saved scene")?
            .material
            .clone())
    }

    /// Apply already validated authored physics without reconstructing the scene or its identities.
    #[cfg(feature = "graphics")]
    pub(crate) fn apply_inspection_physics(
        &self,
        world: &mut World,
        loaded: &LoadedScene,
        id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let definition = self
            .entities
            .iter()
            .find(|entity| entity.id == id)
            .ok_or("Physics instance is not in the saved scene")?;
        let entity = *loaded
            .entities
            .get(id)
            .ok_or("Physics instance is not in the inspection scene")?;
        if let Some(body) = definition.physics {
            world.insert_component(entity, body)?;
        } else {
            world.remove_component::<crate::PhysicsBody>(entity)?;
        }
        Ok(())
    }

    pub fn load(project: &Project, path: &str) -> Result<Self, SceneError> {
        Self::load_with(project, path, &crate::ComponentRegistry::builtin())
    }

    /// Loads with a registry that includes the project's own component types.
    pub fn load_with(
        project: &Project,
        path: &str,
        registry: &crate::ComponentRegistry,
    ) -> Result<Self, SceneError> {
        let scene: SceneFile = read_bounded(project, path, SCENE_MAX_BYTES)?;
        Self::validate(project, path, scene, registry)
    }

    /// Validates an editor candidate against the real project's asset definitions.
    /// No scene or runtime world is modified.
    pub fn from_source(project: &Project, path: &str, source: &str) -> Result<Self, SceneError> {
        Self::from_source_with(project, path, source, &crate::ComponentRegistry::builtin())
    }

    pub fn from_source_with(
        project: &Project,
        path: &str,
        source: &str,
        registry: &crate::ComponentRegistry,
    ) -> Result<Self, SceneError> {
        project.asset_path(path).map_err(|e| error(path, e))?;
        if source.len() as u64 > SCENE_MAX_BYTES {
            return Err(error(path, "scene file exceeds 2 MiB"));
        }
        let scene = toml::from_str(source).map_err(|e| error(path, e))?;
        Self::validate(project, path, scene, registry)
    }

    fn validate(
        project: &Project,
        path: &str,
        scene: SceneFile,
        registry: &crate::ComponentRegistry,
    ) -> Result<Self, SceneError> {
        let labels = crate::ObjectLabels::load(project).map_err(|e| error(path, e))?;
        if scene.schema_version != 1 || !identifier(&scene.id) {
            return Err(error(
                path,
                "requires schema_version = 1 and a valid scene id",
            ));
        }
        if scene.entities.len() > SCENE_MAX_INSTANCES {
            return Err(error(path, "scene exceeds 4096 instances"));
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
        if let Some(wind) = &scene.wind {
            wind.validate().map_err(|e| error(path, e))?;
        }
        let wind = scene.wind;
        let mut entities = Vec::with_capacity(scene.entities.len());
        let mut ids = HashSet::new();
        // Brains shared by several instances load once per scene validation.
        let mut brains: HashMap<String, std::sync::Arc<crate::BrainDefinition>> = HashMap::new();
        // Clip assets are parsed here; samples decode later, only when audio is playing.
        let mut clips: HashMap<String, std::sync::Arc<crate::AudioClipAsset>> = HashMap::new();
        for instance in scene.entities {
            if instance.prefab_label.as_ref().is_some_and(|name| {
                name.trim().is_empty()
                    || name.chars().count() > 64
                    || name.chars().any(char::is_control)
            }) {
                return Err(error(path, "Invalid prefab instance label"));
            }
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
            // Several instances share an asset, so errors name the instance too.
            let object = match instance.name.as_ref().or(instance.prefab_label.as_ref()) {
                Some(name) => format!("object '{name}' ({})", instance.id),
                None => format!("object {}", instance.id),
            };
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
                let mut material_sections = None;
                let geometry = match &definition.structure {
                    Some(path) => {
                        let (mesh, sections) =
                            crate::StructureDefinition::load(project, path)?.assemble()?;
                        if !sections.is_empty() {
                            let mut resolved = Vec::with_capacity(sections.len());
                            for section in sections {
                                let material = crate::MaterialDefinition {
                                    asset: Some(section.material),
                                    ..Default::default()
                                }
                                .resolve(project)?;
                                if material.terrain.is_some() {
                                    return Err(
                                        "Part materials cannot be terrain materials.".into()
                                    );
                                }
                                resolved.push(crate::MaterialSection {
                                    indices: section.indices,
                                    material,
                                });
                            }
                            material_sections = Some(crate::MaterialSections(resolved));
                        }
                        Some(Geometry::Mesh(mesh))
                    }
                    None => geometry,
                };
                if definition.model.is_some() && (geometry.is_some() || definition.kind != "model")
                {
                    return Err("Model assets require kind=model without terrain, structure or primitive geometry.".into());
                }
                let model = definition
                    .model
                    .as_deref()
                    .map(|path| crate::ImportedModel::load(project, path))
                    .transpose()?;
                let (geometry, model_texture, model_rig) = match model {
                    Some(model) => (
                        Some(Geometry::Mesh(model.mesh)),
                        model.texture,
                        model.skeleton.map(|skeleton| {
                            (
                                std::sync::Arc::new(skeleton),
                                std::sync::Arc::from(model.clips),
                            )
                        }),
                    ),
                    None => (geometry, None, None),
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
                let mut light_file = definition.light;
                let mut light_enabled = true;
                let (mut controller, mut spin, mut follow) = (None, None, None);
                let (mut group, mut brain, mut locomotion) = (None, None, None);
                let mut wheels = None;
                let (mut look_at, mut follow_camera) = (None, None);
                let (mut audio_emitter, mut audio_listener) = (None, None);
                let (mut waypoint, mut zone, mut metadata) = (None, None, None);
                let mut ui_surface = None;
                let mut mouse_look = None;
                let mut animator = None;
                let mut animator_look = None;
                let mut project_components = Vec::new();
                let mut seen = HashSet::new();
                for component in &instance.components {
                    let kind = registry.get(&component.kind).ok_or_else(|| {
                        format!(
                            "unknown component type '{}'; known types: {}",
                            component.kind,
                            registry
                                .types()
                                .iter()
                                .map(|t| t.id)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })?;
                    if !seen.insert(kind.id) {
                        return Err(
                            format!("component '{}' appears more than once", kind.id).into()
                        );
                    }
                    kind.validate(&component.fields)?;
                    let values = kind.values(&component.fields);
                    let number = |name| values.number(name).unwrap_or_default();
                    // Built-ins are applied by the subsystem that owns them; project types go to the game.
                    match kind.id {
                        crate::component::LIGHT => {
                            light_file
                                .as_mut()
                                .ok_or("emergent.light requires a light asset")?
                                .apply(&component.fields);
                            light_enabled = component.enabled;
                        }
                        _ if !component.enabled => {}
                        crate::component::CHARACTER_CONTROLLER => {
                            controller = Some(crate::CharacterController {
                                speed: number("speed"),
                                run_speed: number("run_speed"),
                                jump_speed: number("jump_speed"),
                                camera_relative: values.bool("camera_relative").unwrap_or(true),
                                force: number("force"),
                            });
                        }
                        crate::component::SPIN => {
                            let axis =
                                Vec3::from_array(values.vector3("axis").unwrap_or([0., 1., 0.]))
                                    .try_normalize()
                                    .ok_or("emergent.spin axis must not be zero")?;
                            spin = Some(crate::Spin {
                                axis,
                                degrees_per_second: number("degrees_per_second"),
                            });
                        }
                        crate::component::FOLLOW => {
                            let target = values.entity("target").unwrap_or_default().to_string();
                            if target == instance.id {
                                return Err("emergent.follow cannot target its own object".into());
                            }
                            follow = Some((target, number("speed"), number("stop_distance")));
                        }
                        crate::component::LOOK_AT | crate::component::FOLLOW_CAMERA => {
                            let target = values.entity("target").unwrap_or_default().to_string();
                            if target == instance.id {
                                return Err(
                                    format!("{} cannot target its own object", kind.id).into()
                                );
                            }
                            let offset =
                                Vec3::from_array(values.vector3("offset").unwrap_or_default());
                            if kind.id == crate::component::LOOK_AT {
                                look_at = Some((target, offset, number("turn_rate").to_radians()));
                            } else {
                                follow_camera = Some((
                                    target,
                                    offset,
                                    values.entity("mode") != Some("fixed"),
                                    number("smoothing"),
                                    values.bool("look_at").unwrap_or(true),
                                ));
                            }
                        }
                        crate::component::LOCOMOTION => {
                            let path = values.entity("profile").unwrap_or_default();
                            locomotion = Some(
                                crate::LocomotionProfile::load_with(project, path, registry)?.model,
                            );
                            let list = |name| -> Vec<String> {
                                values
                                    .entities(name)
                                    .into_iter()
                                    .map(str::to_string)
                                    .collect()
                            };
                            let (steering, rolling) =
                                (list("steering_wheels"), list("rolling_wheels"));
                            if !steering.is_empty() || !rolling.is_empty() {
                                wheels = Some((steering, rolling, number("wheel_radius")));
                            }
                        }
                        crate::component::GROUP => {
                            group = values.entity("name").map(str::to_string);
                        }
                        crate::component::ANIMATOR => {
                            let (skeleton, clips): &(
                                std::sync::Arc<crate::Skeleton>,
                                std::sync::Arc<[crate::AnimationClip]>,
                            ) = model_rig
                                .as_ref()
                                .ok_or("emergent.animator requires a skinned model")?;
                            let motion = values.entity("motion").unwrap_or_default();
                            let mut all = clips.to_vec();
                            // Comma-separated: one mocap take per file is the norm.
                            for motion in motion.split(',').map(str::trim).filter(|m| !m.is_empty())
                            {
                                let (source, source_clips) = if motion.ends_with(".bvh") {
                                    let take = crate::Motion::load(project, motion)?;
                                    (take.skeleton, vec![take.clip])
                                } else {
                                    let model = crate::ImportedModel::load(project, motion)?;
                                    let skeleton = model
                                        .skeleton
                                        .ok_or("emergent.animator motion model has no skeleton")?;
                                    (skeleton, model.clips)
                                };
                                let (source_map, target_map) = (
                                    crate::HumanoidMap::detect(&source),
                                    crate::HumanoidMap::detect(skeleton),
                                );
                                for clip in &source_clips {
                                    all.push(crate::retarget(
                                        &source,
                                        &source_map,
                                        clip,
                                        skeleton,
                                        &target_map,
                                    )?);
                                }
                            }
                            let blend = values.entity("blend").unwrap_or_default();
                            let speeds: Vec<f32> =
                                if values.bool("in_place").unwrap_or(false) || !blend.is_empty() {
                                    all.iter_mut().map(|c| c.make_in_place(skeleton)).collect()
                                } else {
                                    Vec::new()
                                };
                            let mut entries = Vec::new();
                            for entry in blend.split(',').map(str::trim).filter(|e| !e.is_empty()) {
                                let (name, speed) = match entry.split_once(':') {
                                    Some((name, speed)) => (
                                        name.trim(),
                                        speed.trim().parse::<f32>().map_err(|_| {
                                            format!("Blend speed {speed:?} is not a number")
                                        })?,
                                    ),
                                    None => (
                                        entry,
                                        all.iter()
                                            .position(|c| c.name() == entry)
                                            .map_or(0.0, |i| speeds[i]),
                                    ),
                                };
                                entries.push((name, speed));
                            }
                            let clips: std::sync::Arc<[crate::AnimationClip]> = all.into();
                            let name = values.entity("clip").unwrap_or_default();
                            let clip = if name.is_empty() {
                                clips.first().map(|c| c.name())
                            } else {
                                Some(name)
                            };
                            let mut playing =
                                crate::Animator::new(skeleton.clone(), clips.clone(), clip)?;
                            playing.speed = number("speed");
                            playing.looping = values.bool("loop").unwrap_or(true);
                            playing.playing = values.bool("playing").unwrap_or(true);
                            playing.set_blend(&entries)?;
                            playing.turn_rate = number("turn_rate").to_radians();
                            playing.foot_ik = values.bool("foot_ik").unwrap_or(true);
                            let look = values.entity("look_at").unwrap_or_default();
                            if look == instance.id {
                                return Err(
                                    "emergent.animator cannot look at its own object".into()
                                );
                            }
                            if !look.is_empty() {
                                animator_look = Some(look.to_string());
                            }
                            animator = Some(playing);
                        }
                        "emergent.mouse_look" => {
                            if definition.kind != "camera" {
                                return Err("Mouse look requires a camera asset".into());
                            }
                            mouse_look = Some(crate::MouseLook::new(
                                number("sensitivity"),
                                number("pitch_limit"),
                                transform.rotation,
                            )?);
                        }
                        "emergent.ui" => {
                            let path = values.entity("ui").unwrap_or_default();
                            if !path.ends_with(".ui.toml") {
                                return Err("UI asset must end in .ui.toml".into());
                            }
                            let surface = crate::UiSurface {
                                definition: std::sync::Arc::new(crate::ui::UiDefinition::load(
                                    project, path,
                                )?),
                                screen: values.entity("space") != Some("world"),
                                width: number("width"),
                                height: number("height"),
                                x: number("x"),
                                y: number("y"),
                            };
                            surface.validate()?;
                            ui_surface = Some(surface);
                        }
                        crate::component::AUDIO_SOURCE => {
                            let path = values.entity("clip").unwrap_or_default();
                            let clip = match clips.get(path) {
                                Some(clip) => clip.clone(),
                                None => {
                                    let loaded = std::sync::Arc::new(crate::AudioClipAsset::load(
                                        project, path,
                                    )?);
                                    clips.insert(path.to_string(), loaded.clone());
                                    loaded
                                }
                            };
                            let settings = crate::SoundSettings {
                                looping: values.entity("mode") != Some("one_shot"),
                                volume: number("volume"),
                                pitch: number("pitch"),
                                spatial: number("spatial"),
                                min_distance: number("min_distance"),
                                max_distance: number("max_distance"),
                                rolloff: if values.entity("rolloff") == Some("linear") {
                                    crate::Rolloff::Linear
                                } else {
                                    crate::Rolloff::Inverse
                                },
                                doppler: number("doppler"),
                                reverb: number("reverb"),
                                bus: clip.bus,
                            };
                            settings
                                .validate()
                                .map_err(|e| format!("emergent.audio_source: {e}"))?;
                            let autoplay = values.bool("autoplay").unwrap_or(true);
                            audio_emitter = Some(
                                crate::AudioEmitter::new(clip, settings, autoplay).with_play_on(
                                    values.entity("play_on").unwrap_or_default().trim(),
                                ),
                            );
                        }
                        crate::component::WAYPOINT => {
                            let links: Vec<String> = values
                                .entities("links")
                                .into_iter()
                                .map(str::to_string)
                                .collect();
                            if links.iter().any(|link| link == &instance.id) {
                                return Err("emergent.waypoint cannot link to itself".into());
                            }
                            let range =
                                |from: &str, to: &str| -> Result<Option<(f32, f32)>, String> {
                                    match (values.number(from), values.number(to)) {
                                    (Some(a), Some(b)) => Ok(Some((a.to_radians(), b.to_radians()))),
                                    (None, None) => Ok(None),
                                    _ => Err(format!("emergent.waypoint needs both {from} and {to}, or neither")),
                                }
                                };
                            waypoint = Some((
                                links,
                                crate::Waypoint {
                                    links: Vec::new(),
                                    two_way: values.bool("two_way").unwrap_or(true),
                                    radius: number("radius"),
                                    entry: range("entry_from", "entry_to")?,
                                    exit: range("exit_from", "exit_to")?,
                                    altitude: values.number("altitude"),
                                },
                            ));
                        }
                        crate::component::ZONE => {
                            let shape = if values.entity("shape") == Some("rect") {
                                crate::ZoneShape::Rect {
                                    width: number("width"),
                                    depth: number("depth"),
                                }
                            } else {
                                crate::ZoneShape::Circle {
                                    radius: number("radius"),
                                }
                            };
                            let layers = values
                                .entity("layers")
                                .unwrap_or_default()
                                .split(',')
                                .map(str::trim)
                                .filter(|layer| !layer.is_empty())
                                .map(str::to_string)
                                .collect();
                            zone = Some(crate::Zone {
                                shape,
                                height: number("height"),
                                cost: if values.bool("blocked").unwrap_or(false) {
                                    f32::INFINITY
                                } else {
                                    number("cost")
                                },
                                layers,
                            });
                        }
                        crate::component::METADATA => {
                            let pairs = values
                                .table("values")
                                .map(|table| {
                                    table
                                        .iter()
                                        .filter_map(|(key, value)| {
                                            let value = match value {
                                                toml::Value::String(text) => {
                                                    crate::MetaValue::Text(text.clone())
                                                }
                                                toml::Value::Integer(n) => {
                                                    crate::MetaValue::Number(*n as f64)
                                                }
                                                toml::Value::Float(n) => {
                                                    crate::MetaValue::Number(*n)
                                                }
                                                toml::Value::Boolean(b) => {
                                                    crate::MetaValue::Bool(*b)
                                                }
                                                _ => return None,
                                            };
                                            Some((key.clone(), value))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            metadata = Some(crate::Metadata(pairs));
                        }
                        crate::component::AUDIO_LISTENER => {
                            audio_listener = Some(crate::AudioListener {
                                gain: number("gain"),
                                reverb: crate::ReverbSettings {
                                    room: number("reverb_room"),
                                    damping: number("reverb_damping"),
                                },
                                ducking_db: number("ducking"),
                            });
                        }
                        crate::component::BRAIN => {
                            let modes = values.array("modes");
                            let legacy = values.entity("brain").is_some()
                                || !values.entities("stack").is_empty();
                            if !modes.is_empty() && legacy {
                                return Err(
                                    "emergent.brain uses either brain and stack, or modes, not both"
                                        .into(),
                                );
                            }
                            // Legacy brain + stack load as always-on modes without transitions.
                            let mut sources: Vec<(&str, ModeSource)> = Vec::new();
                            if modes.is_empty() {
                                for (i, path) in
                                    std::iter::once(values.entity("brain").unwrap_or_default())
                                        .chain(values.entities("stack"))
                                        .enumerate()
                                {
                                    sources.push((
                                        path,
                                        ModeSource::new(
                                            if i == 0 {
                                                "main".into()
                                            } else {
                                                format!("stack{i}")
                                            },
                                            true,
                                        ),
                                    ));
                                }
                            } else {
                                // Validated by the component schema: names, paths and links.
                                let position = |name: Option<&str>| {
                                    name.and_then(|name| {
                                        modes.iter().position(|m| m["name"].as_str() == Some(name))
                                    })
                                };
                                for mode in modes {
                                    let text = |key| mode.get(key).and_then(toml::Value::as_str);
                                    let flag = |key| mode.get(key).and_then(toml::Value::as_bool);
                                    let mut source = ModeSource::new(
                                        text("name").unwrap_or_default().to_string(),
                                        flag("active").unwrap_or(false),
                                    );
                                    source.on_next = position(text("on_next"));
                                    source.on_previous = position(text("on_previous"));
                                    source.keep_running = flag("keep_running").unwrap_or(false);
                                    sources.push((text("brain").unwrap_or_default(), source));
                                }
                            }
                            let mut stack = Vec::with_capacity(sources.len());
                            for (path, source) in sources {
                                let definition = match brains.get(path) {
                                    Some(definition) => definition.clone(),
                                    None => {
                                        let loaded =
                                            std::sync::Arc::new(crate::BrainDefinition::load_with(
                                                project, path, registry,
                                            )?);
                                        brains.insert(path.to_string(), loaded.clone());
                                        loaded
                                    }
                                };
                                stack.push((definition, source));
                            }
                            brain = Some((
                                stack,
                                crate::BrainSettings {
                                    max_speed: number("max_speed"),
                                    acceleration: number("acceleration"),
                                    planar: values.bool("planar").unwrap_or(false),
                                    face_movement: values.bool("face_movement").unwrap_or(true),
                                    turn_rate: number("turn_rate").to_radians(),
                                },
                            ));
                        }
                        _ => {
                            project_components.push((kind.id.to_string(), component.fields.clone()))
                        }
                    }
                }
                let light = light_file.map(LightFile::source).transpose()?;
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
                    "model"
                        if definition.model.is_some()
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
                let light = light.filter(|_| light_enabled);
                let authored_physics = instance.physics.or(definition.physics);
                let pending_physics = authored_physics
                    .as_ref()
                    .filter(|p| {
                        p.enabled
                            && geometry.is_none()
                            && p.collider.half_extents.is_none()
                            && p.collider
                                .shape
                                .as_deref()
                                .is_none_or(|shape| shape == "box")
                    })
                    .cloned();
                // Validate all non-geometry settings now; fit the real child bounds after hierarchy validation.
                let physics = authored_physics
                    .map(|mut p| {
                        if pending_physics.is_some() {
                            p.collider.half_extents = Some([0.01; 3]);
                        }
                        p.resolve(geometry.as_ref(), terrain)
                    })
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
                // An authored material replaces a model's own texture; otherwise it renders as imported.
                let material = match (material, model_texture) {
                    (None, Some(texture)) => Some(crate::Material::with_uv_image(
                        crate::MaterialDefinition {
                            shading: crate::MaterialShading::Smooth,
                            ..Default::default()
                        },
                        texture,
                    )?),
                    // Heightfields render smooth like Terrain Builder previews. The flat default
                    // would give every triangle its own vertices: 300MB at 1024, beyond GPU limits.
                    (None, None) if terrain_field.is_some() => Some(
                        crate::MaterialDefinition {
                            shading: crate::MaterialShading::Smooth,
                            ..Default::default()
                        }
                        .resolve(project)?,
                    ),
                    (material, _) => material,
                };
                if locomotion.is_some()
                    && (controller.is_some() || follow.is_some() || spin.is_some())
                {
                    return Err("locomotion moves and turns the object itself; it works with a brain but not with a character controller, follow or spin".into());
                }
                if let Some(model) = locomotion {
                    model.validate_physics(physics)?;
                    if model.is_physical() {
                        if transform.scale.min_element() <= 0.0
                            || transform.scale.x != transform.scale.y
                            || transform.scale.x != transform.scale.z
                        {
                            return Err(
                                "drone_physics requires positive uniform local scale".into()
                            );
                        }
                        if brain
                            .as_ref()
                            .is_some_and(|(stack, _)| stack.iter().any(|(b, _)| b.looks()))
                        {
                            return Err("drone_physics cannot execute brain look goals until the powered pilot is implemented".into());
                        }
                    }
                }
                if [
                    controller.is_some(),
                    follow.is_some(),
                    brain.is_some(),
                    follow_camera.is_some(),
                ]
                .iter()
                .filter(|b| **b)
                .count()
                    > 1
                {
                    return Err("an object can have only one of character controller, follow, brain and follow camera; each moves it".into());
                }
                if follow_camera.is_some() && locomotion.is_some() {
                    return Err("follow camera and locomotion both move the object; use one".into());
                }
                if brain
                    .as_ref()
                    .is_some_and(|(stack, _)| stack.iter().any(|(b, _)| b.looks()))
                    && (spin.is_some() || look_at.is_some())
                {
                    return Err("this brain looks (Look at, Look along or Scan), which turns the object; remove spin or the Look at component".into());
                }
                if look_at.is_some() && (spin.is_some() || locomotion.is_some()) {
                    return Err(
                        "look at turns the object, as do spin and locomotion; use one".into(),
                    );
                }
                if (look_at.is_some() || follow_camera.is_some())
                    && physics.is_some_and(|p| p.body == crate::BodyKind::Dynamic)
                {
                    return Err("look at and follow camera set the transform, which a dynamic physics body owns; make the body static or remove physics".into());
                }
                if (spin.is_some() || follow.is_some())
                    && physics.is_some_and(|p| p.body == crate::BodyKind::Dynamic)
                {
                    return Err("spin and follow move the transform, which a dynamic physics body owns; make the body static or remove physics".into());
                }
                if mouse_look.is_some()
                    && (spin.is_some()
                        || look_at.is_some()
                        || follow_camera.is_some()
                        || brain.is_some()
                        || locomotion.is_some()
                        || controller.is_some()
                        || physics.is_some_and(|p| p.body == crate::BodyKind::Dynamic))
                {
                    return Err("Mouse look owns camera rotation; remove other camera movers and dynamic camera physics".into());
                }
                let classification = instance
                    .classification
                    .or(definition.classification)
                    .unwrap_or_default();
                labels.validate_object(classification)?;
                Ok(ValidatedEntity {
                    classification,
                    look_at,
                    follow_camera,
                    locomotion,
                    wheels,
                    group,
                    brain,
                    controller,
                    spin,
                    animator,
                    animator_look,
                    follow,
                    project_components,
                    raymarched_terrain,
                    ui_surface,
                    mouse_look,
                    audio_emitter,
                    audio_listener,
                    waypoint,
                    zone,
                    metadata,
                    brain_bindings: instance.brain_bindings,
                    id: instance.id,
                    parent: instance.parent,
                    name: instance.name.or(definition.name).unwrap_or(definition.id),
                    transform,
                    geometry,
                    light,
                    camera,
                    physics,
                    pending_physics,
                    material,
                    material_sections,
                    particles,
                    terrain_field: if sculpted { terrain_field } else { None },
                })
            })();
            entities.push(result.map_err(|e| error(asset, format!("{object}: {e}")))?);
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
        let ui_pixels: u64 = entities
            .iter()
            .filter_map(|e| e.ui_surface.as_ref())
            .map(|u| u64::from(u.definition.canvas[0]) * u64::from(u.definition.canvas[1]))
            .sum();
        if ui_pixels > 16_777_216 {
            return Err(error(path, "Scene UI surfaces exceed 64 MiB RGBA budget"));
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
        fit_child_physics(&mut entities, path)?;
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
            for (type_id, fields) in &entity.project_components {
                let kind = registry.get(type_id).expect("validated component type");
                let values = kind.values(fields);
                for field in &kind.fields {
                    let targets = match field.kind {
                        crate::FieldKind::Entity => values.entity(field.name).into_iter().collect(),
                        crate::FieldKind::Entities => values.entities(field.name),
                        _ => continue,
                    };
                    for target in targets {
                        if !by_id.contains_key(target) {
                            return Err(error(path, format!("instance {}: {type_id}.{} target '{target}' is not in this scene", entity.id, field.name)));
                        }
                    }
                }
            }
            for (local, target) in &entity.brain_bindings {
                if !identifier(local) || !by_id.contains_key(target.as_str()) {
                    return Err(error(
                        path,
                        format!(
                            "instance {}: invalid brain binding {local} -> {target}",
                            entity.id
                        ),
                    ));
                }
            }
            for definition in entity
                .brain
                .iter()
                .flat_map(|(stack, _)| stack)
                .map(|(definition, _)| definition)
            {
                if let Some(missing) = definition.object_references().into_iter().find(|id| {
                    entity
                        .brain_target(id)
                        .is_none_or(|target| !by_id.contains_key(target))
                }) {
                    return Err(error(
                        path,
                        format!("instance {}: brain {} names '{missing}', which is not in this scene; use a group to target objects across scenes", entity.id, definition.path),
                    ));
                }
            }
            for (name, target) in [
                ("look at", entity.look_at.as_ref().map(|l| &l.0)),
                ("follow camera", entity.follow_camera.as_ref().map(|c| &c.0)),
            ] {
                if let Some(target) = target.filter(|t| !by_id.contains_key(t.as_str())) {
                    return Err(error(
                        path,
                        format!(
                            "instance {}: {name} target '{target}' is not in this scene",
                            entity.id
                        ),
                    ));
                }
            }
            if let Some((steering, rolling, _)) = &entity.wheels {
                let mut seen = std::collections::HashSet::new();
                for wheel in steering.iter().chain(rolling) {
                    let problem = if !seen.insert(wheel.as_str()) {
                        Some("is listed more than once")
                    } else if let Some(target) = by_id.get(wheel.as_str()) {
                        // Parents were checked for cycles above, so this walk ends.
                        let mut ancestor = target.parent.as_deref();
                        while ancestor.is_some_and(|id| id != entity.id) {
                            ancestor = by_id
                                .get(ancestor.unwrap())
                                .and_then(|e| e.parent.as_deref());
                        }
                        if ancestor.is_none() {
                            Some("must be a child (or deeper descendant) of the car")
                        } else if target.locomotion.is_some()
                            || target.spin.is_some()
                            || target.look_at.is_some()
                            || target.follow.is_some()
                            || target.follow_camera.is_some()
                            || target.controller.is_some()
                        {
                            Some("has its own mover; the wheel rig turns it")
                        } else {
                            None
                        }
                    } else {
                        Some("is not in this scene")
                    };
                    if let Some(problem) = problem {
                        return Err(error(
                            path,
                            format!("instance {}: wheel '{wheel}' {problem}", entity.id),
                        ));
                    }
                }
            }
            if let Some((links, ..)) = &entity.waypoint {
                for link in links {
                    if !by_id
                        .get(link.as_str())
                        .is_some_and(|target| target.waypoint.is_some())
                    {
                        return Err(error(
                            path,
                            format!(
                                "instance {}: waypoint link '{link}' is not a waypoint in this scene",
                                entity.id
                            ),
                        ));
                    }
                }
            }
            if let Some(target) = &entity.animator_look {
                if !by_id.contains_key(target.as_str()) {
                    return Err(error(
                        path,
                        format!(
                            "instance {}: animator look-at target '{target}' is not in this scene",
                            entity.id
                        ),
                    ));
                }
            }
            if let Some((target, ..)) = &entity.follow {
                if !by_id.contains_key(target.as_str()) {
                    return Err(error(
                        path,
                        format!(
                            "instance {}: follow target '{target}' is not in this scene",
                            entity.id
                        ),
                    ));
                }
            }
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
            wind,
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
            let mut components = Vec::new();
            let mut groups = world
                .resource::<crate::GroupNames>()
                .cloned()
                .unwrap_or_default();
            for entity in &self.entities {
                let id = world.spawn_child(root, entity.name.clone())?;
                world.insert_component(id, entity.classification)?;
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
                if let Some(sections) = &entity.material_sections {
                    world.insert_component(id, sections.clone())?;
                }
                if let Some(physics) = entity.physics {
                    world.insert_component(id, physics)?;
                }
                if let Some(light) = entity.light {
                    world.insert_component(id, light)?;
                    lights.push(id);
                }
                if let Some(look) = entity.mouse_look {
                    world.insert_component(id, look)?;
                }
                if let Some(camera) = entity.camera {
                    world.insert_component(id, camera)?;
                    cameras.push(id);
                }
                if let Some(controller) = entity.controller {
                    world.insert_component(id, controller)?;
                }
                if let Some(spin) = entity.spin {
                    world.insert_component(id, spin)?;
                }
                if let Some(animator) = &entity.animator {
                    world.insert_component(id, animator.clone())?;
                }
                if let Some(surface) = &entity.ui_surface {
                    world.insert_component(id, surface.clone())?;
                }
                if let Some(emitter) = &entity.audio_emitter {
                    world.insert_component(id, emitter.clone())?;
                }
                if let Some(listener) = entity.audio_listener {
                    world.insert_component(id, listener)?;
                }
                if let Some(model) = entity.locomotion {
                    world.insert_component(id, crate::LocomotionAgent::new(model))?;
                }
                entities.insert(entity.id.clone(), id);
            }
            // Share compiled brains only when both the asset and object bindings match.
            let mut compiled: HashMap<_, std::sync::Arc<crate::CompiledBrain>> = HashMap::new();
            for entity in &self.entities {
                let Some((stack, settings)) = &entity.brain else {
                    continue;
                };
                let mut bindings: Vec<_> = entity
                    .brain_bindings
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                bindings.sort();
                let modes = stack
                    .iter()
                    .map(|(definition, mode)| crate::BrainMode {
                        name: mode.name.clone(),
                        active: mode.active,
                        on_next: mode.on_next,
                        on_previous: mode.on_previous,
                        keep_running: mode.keep_running,
                        brain: compiled
                            .entry((definition.path.clone(), bindings.clone()))
                            .or_insert_with(|| {
                                std::sync::Arc::new(crate::CompiledBrain::compile(
                                    definition,
                                    &|id| {
                                        entity
                                            .brain_target(id)
                                            .and_then(|target| entities.get(target))
                                            .copied()
                                    },
                                    &mut groups,
                                ))
                            })
                            .clone(),
                    })
                    .collect();
                let agent = crate::BrainAgent::with_modes(
                    modes,
                    *settings,
                    crate::brain::seed_from(&entity.id),
                    Some(&mut groups),
                )
                // Names, links and the mode count were validated when the scene loaded.
                .expect("brain modes validated at load");
                world.insert_component(entities[&entity.id], agent)?;
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
                if let Some(name) = &entity.group {
                    let group = groups.id(name);
                    world.insert_component(id, crate::Group(group))?;
                }
                if let Some((target, offset, turn_rate)) = entity.look_at {
                    let target = entities[&target];
                    world.insert_component(
                        id,
                        crate::LookAt {
                            target,
                            offset,
                            turn_rate,
                        },
                    )?;
                }
                if let Some((target, offset, behind, smoothing, look)) = entity.follow_camera {
                    let target = entities[&target];
                    world.insert_component(
                        id,
                        crate::FollowCamera {
                            target,
                            offset,
                            behind,
                            smoothing,
                            look,
                        },
                    )?;
                }
                if let Some(target) = &entity.animator_look {
                    let target = entities[target];
                    if let Some(mut animator) = world.component_mut::<crate::Animator>(id)? {
                        animator.look_target = Some(target);
                    }
                }
                if let Some((target, speed, stop_distance)) = entity.follow {
                    let target = entities[&target];
                    world.insert_component(
                        id,
                        crate::Follow {
                            target,
                            speed,
                            stop_distance,
                        },
                    )?;
                }
                if let Some((steering, rolling, radius)) = entity.wheels {
                    let mut wheels = Vec::with_capacity(steering.len() + rolling.len());
                    for (list, steers) in [(steering, true), (rolling, false)] {
                        for wheel in list {
                            let wheel = entities[&wheel];
                            let rest = world.local_transform(wheel)?.rotation;
                            wheels.push(crate::locomotion::Wheel {
                                id: wheel,
                                steers,
                                rest,
                            });
                        }
                    }
                    world.insert_component(id, crate::locomotion::WheelRig::new(wheels, radius))?;
                }
                if let Some((links, mut waypoint)) = entity.waypoint {
                    waypoint.links = links.iter().map(|link| entities[link]).collect();
                    world.insert_component(id, waypoint)?;
                }
                if let Some(zone) = entity.zone {
                    world.insert_component(id, zone)?;
                }
                if let Some(metadata) = entity.metadata {
                    world.insert_component(id, metadata)?;
                }
                components.extend(entity.project_components.into_iter().map(
                    |(type_id, fields)| SceneComponent {
                        entity: id,
                        type_id,
                        fields,
                    },
                ));
            }
            world.insert_resource(groups);
            let active_camera = active_camera_name.as_ref().map(|name| entities[name]);
            if let Some(camera) = active_camera {
                world.insert_resource(crate::ActiveCamera(camera));
            }
            if let Some(wind) = self.wind {
                world.insert_resource(crate::Wind::from(wind));
            }
            world.propagate_transforms();
            crate::map::refresh(world);
            Ok(LoadedScene {
                root,
                environment: self.environment.clone(),
                entities,
                renderables,
                lights,
                cameras,
                active_camera,
                components,
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

/// A reusable authored object graph with exactly one root. Loading performs all I/O.
pub struct PrefabDefinition {
    scene: SceneDefinition,
    root: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefabFile {
    schema_version: u32,
    id: String,
    name: String,
    root: String,
    entities: Vec<InstanceFile>,
}

impl PrefabDefinition {
    pub fn load(project: &Project, path: &str) -> Result<Self, SceneError> {
        Self::load_with(project, path, &crate::ComponentRegistry::builtin())
    }

    pub fn load_with(
        project: &Project,
        path: &str,
        registry: &crate::ComponentRegistry,
    ) -> Result<Self, SceneError> {
        let file: PrefabFile = read(project, path)?;
        Self::validate(project, path, file, registry)
    }

    /// Looks up an exact, unique prefab name under the project's asset roots.
    /// This performs loading-time I/O; retain the result for repeated spawning.
    pub fn load_named(project: &Project, name: &str) -> Result<Self, SceneError> {
        Self::load_named_with(project, name, &crate::ComponentRegistry::builtin())
    }

    /// Name lookup with the game's component registry. Duplicate names are errors.
    pub fn load_named_with(
        project: &Project,
        name: &str,
        registry: &crate::ComponentRegistry,
    ) -> Result<Self, SceneError> {
        if name.trim().is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control)
        {
            return Err(error(
                name,
                "prefab label must contain 1–64 printable characters",
            ));
        }
        let mut pending = project.asset_roots().to_vec();
        let mut visited = HashSet::new();
        let mut found = None;
        while let Some(directory) = pending.pop() {
            if !visited.insert(directory.clone()) {
                continue;
            }
            let absolute = project.asset_path(&directory).map_err(|e| error(name, e))?;
            for entry in std::fs::read_dir(absolute).map_err(|e| error(name, e))? {
                let entry = entry.map_err(|e| error(name, e))?;
                let kind = entry.file_type().map_err(|e| error(name, e))?;
                let relative = directory.join(entry.file_name());
                if kind.is_dir() {
                    pending.push(relative);
                } else if kind.is_file()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|file| file.ends_with(".prefab.toml"))
                {
                    let path = relative
                        .to_str()
                        .ok_or_else(|| error(name, "prefab path is not UTF-8"))?
                        .replace('\\', "/");
                    let file: PrefabFile = read(project, &path)?;
                    if file.name == name {
                        if found.is_some() {
                            return Err(error(
                                name,
                                "prefab label is ambiguous; load by asset path instead",
                            ));
                        }
                        found = Some((path, file));
                    }
                }
                // Symlinks are skipped so discovery cannot escape roots or revisit a cycle.
            }
        }
        let (path, file) =
            found.ok_or_else(|| error(name, "no prefab with this label exists in the project"))?;
        Self::validate(project, &path, file, registry)
    }

    pub fn from_source_with(
        project: &Project,
        path: &str,
        source: &str,
        registry: &crate::ComponentRegistry,
    ) -> Result<Self, SceneError> {
        project.asset_path(path).map_err(|e| error(path, e))?;
        if source.len() as u64 > MAX_FILE_BYTES {
            return Err(error(path, "prefab file exceeds 256 KiB"));
        }
        Self::validate(
            project,
            path,
            toml::from_str(source).map_err(|e| error(path, e))?,
            registry,
        )
    }

    fn validate(
        project: &Project,
        path: &str,
        file: PrefabFile,
        registry: &crate::ComponentRegistry,
    ) -> Result<Self, SceneError> {
        let roots: Vec<_> = file
            .entities
            .iter()
            .filter(|e| e.parent.is_none())
            .collect();
        if roots.len() != 1 || roots[0].id != file.root {
            return Err(error(
                path,
                "prefab requires exactly one root matching its root ID",
            ));
        }
        if file.name.trim().is_empty()
            || file.name.chars().count() > 64
            || file.name.chars().any(char::is_control)
        {
            return Err(error(
                path,
                "prefab name must contain 1–64 printable characters",
            ));
        }
        let mut scene = SceneDefinition::validate(
            project,
            path,
            SceneFile {
                schema_version: file.schema_version,
                id: file.id,
                name: Some(file.name.clone()),
                builder: None,
                kind: None,
                template: None,
                runtime_status: None,
                active_camera: None,
                entities: file.entities,
                environment: Default::default(),
                wind: None,
            },
            registry,
        )?;
        // The prefab's independently named root is its normal World entity.
        scene
            .entities
            .iter_mut()
            .find(|e| e.id == file.root)
            .expect("validated root")
            .name = file.name;
        Ok(Self {
            scene,
            root: file.root,
        })
    }

    /// Instantiates fresh world-scoped handles. `entities` is keyed by prefab-local ID.
    /// Attach returned project components and renderables just as for an authored scene.
    pub fn instantiate(&self, world: &mut World) -> Result<LoadedScene, SceneError> {
        self.instantiate_as(world, &self.scene.name)
    }

    /// Spawns an independently named instance without modifying the loaded definition.
    pub fn instantiate_as(&self, world: &mut World, name: &str) -> Result<LoadedScene, SceneError> {
        let path = self.scene.path.clone();
        if name.trim().is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control)
        {
            return Err(error(
                &path,
                "instance name must contain 1–64 printable characters",
            ));
        }
        let mut scene = self.scene.clone();
        scene
            .entities
            .iter_mut()
            .find(|e| e.id == self.root)
            .expect("validated root")
            .name = name.into();
        let mut loaded = scene.instantiate(world)?;
        let root = loaded.entities[&self.root];
        if let Err(e) = world.reparent(root, None) {
            let _ = world.remove_subtree(loaded.root);
            return Err(error(&path, e));
        }
        world
            .remove_subtree(loaded.root)
            .map_err(|e| error(&path, e))?;
        loaded.root = root;
        Ok(loaded)
    }
}

#[cfg(all(test, feature = "graphics", feature = "physics"))]
mod live_physics_tests {
    use super::*;

    #[test]
    fn physics_attributes_change_without_replacing_entities_or_transforms() {
        let project = Project::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/standard"),
        )
        .unwrap();
        let mut scene =
            SceneDefinition::load(&project, "assets/scenes/first-person.scene.toml").unwrap();
        let mut world = World::new();
        let loaded = scene.clone().instantiate(&mut world).unwrap();
        let floor = loaded.entities["floor"];
        let wall = loaded.entities["wall"];
        let pose = *world.local_transform(floor).unwrap();
        let original = *world
            .component::<crate::PhysicsBody>(wall)
            .unwrap()
            .unwrap();
        scene
            .entities
            .iter_mut()
            .find(|e| e.id == "floor")
            .unwrap()
            .physics
            .as_mut()
            .unwrap()
            .friction = 0.8;
        scene
            .apply_inspection_physics(&mut world, &loaded, "floor")
            .unwrap();
        assert_eq!(
            world
                .component::<crate::PhysicsBody>(floor)
                .unwrap()
                .unwrap()
                .friction,
            0.8
        );
        assert_eq!(*world.local_transform(floor).unwrap(), pose);
        assert_eq!(
            *world
                .component::<crate::PhysicsBody>(wall)
                .unwrap()
                .unwrap(),
            original
        );
        scene
            .entities
            .iter_mut()
            .find(|e| e.id == "floor")
            .unwrap()
            .physics = None;
        scene
            .apply_inspection_physics(&mut world, &loaded, "floor")
            .unwrap();
        assert!(world
            .component::<crate::PhysicsBody>(floor)
            .unwrap()
            .is_none());
        assert!(scene
            .apply_inspection_physics(&mut world, &loaded, "missing")
            .is_err());
        world.remove_subtree(floor).unwrap();
        let replacement = world.spawn("replacement");
        assert!(scene
            .apply_inspection_physics(&mut world, &loaded, "floor")
            .is_err());
        assert!(world
            .component::<crate::PhysicsBody>(replacement)
            .unwrap()
            .is_none());
    }
}
