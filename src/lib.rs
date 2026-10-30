//! Emergent's small, headless engine foundation.
//!
//! World lengths use metres (1 world unit = 1 m). Transform scale is dimensionless;
//! it multiplies authored geometry dimensions and composes through the hierarchy.

mod behavior;
mod camera;
#[cfg(feature = "density-mesh")]
mod density_mesh;
mod engine;
#[cfg(feature = "density-mesh")]
pub use density_mesh::{DensityMesh, DensityMeshError, DENSITY_MAX_SAMPLES};
mod environment;
mod geometry;
#[cfg(feature = "graphics")]
mod gpu_profile;
#[cfg(feature = "graphics")]
mod graphics;
#[cfg(feature = "graphics")]
pub use gpu_profile::GpuFrameProfile;
mod inspection;
mod light;
mod material;
mod noise;
#[cfg(feature = "graphics")]
mod particle_graphics;
mod particles;
mod physics;
mod primitives;
mod project;
mod raymarched_terrain;
#[cfg(feature = "graphics")]
mod raymarched_terrain_graphics;
#[cfg(feature = "graphics")]
pub use raymarched_terrain_graphics::{RaymarchedFrameStats, RaymarchedRenderOptions};
mod scene;
#[cfg(feature = "graphics")]
mod skybox_graphics;
mod structure;
mod terrain;
mod texture;
mod tween;
mod world;

pub use behavior::{
    ports as behavior_ports, BehaviorError, BehaviorEvaluationStats, BehaviorId, BehaviorRunState,
    BehaviorSpec, BehaviorValue, BehaviorValueType, PortId,
};
pub use bevy_ecs;
pub use bevy_ecs::component::Component;
pub use camera::{Camera, CameraError, CameraProjection};
pub use engine::{
    Engine, EngineConfig, EngineContext, EngineError, FrameAdvance, Game, SchedulePhase,
    SimulationTime,
};
pub use geometry::{Color, Geometry, GeometryError, Line, Mesh, MeshVertex, Point, Polygon};
pub use glam::{EulerRot, Mat4, Quat, Vec2, Vec3};
#[cfg(feature = "graphics")]
pub use graphics::{
    CubeRenderer, GeometryId, GeometryInstance, GeometryRenderer, GraphicsError, LightInstance,
    RaymarchedTerrainId, RenderOutcome, RenderedFrame, RendererInfo, ShadingMode,
};
pub use inspection::{
    gizmo_frame, project_point, GizmoTool, InspectionRay, PickingMesh, TransformGizmo,
    TransformSpace,
};
pub use light::{LightError, LightKind, LightSource, ShadowCasting};
pub use primitives::{Primitive, PrimitiveKind};
pub use project::{
    BuildProfile, Project, ProjectError, ProjectManifest, ProjectOutput, TargetOs,
    PROJECT_SCHEMA_VERSION,
};
pub use scene::{LoadedScene, SceneDefinition, SceneError};
pub use world::{
    Entity, EntityId, EntityIdentity, EntityLifecycleCommandKind, EntityLifecycleQueue,
    EntityLifecycleResult, EntityLifecycleResults, GlobalTransform, PropagationStats, Transform,
    World, WorldError,
};

pub use noise::{
    NoiseAlgorithm, NoiseDefinition, NoiseError, NoiseField, NoiseGenerator, NoiseSettings,
    NOISE_MAX_EVALUATIONS,
};

pub use physics::{BodyKind, BoxCollider, Collider, PhysicsBody, PhysicsError, TerrainCollider};

pub use material::{
    Material, MaterialDefinition, MaterialError, MaterialLayer, MaterialNoiseMap, MaterialShading,
    MaterialTextureMap, TerrainPaint,
};

pub use terrain::{TerrainBase, TerrainField, TerrainPreset, TerrainSettings, TERRAIN_RESOLUTIONS};

pub use texture::{TextureImage, TEXTURE_MAX_BYTES, TEXTURE_MAX_PIXELS};
pub use tween::{lerp, Easing, Tween, TweenDefinition, TweenError, TweenPlayback, TweenSettings};

pub use structure::{
    PolygonMesh, StructureDefinition, StructureError, StructurePart, StructureTransform,
};

pub use particles::{
    Particle, ParticleBlend, ParticleDefinition, ParticleEffect, ParticleEmitter, ParticleError,
    ParticleShape, ParticleStore, ParticleSystemId, PARTICLE_MAX_LIVE, PARTICLE_STEP,
};

pub use environment::{EnvironmentDefinition, SceneEnvironment};
pub use raymarched_terrain::{RaymarchedBiome, RaymarchedTerrain};
