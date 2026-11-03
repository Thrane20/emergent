//! Uniform wind: a scene's `[wind]` section (speed and the compass direction it blows from) and the
//! world resource physical aircraft fly through. Game code may replace the resource at runtime.
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::Vec3;

/// Authored wind: metres per second, blowing *from* a compass bearing (0 = from −Z/north).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WindDefinition {
    pub speed: f32,
    #[serde(default)]
    pub from: f32,
}

impl WindDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if !self.speed.is_finite()
            || !(0.0..=100.0).contains(&self.speed)
            || !self.from.is_finite()
            || !(0.0..=360.0).contains(&self.from)
        {
            return Err("wind needs a speed of 0-100 m/s and a from-bearing of 0-360°".into());
        }
        Ok(())
    }
}

/// The air's velocity everywhere, m/s. Default: calm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct Wind {
    pub velocity: Vec3,
}

impl Wind {
    /// Wind of `speed` blowing from the compass bearing `from_degrees` (towards the opposite one).
    pub fn from_compass(speed: f32, from_degrees: f32) -> Self {
        let from = from_degrees.to_radians();
        Self {
            velocity: -Vec3::new(from.sin(), 0.0, -from.cos()) * speed,
        }
    }
    /// (speed, from-bearing in degrees) of the horizontal wind.
    pub fn compass(&self) -> (f32, f32) {
        let flat = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
        let speed = flat.length();
        let from = if speed > 1e-4 {
            (-flat.x).atan2(flat.z).to_degrees().rem_euclid(360.0)
        } else {
            0.0
        };
        (speed, from)
    }
}

impl From<WindDefinition> for Wind {
    fn from(definition: WindDefinition) -> Self {
        Self::from_compass(definition.speed, definition.from)
    }
}
