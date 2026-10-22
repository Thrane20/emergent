//! Emergent's small, headless engine foundation.

mod engine;
mod world;

pub use engine::{Engine, EngineConfig, EngineContext, EngineError, Game};
pub use glam::{Mat4, Quat, Vec3};
pub use world::{
    Entity, EntityId, GlobalTransform, PropagationStats, Transform, World, WorldError,
};
