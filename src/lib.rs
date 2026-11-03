//! Emergent's small, headless engine foundation.
//!
//! World lengths use metres (1 world unit = 1 m). Transform scale is dimensionless;
//! it multiplies authored geometry dimensions and composes through the hierarchy.

pub mod audio;
mod behavior;
pub mod brain;
mod camera;
pub mod component;
pub mod control;
#[cfg(feature = "density-mesh")]
mod density_mesh;
mod engine;
#[cfg(feature = "density-mesh")]
pub use density_mesh::{DensityMesh, DensityMeshError, DENSITY_MAX_SAMPLES};
mod environment;
pub mod fixed_wing;
mod geometry;
#[cfg(feature = "graphics")]
mod gpu_profile;
#[cfg(feature = "graphics")]
mod graphics;
mod input;
pub mod locomotion;
pub mod map;
mod model;
mod motion;
pub mod tower;
pub mod wind;
#[cfg(feature = "graphics")]
pub use gpu_profile::GpuFrameProfile;
pub use wind::{Wind, WindDefinition};
mod inspection;
mod light;
mod material;
mod noise;
#[cfg(feature = "graphics")]
mod particle_graphics;
mod particles;
mod physics;
pub mod plugin;
mod primitives;
mod project;
#[cfg(feature = "graphics")]
pub mod project_host;
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
pub mod ui;
pub mod ui_surface;
pub use ui_surface::UiSurface;
mod world;

pub use audio::{
    AudioClipAsset, AudioConfig, AudioEmitter, AudioError, AudioListener, AudioWorld, Bus, Clip,
    ClipId, Mixer, PlayAt, ReverbSettings, Rolloff, SoundId, SoundSettings,
};
pub use behavior::{
    ports as behavior_ports, BehaviorError, BehaviorEvaluationStats, BehaviorId, BehaviorRunState,
    BehaviorSpec, BehaviorValue, BehaviorValueType, PortId,
};
pub use bevy_ecs;
pub use bevy_ecs::component::Component;
pub use brain::{
    BrainAgent, BrainDefinition, BrainSettings, CompiledBrain, Group, GroupNames, Look,
    PublishedSignals, Signal,
};
pub use camera::{Camera, CameraError, CameraProjection};
pub use component::{ComponentField, ComponentRegistry, ComponentType, ComponentValues, FieldKind};
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
pub use input::{Input, Key, KeySet, MouseMotion};
pub use inspection::{
    gizmo_frame, project_point, GizmoTool, InspectionRay, PickingMesh, TransformGizmo,
    TransformSpace,
};
pub use light::{LightError, LightKind, LightSource, ShadowCasting};
pub use locomotion::{Activity, LocomotionAgent, LocomotionProfile, Wheel, WheelRig};
pub use map::{MapError, MetaValue, Metadata, Waypoint, Zone, ZoneShape};
pub use motion::{
    ActiveCamera, CharacterController, Follow, FollowCamera, LookAt, MouseLook, Spin,
};
pub use plugin::{Plugin, PluginRuntime, PluginRuntimes};
pub use primitives::{Primitive, PrimitiveKind};
pub use project::{
    BuildProfile, PluginManifest, Project, ProjectError, ProjectManifest, ProjectOutput,
    ProjectPlugin, TargetOs, PLUGIN_SCHEMA_VERSION, PROJECT_SCHEMA_VERSION,
};
pub use scene::{
    LoadedScene, PrefabDefinition, SceneComponent, SceneDefinition, SceneError, SCENE_MAX_BYTES,
    SCENE_MAX_INSTANCES,
};
pub use world::{
    Entity, EntityId, EntityIdentity, EntityLifecycleCommandKind, EntityLifecycleQueue,
    EntityLifecycleResult, EntityLifecycleResults, GlobalTransform, PropagationStats, Transform,
    World, WorldError,
};

pub use noise::{
    NoiseAlgorithm, NoiseDefinition, NoiseError, NoiseField, NoiseGenerator, NoiseSettings,
    NOISE_MAX_EVALUATIONS,
};

pub use physics::{
    BodyKind, BoxCollider, Collider, PhysicsBody, PhysicsBodyState, PhysicsError, TerrainCollider,
    PHYSICS_GRAVITY,
};

pub use material::{
    Material, MaterialDefinition, MaterialError, MaterialLayer, MaterialNoiseMap, MaterialShading,
    MaterialTextureMap, TerrainPaint,
};
pub use model::{
    ImportedModel, ModelError, MODEL_MAX_BYTES, MODEL_MAX_TRIANGLES, MODEL_MAX_VERTICES,
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

mod shader;
pub use shader::{ShaderDefinition, ShaderNode, ShaderOp, ShaderOutputs, ShaderParameter};

mod object_labels;
pub use object_labels::{ObjectClassification, ObjectLabels};
