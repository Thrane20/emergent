//! Emergent's small, headless engine foundation.

mod behavior;
mod engine;
mod geometry;
#[cfg(feature = "graphics")]
mod graphics;
mod light;
mod primitives;
mod project;
mod world;

pub use behavior::{
    ports as behavior_ports, BehaviorError, BehaviorEvaluationStats, BehaviorId, BehaviorRunState,
    BehaviorSpec, BehaviorValue, BehaviorValueType, PortId,
};
pub use bevy_ecs;
pub use bevy_ecs::component::Component;
pub use engine::{
    Engine, EngineConfig, EngineContext, EngineError, FrameAdvance, Game, SchedulePhase,
    SimulationTime,
};
pub use geometry::{Color, Geometry, GeometryError, Line, Mesh, MeshVertex, Point, Polygon};
pub use glam::{Mat4, Quat, Vec3};
#[cfg(feature = "graphics")]
pub use graphics::{
    CubeRenderer, GeometryId, GeometryInstance, GeometryRenderer, GraphicsError, RenderOutcome,
    RendererInfo,
};
pub use light::{LightError, LightKind, LightSource, ShadowCasting};
pub use primitives::{Primitive, PrimitiveKind};
pub use project::{
    BuildProfile, Project, ProjectError, ProjectManifest, ProjectOutput, TargetOs,
    PROJECT_SCHEMA_VERSION,
};
pub use world::{
    Entity, EntityId, EntityIdentity, EntityLifecycleCommandKind, EntityLifecycleQueue,
    EntityLifecycleResult, EntityLifecycleResults, GlobalTransform, PropagationStats, Transform,
    World, WorldError,
};
