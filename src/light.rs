use std::error::Error;
use std::f32::consts::FRAC_PI_2;
use std::fmt;

use bevy_ecs::component::Component;

use crate::Color;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightKind {
    Ambient,
    Directional,
    Point,
    Spot,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShadowCasting {
    #[default]
    Disabled,
    Enabled,
}

/// Validated light data attached to a canonical Emergent entity.
///
/// The entity transform supplies position and orientation. Directional and spot
/// lights face along the entity's transformed negative Z axis. The current
/// renderer does not yet evaluate lights or render shadows.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct LightSource {
    kind: LightKind,
    color: Color,
    intensity: f32,
    range: Option<f32>,
    spot_half_angles: Option<[f32; 2]>,
    shadow_casting: ShadowCasting,
}

impl LightSource {
    pub fn ambient(color: Color, intensity: f32) -> Result<Self, LightError> {
        Self::new(LightKind::Ambient, color, intensity, None, None)
    }

    pub fn directional(color: Color, intensity: f32) -> Result<Self, LightError> {
        Self::new(LightKind::Directional, color, intensity, None, None)
    }

    pub fn point(color: Color, intensity: f32, range: f32) -> Result<Self, LightError> {
        validate_range(range)?;
        Self::new(LightKind::Point, color, intensity, Some(range), None)
    }

    pub fn spot(
        color: Color,
        intensity: f32,
        range: f32,
        inner_half_angle: f32,
        outer_half_angle: f32,
    ) -> Result<Self, LightError> {
        validate_range(range)?;
        validate_spot_angles(inner_half_angle, outer_half_angle)?;
        Self::new(
            LightKind::Spot,
            color,
            intensity,
            Some(range),
            Some([inner_half_angle, outer_half_angle]),
        )
    }

    pub fn with_shadow_casting(
        mut self,
        shadow_casting: ShadowCasting,
    ) -> Result<Self, LightError> {
        if self.kind == LightKind::Ambient && shadow_casting == ShadowCasting::Enabled {
            return Err(LightError::AmbientCannotCastShadows);
        }
        self.shadow_casting = shadow_casting;
        Ok(self)
    }

    pub fn kind(self) -> LightKind {
        self.kind
    }

    pub fn color(self) -> Color {
        self.color
    }

    pub fn intensity(self) -> f32 {
        self.intensity
    }

    pub fn range(self) -> Option<f32> {
        self.range
    }

    pub fn spot_half_angles(self) -> Option<[f32; 2]> {
        self.spot_half_angles
    }

    pub fn shadow_casting(self) -> ShadowCasting {
        self.shadow_casting
    }

    fn new(
        kind: LightKind,
        color: Color,
        intensity: f32,
        range: Option<f32>,
        spot_half_angles: Option<[f32; 2]>,
    ) -> Result<Self, LightError> {
        validate_intensity(intensity)?;
        Ok(Self {
            kind,
            color,
            intensity,
            range,
            spot_half_angles,
            shadow_casting: ShadowCasting::Disabled,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightError {
    InvalidIntensity,
    InvalidRange,
    InvalidSpotAngles,
    AmbientCannotCastShadows,
}

impl fmt::Display for LightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIntensity => {
                formatter.write_str("light intensity must be finite and non-negative")
            }
            Self::InvalidRange => formatter.write_str("light range must be finite and positive"),
            Self::InvalidSpotAngles => formatter.write_str(
                "spot half-angles must be finite, ordered, and between zero and 90 degrees",
            ),
            Self::AmbientCannotCastShadows => {
                formatter.write_str("ambient lights cannot request shadow casting")
            }
        }
    }
}

impl Error for LightError {}

fn validate_intensity(intensity: f32) -> Result<(), LightError> {
    if intensity.is_finite() && intensity >= 0.0 {
        Ok(())
    } else {
        Err(LightError::InvalidIntensity)
    }
}

fn validate_range(range: f32) -> Result<(), LightError> {
    if range.is_finite() && range > 0.0 {
        Ok(())
    } else {
        Err(LightError::InvalidRange)
    }
}

fn validate_spot_angles(inner: f32, outer: f32) -> Result<(), LightError> {
    if inner.is_finite() && outer.is_finite() && inner >= 0.0 && inner < outer && outer <= FRAC_PI_2
    {
        Ok(())
    } else {
        Err(LightError::InvalidSpotAngles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Vec3, World, WorldError};

    #[test]
    fn constructs_every_supported_light_kind() {
        let ambient = LightSource::ambient(Color::WHITE, 0.2).unwrap();
        let directional = LightSource::directional(Color::WHITE, 3.0)
            .unwrap()
            .with_shadow_casting(ShadowCasting::Enabled)
            .unwrap();
        let point = LightSource::point(Color::YELLOW, 8.0, 20.0).unwrap();
        let spot = LightSource::spot(Color::WHITE, 12.0, 30.0, 0.2, 0.6).unwrap();

        assert_eq!(ambient.kind(), LightKind::Ambient);
        assert_eq!(directional.kind(), LightKind::Directional);
        assert_eq!(directional.shadow_casting(), ShadowCasting::Enabled);
        assert_eq!(point.range(), Some(20.0));
        assert_eq!(spot.spot_half_angles(), Some([0.2, 0.6]));
        assert_eq!(spot.color(), Color::WHITE);
        assert_eq!(spot.intensity(), 12.0);
    }

    #[test]
    fn rejects_invalid_light_parameters() {
        assert_eq!(
            LightSource::ambient(Color::WHITE, f32::NAN),
            Err(LightError::InvalidIntensity)
        );
        assert_eq!(
            LightSource::directional(Color::WHITE, -1.0),
            Err(LightError::InvalidIntensity)
        );
        assert_eq!(
            LightSource::point(Color::WHITE, 1.0, 0.0),
            Err(LightError::InvalidRange)
        );
        assert_eq!(
            LightSource::spot(Color::WHITE, 1.0, 10.0, 0.8, 0.4),
            Err(LightError::InvalidSpotAngles)
        );
        assert_eq!(
            LightSource::ambient(Color::WHITE, 1.0)
                .unwrap()
                .with_shadow_casting(ShadowCasting::Enabled),
            Err(LightError::AmbientCannotCastShadows)
        );
    }

    #[test]
    fn light_component_uses_canonical_entity_lifecycle() {
        let mut world = World::new();
        let root = world.spawn("scene");
        let light_entity = world.spawn_child(root, "light").unwrap();
        world
            .set_local_transform(
                light_entity,
                crate::Transform::from_translation(Vec3::new(0.0, 4.0, 2.0)),
            )
            .unwrap();
        let light = LightSource::point(Color::WHITE, 4.0, 16.0).unwrap();
        world.insert_component(light_entity, light).unwrap();

        assert_eq!(
            world.component::<LightSource>(light_entity).unwrap(),
            Some(&light)
        );
        assert_eq!(world.remove_subtree(light_entity).unwrap(), 1);
        assert!(matches!(
            world.component::<LightSource>(light_entity),
            Err(WorldError::MissingEntity(_))
        ));
    }
}
