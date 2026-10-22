//! Emergent's small, headless engine foundation.

mod behavior;
mod engine;
mod world;

pub use behavior::{
    ports as behavior_ports, BehaviorError, BehaviorEvaluationStats, BehaviorId, BehaviorRunState,
    BehaviorSpec, BehaviorValue, BehaviorValueType, PortId,
};
pub use bevy_ecs;
pub use bevy_ecs::component::Component;
pub use engine::{
    Engine, EngineConfig, EngineContext, EngineError, Game, SchedulePhase, SimulationTime,
};
pub use glam::{Mat4, Quat, Vec3};
pub use world::{
    Entity, EntityId, EntityIdentity, EntityLifecycleCommandKind, EntityLifecycleQueue,
    EntityLifecycleResult, EntityLifecycleResults, GlobalTransform, PropagationStats, Transform,
    World, WorldError,
};
