use std::fmt;

use bevy_ecs::component::Component;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraProjection {
    Perspective {
        vertical_fov_radians: f32,
        near: f32,
        far: f32,
    },
    Isometric {
        vertical_size: f32,
        near: f32,
        far: f32,
    },
}

impl CameraProjection {
    /// Camera-local near/far rectangles, looking down -Z, without transform scale.
    pub fn frustum_corners(self, aspect: f32) -> Result<[crate::Vec3; 8], CameraError> {
        if !aspect.is_finite() || aspect <= 0.0 {
            return Err(CameraError(
                "camera aspect ratio must be finite and positive",
            ));
        }
        validate_planes(self.near(), self.far())?;
        let half_heights = match self {
            Self::Perspective {
                vertical_fov_radians,
                near,
                far,
            } => {
                if !vertical_fov_radians.is_finite()
                    || !(1.0_f32.to_radians()..179.0_f32.to_radians())
                        .contains(&vertical_fov_radians)
                {
                    return Err(CameraError("invalid perspective field of view"));
                }
                let tangent = (vertical_fov_radians * 0.5).tan();
                [near * tangent, far * tangent]
            }
            Self::Isometric {
                vertical_size,
                near,
                far,
            } => {
                Self::isometric(vertical_size, near, far)?;
                [vertical_size * 0.5; 2]
            }
        };
        let corners = std::array::from_fn(|i| {
            let y = half_heights[i / 4];
            crate::Vec3::new(
                if i % 4 == 0 || i % 4 == 3 {
                    -y * aspect
                } else {
                    y * aspect
                },
                if i % 4 < 2 { -y } else { y },
                -if i < 4 { self.near() } else { self.far() },
            )
        });
        if !corners.iter().all(|corner| corner.is_finite()) {
            return Err(CameraError("camera frustum exceeds finite bounds"));
        }
        Ok(corners)
    }

    pub fn perspective(
        vertical_fov_degrees: f32,
        near: f32,
        far: f32,
    ) -> Result<Self, CameraError> {
        if !vertical_fov_degrees.is_finite() || !(1.0..179.0).contains(&vertical_fov_degrees) {
            return Err(CameraError(
                "perspective field of view must be finite and between 1 and 179 degrees",
            ));
        }
        validate_planes(near, far)?;
        Ok(Self::Perspective {
            vertical_fov_radians: vertical_fov_degrees.to_radians(),
            near,
            far,
        })
    }

    pub fn isometric(vertical_size: f32, near: f32, far: f32) -> Result<Self, CameraError> {
        if !vertical_size.is_finite() || !(0.001..=1_000_000.0).contains(&vertical_size) {
            return Err(CameraError(
                "isometric view size must be finite and within 0.001..=1000000",
            ));
        }
        validate_planes(near, far)?;
        Ok(Self::Isometric {
            vertical_size,
            near,
            far,
        })
    }

    pub fn near(self) -> f32 {
        match self {
            Self::Perspective { near, .. } | Self::Isometric { near, .. } => near,
        }
    }

    pub fn far(self) -> f32 {
        match self {
            Self::Perspective { far, .. } | Self::Isometric { far, .. } => far,
        }
    }

    pub fn with_far_at_least(self, minimum: f32) -> Self {
        let far = self.far().max(minimum);
        match self {
            Self::Perspective {
                vertical_fov_radians,
                near,
                ..
            } => Self::Perspective {
                vertical_fov_radians,
                near,
                far,
            },
            Self::Isometric {
                vertical_size,
                near,
                ..
            } => Self::Isometric {
                vertical_size,
                near,
                far,
            },
        }
    }
}

#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct Camera {
    projection: CameraProjection,
}

impl Camera {
    pub fn new(projection: CameraProjection) -> Self {
        Self { projection }
    }

    pub fn projection(self) -> CameraProjection {
        self.projection
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CameraError(pub &'static str);

impl fmt::Display for CameraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for CameraError {}

fn validate_planes(near: f32, far: f32) -> Result<(), CameraError> {
    if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near || far > 1_000_000.0 {
        return Err(CameraError(
            "camera clipping planes require 0 < near < far <= 1000000",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_parameters_are_bounded() {
        assert!(CameraProjection::perspective(60.0, 0.1, 1_000.0).is_ok());
        assert!(CameraProjection::perspective(180.0, 0.1, 1_000.0).is_err());
        assert!(CameraProjection::isometric(20.0, 0.1, 1_000.0).is_ok());
        assert!(CameraProjection::isometric(0.0, 0.1, 1_000.0).is_err());
        assert!(CameraProjection::perspective(60.0, 10.0, 1.0).is_err());
    }

    #[test]
    fn frustum_matches_near_far_fov_and_orthographic_extent() {
        let corners = CameraProjection::perspective(90., 1., 10.)
            .unwrap()
            .frustum_corners(2.)
            .unwrap();
        assert!(corners[0].abs_diff_eq(crate::Vec3::new(-2., -1., -1.), 1e-5));
        assert!(corners[6].abs_diff_eq(crate::Vec3::new(20., 10., -10.), 1e-4));
        let ortho = CameraProjection::isometric(6., 0.5, 12.)
            .unwrap()
            .frustum_corners(2.)
            .unwrap();
        assert_eq!(ortho[0], crate::Vec3::new(-6., -3., -0.5));
        assert_eq!(ortho[6], crate::Vec3::new(6., 3., -12.));
        for aspect in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(CameraProjection::isometric(6., 0.5, 12.)
                .unwrap()
                .frustum_corners(aspect)
                .is_err());
        }
        assert!(CameraProjection::perspective(1., 0.1, 100.)
            .unwrap()
            .frustum_corners(1.)
            .is_ok());
    }
}
