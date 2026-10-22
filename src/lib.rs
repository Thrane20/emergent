//! Emergent's small, headless engine foundation.

mod engine;
mod world;

pub use engine::{Engine, EngineConfig, EngineContext, EngineError, Game};
pub use world::{Entity, EntityId, Transform, Vec3, World, WorldError};
