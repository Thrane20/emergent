use std::f32::consts::{PI, TAU};

use glam::Vec3;

use crate::{Color, GeometryError, Mesh, MeshVertex};

const MIN_SEGMENTS: u32 = 3;
const MIN_SPHERE_STACKS: u32 = 2;
const MIN_CAPSULE_RINGS: u32 = 2;
const MAX_SUBDIVISIONS: u32 = 512;
const MIN_DIMENSION: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimitiveKind {
    Plane,
    Cuboid,
    Pyramid,
    Sphere,
    Cylinder,
    Cone,
    Capsule,
    Torus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Primitive {
    shape: PrimitiveShape,
}

#[derive(Clone, Debug, PartialEq)]
enum PrimitiveShape {
    Plane {
        half_width: f32,
        half_depth: f32,
        color: Color,
    },
    Cuboid {
        half_extents: Vec3,
        color: Color,
    },
    Pyramid {
        half_extent: f32,
        height: f32,
        color: Color,
    },
    Sphere {
        radius: f32,
        sectors: u32,
        stacks: u32,
        color: Color,
    },
    Cylinder {
        radius: f32,
        half_height: f32,
        segments: u32,
        color: Color,
    },
    Cone {
        radius: f32,
        height: f32,
        segments: u32,
        color: Color,
    },
    Capsule {
        radius: f32,
        half_segment: f32,
        segments: u32,
        hemisphere_rings: u32,
        color: Color,
    },
    Torus {
        major_radius: f32,
        minor_radius: f32,
        major_segments: u32,
        minor_segments: u32,
        color: Color,
    },
}

impl Primitive {
    pub fn plane(half_width: f32, half_depth: f32, color: Color) -> Result<Self, GeometryError> {
        validate_dimensions(&[half_width, half_depth])?;
        Ok(Self {
            shape: PrimitiveShape::Plane {
                half_width,
                half_depth,
                color,
            },
        })
    }

    pub fn cuboid(half_extents: Vec3, color: Color) -> Result<Self, GeometryError> {
        if !half_extents.is_finite() {
            return Err(GeometryError::InvalidDimensions);
        }
        validate_dimensions(&half_extents.to_array())?;
        Ok(Self {
            shape: PrimitiveShape::Cuboid {
                half_extents,
                color,
            },
        })
    }

    pub fn cube(half_extent: f32, color: Color) -> Result<Self, GeometryError> {
        Self::cuboid(Vec3::splat(half_extent), color)
    }

    pub fn pyramid(half_extent: f32, height: f32, color: Color) -> Result<Self, GeometryError> {
        validate_dimensions(&[half_extent, height])?;
        Ok(Self {
            shape: PrimitiveShape::Pyramid {
                half_extent,
                height,
                color,
            },
        })
    }

    pub fn sphere(
        radius: f32,
        sectors: u32,
        stacks: u32,
        color: Color,
    ) -> Result<Self, GeometryError> {
        validate_dimensions(&[radius])?;
        validate_subdivision("sphere sectors", sectors, MIN_SEGMENTS)?;
        validate_subdivision("sphere stacks", stacks, MIN_SPHERE_STACKS)?;
        Ok(Self {
            shape: PrimitiveShape::Sphere {
                radius,
                sectors,
                stacks,
                color,
            },
        })
    }

    pub fn cylinder(
        radius: f32,
        half_height: f32,
        segments: u32,
        color: Color,
    ) -> Result<Self, GeometryError> {
        validate_dimensions(&[radius, half_height])?;
        validate_subdivision("cylinder segments", segments, MIN_SEGMENTS)?;
        Ok(Self {
            shape: PrimitiveShape::Cylinder {
                radius,
                half_height,
                segments,
                color,
            },
        })
    }

    pub fn cone(
        radius: f32,
        height: f32,
        segments: u32,
        color: Color,
    ) -> Result<Self, GeometryError> {
        validate_dimensions(&[radius, height])?;
        validate_subdivision("cone segments", segments, MIN_SEGMENTS)?;
        Ok(Self {
            shape: PrimitiveShape::Cone {
                radius,
                height,
                segments,
                color,
            },
        })
    }

    pub fn capsule(
        radius: f32,
        half_segment: f32,
        segments: u32,
        hemisphere_rings: u32,
        color: Color,
    ) -> Result<Self, GeometryError> {
        validate_dimensions(&[radius, half_segment])?;
        validate_subdivision("capsule segments", segments, MIN_SEGMENTS)?;
        validate_subdivision(
            "capsule hemisphere rings",
            hemisphere_rings,
            MIN_CAPSULE_RINGS,
        )?;
        Ok(Self {
            shape: PrimitiveShape::Capsule {
                radius,
                half_segment,
                segments,
                hemisphere_rings,
                color,
            },
        })
    }

    pub fn torus(
        major_radius: f32,
        minor_radius: f32,
        major_segments: u32,
        minor_segments: u32,
        color: Color,
    ) -> Result<Self, GeometryError> {
        validate_dimensions(&[major_radius, minor_radius])?;
        if major_radius <= minor_radius {
            return Err(GeometryError::InvalidTorusRadii);
        }
        validate_subdivision("torus major segments", major_segments, MIN_SEGMENTS)?;
        validate_subdivision("torus minor segments", minor_segments, MIN_SEGMENTS)?;
        Ok(Self {
            shape: PrimitiveShape::Torus {
                major_radius,
                minor_radius,
                major_segments,
                minor_segments,
                color,
            },
        })
    }

    pub fn kind(&self) -> PrimitiveKind {
        match self.shape {
            PrimitiveShape::Plane { .. } => PrimitiveKind::Plane,
            PrimitiveShape::Cuboid { .. } => PrimitiveKind::Cuboid,
            PrimitiveShape::Pyramid { .. } => PrimitiveKind::Pyramid,
            PrimitiveShape::Sphere { .. } => PrimitiveKind::Sphere,
            PrimitiveShape::Cylinder { .. } => PrimitiveKind::Cylinder,
            PrimitiveShape::Cone { .. } => PrimitiveKind::Cone,
            PrimitiveShape::Capsule { .. } => PrimitiveKind::Capsule,
            PrimitiveShape::Torus { .. } => PrimitiveKind::Torus,
        }
    }

    pub fn to_mesh(&self) -> Result<Mesh, GeometryError> {
        match self.shape {
            PrimitiveShape::Plane {
                half_width,
                half_depth,
                color,
            } => plane_mesh(half_width, half_depth, color),
            PrimitiveShape::Cuboid {
                half_extents,
                color,
            } => cuboid_mesh(half_extents, color),
            PrimitiveShape::Pyramid {
                half_extent,
                height,
                color,
            } => pyramid_mesh(half_extent, height, color),
            PrimitiveShape::Sphere {
                radius,
                sectors,
                stacks,
                color,
            } => sphere_mesh(radius, sectors, stacks, color),
            PrimitiveShape::Cylinder {
                radius,
                half_height,
                segments,
                color,
            } => cylinder_mesh(radius, half_height, segments, color),
            PrimitiveShape::Cone {
                radius,
                height,
                segments,
                color,
            } => cone_mesh(radius, height, segments, color),
            PrimitiveShape::Capsule {
                radius,
                half_segment,
                segments,
                hemisphere_rings,
                color,
            } => capsule_mesh(radius, half_segment, segments, hemisphere_rings, color),
            PrimitiveShape::Torus {
                major_radius,
                minor_radius,
                major_segments,
                minor_segments,
                color,
            } => torus_mesh(
                major_radius,
                minor_radius,
                major_segments,
                minor_segments,
                color,
            ),
        }
    }
}

fn vertex(position: Vec3, color: Color) -> MeshVertex {
    MeshVertex::new(position, color).expect("validated primitive math produces finite vertices")
}

fn plane_mesh(half_width: f32, half_depth: f32, color: Color) -> Result<Mesh, GeometryError> {
    Mesh::new(
        vec![
            vertex(Vec3::new(-half_width, 0.0, -half_depth), color),
            vertex(Vec3::new(-half_width, 0.0, half_depth), color),
            vertex(Vec3::new(half_width, 0.0, half_depth), color),
            vertex(Vec3::new(half_width, 0.0, -half_depth), color),
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
}

fn cuboid_mesh(half: Vec3, color: Color) -> Result<Mesh, GeometryError> {
    let vertices = [
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(half.x, -half.y, half.z),
        Vec3::new(half.x, half.y, half.z),
        Vec3::new(-half.x, half.y, half.z),
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(-half.x, half.y, -half.z),
    ]
    .into_iter()
    .map(|position| vertex(position, color))
    .collect();
    Mesh::new(
        vertices,
        vec![
            0, 1, 2, 0, 2, 3, 1, 5, 6, 1, 6, 2, 5, 4, 7, 5, 7, 6, 4, 0, 3, 4, 3, 7, 3, 2, 6, 3, 6,
            7, 4, 5, 1, 4, 1, 0,
        ],
    )
}

fn pyramid_mesh(half_extent: f32, height: f32, color: Color) -> Result<Mesh, GeometryError> {
    let half_height = height * 0.5;
    Mesh::new(
        vec![
            vertex(Vec3::new(-half_extent, -half_height, half_extent), color),
            vertex(Vec3::new(half_extent, -half_height, half_extent), color),
            vertex(Vec3::new(half_extent, -half_height, -half_extent), color),
            vertex(Vec3::new(-half_extent, -half_height, -half_extent), color),
            vertex(Vec3::new(0.0, half_height, 0.0), color),
        ],
        vec![0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4, 0, 3, 2, 0, 2, 1],
    )
}

fn sphere_mesh(
    radius: f32,
    sectors: u32,
    stacks: u32,
    color: Color,
) -> Result<Mesh, GeometryError> {
    let ring_count = stacks - 1;
    let mut vertices = Vec::with_capacity((2 + ring_count * sectors) as usize);
    vertices.push(vertex(Vec3::Y * radius, color));
    for ring in 1..stacks {
        let latitude = PI * ring as f32 / stacks as f32;
        let y = radius * latitude.cos();
        let ring_radius = radius * latitude.sin();
        push_ring(&mut vertices, y, ring_radius, sectors, color);
    }
    let bottom = vertices.len() as u32;
    vertices.push(vertex(Vec3::NEG_Y * radius, color));

    let mut indices = Vec::with_capacity((6 * sectors * ring_count) as usize);
    for sector in 0..sectors {
        let next = (sector + 1) % sectors;
        indices.extend_from_slice(&[0, 1 + next, 1 + sector]);
    }
    for ring in 0..ring_count.saturating_sub(1) {
        connect_rings(
            &mut indices,
            1 + ring * sectors,
            1 + (ring + 1) * sectors,
            sectors,
        );
    }
    let last_ring = 1 + (ring_count - 1) * sectors;
    for sector in 0..sectors {
        let next = (sector + 1) % sectors;
        indices.extend_from_slice(&[bottom, last_ring + sector, last_ring + next]);
    }
    Mesh::new(vertices, indices)
}

fn cylinder_mesh(
    radius: f32,
    half_height: f32,
    segments: u32,
    color: Color,
) -> Result<Mesh, GeometryError> {
    let mut vertices = Vec::with_capacity((2 * segments + 2) as usize);
    push_ring(&mut vertices, -half_height, radius, segments, color);
    push_ring(&mut vertices, half_height, radius, segments, color);
    let bottom_center = vertices.len() as u32;
    vertices.push(vertex(Vec3::NEG_Y * half_height, color));
    let top_center = vertices.len() as u32;
    vertices.push(vertex(Vec3::Y * half_height, color));
    let mut indices = Vec::with_capacity((12 * segments) as usize);
    connect_rings(&mut indices, 0, segments, segments);
    for segment in 0..segments {
        let next = (segment + 1) % segments;
        indices.extend_from_slice(&[bottom_center, next, segment]);
        indices.extend_from_slice(&[top_center, segments + segment, segments + next]);
    }
    Mesh::new(vertices, indices)
}

fn cone_mesh(radius: f32, height: f32, segments: u32, color: Color) -> Result<Mesh, GeometryError> {
    let half_height = height * 0.5;
    let mut vertices = Vec::with_capacity((segments + 2) as usize);
    push_ring(&mut vertices, -half_height, radius, segments, color);
    let apex = vertices.len() as u32;
    vertices.push(vertex(Vec3::Y * half_height, color));
    let center = vertices.len() as u32;
    vertices.push(vertex(Vec3::NEG_Y * half_height, color));
    let mut indices = Vec::with_capacity((6 * segments) as usize);
    for segment in 0..segments {
        let next = (segment + 1) % segments;
        indices.extend_from_slice(&[segment, next, apex]);
        indices.extend_from_slice(&[center, next, segment]);
    }
    Mesh::new(vertices, indices)
}

fn capsule_mesh(
    radius: f32,
    half_segment: f32,
    segments: u32,
    hemisphere_rings: u32,
    color: Color,
) -> Result<Mesh, GeometryError> {
    let profile_ring_count = hemisphere_rings * 2;
    let mut vertices = Vec::with_capacity((2 + profile_ring_count * segments) as usize);
    vertices.push(vertex(Vec3::Y * (half_segment + radius), color));
    for ring in 1..=hemisphere_rings {
        let angle = PI * 0.5 * ring as f32 / hemisphere_rings as f32;
        push_ring(
            &mut vertices,
            half_segment + radius * angle.cos(),
            radius * angle.sin(),
            segments,
            color,
        );
    }
    for ring in 0..hemisphere_rings {
        let angle = PI * 0.5 * ring as f32 / hemisphere_rings as f32;
        push_ring(
            &mut vertices,
            -half_segment - radius * angle.sin(),
            radius * angle.cos(),
            segments,
            color,
        );
    }
    let bottom = vertices.len() as u32;
    vertices.push(vertex(Vec3::NEG_Y * (half_segment + radius), color));

    let mut indices = Vec::with_capacity((6 * segments * profile_ring_count) as usize);
    for segment in 0..segments {
        let next = (segment + 1) % segments;
        indices.extend_from_slice(&[0, 1 + next, 1 + segment]);
    }
    for ring in 0..profile_ring_count - 1 {
        connect_rings(
            &mut indices,
            1 + ring * segments,
            1 + (ring + 1) * segments,
            segments,
        );
    }
    let last_ring = 1 + (profile_ring_count - 1) * segments;
    for segment in 0..segments {
        let next = (segment + 1) % segments;
        indices.extend_from_slice(&[bottom, last_ring + segment, last_ring + next]);
    }
    Mesh::new(vertices, indices)
}

fn torus_mesh(
    major_radius: f32,
    minor_radius: f32,
    major_segments: u32,
    minor_segments: u32,
    color: Color,
) -> Result<Mesh, GeometryError> {
    let mut vertices = Vec::with_capacity((major_segments * minor_segments) as usize);
    for major in 0..major_segments {
        let major_angle = TAU * major as f32 / major_segments as f32;
        let (major_sin, major_cos) = major_angle.sin_cos();
        for minor in 0..minor_segments {
            let minor_angle = TAU * minor as f32 / minor_segments as f32;
            let (minor_sin, minor_cos) = minor_angle.sin_cos();
            let radial = major_radius + minor_radius * minor_cos;
            vertices.push(vertex(
                Vec3::new(
                    radial * major_cos,
                    minor_radius * minor_sin,
                    radial * major_sin,
                ),
                color,
            ));
        }
    }
    let mut indices = Vec::with_capacity((6 * major_segments * minor_segments) as usize);
    for major in 0..major_segments {
        let next_major = (major + 1) % major_segments;
        for minor in 0..minor_segments {
            let next_minor = (minor + 1) % minor_segments;
            let a = major * minor_segments + minor;
            let b = major * minor_segments + next_minor;
            let c = next_major * minor_segments + minor;
            let d = next_major * minor_segments + next_minor;
            indices.extend_from_slice(&[a, c, d, a, d, b]);
        }
    }
    Mesh::new(vertices, indices)
}

fn push_ring(vertices: &mut Vec<MeshVertex>, y: f32, radius: f32, segments: u32, color: Color) {
    for segment in 0..segments {
        let angle = TAU * segment as f32 / segments as f32;
        let (sin, cos) = angle.sin_cos();
        vertices.push(vertex(Vec3::new(radius * cos, y, radius * sin), color));
    }
}

fn connect_rings(indices: &mut Vec<u32>, lower: u32, upper: u32, segments: u32) {
    for segment in 0..segments {
        let next = (segment + 1) % segments;
        let a = lower + segment;
        let b = lower + next;
        let c = upper + segment;
        let d = upper + next;
        indices.extend_from_slice(&[a, c, d, a, d, b]);
    }
}

fn validate_dimensions(values: &[f32]) -> Result<(), GeometryError> {
    if values
        .iter()
        .any(|value| !value.is_finite() || *value < MIN_DIMENSION)
    {
        return Err(GeometryError::InvalidDimensions);
    }
    Ok(())
}

fn validate_subdivision(
    field: &'static str,
    value: u32,
    minimum: u32,
) -> Result<(), GeometryError> {
    if !(minimum..=MAX_SUBDIVISIONS).contains(&value) {
        return Err(GeometryError::InvalidSubdivision {
            field,
            found: value,
            minimum,
            maximum: MAX_SUBDIVISIONS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_all_primitive_families_as_valid_meshes() {
        let primitives = [
            Primitive::plane(1.0, 0.75, Color::WHITE).unwrap(),
            Primitive::cube(1.0, Color::RED).unwrap(),
            Primitive::pyramid(1.0, 2.0, Color::YELLOW).unwrap(),
            Primitive::sphere(1.0, 16, 8, Color::GREEN).unwrap(),
            Primitive::cylinder(1.0, 1.0, 16, Color::BLUE).unwrap(),
            Primitive::cone(1.0, 2.0, 16, Color::CYAN).unwrap(),
            Primitive::capsule(0.6, 0.8, 16, 4, Color::MAGENTA).unwrap(),
            Primitive::torus(1.0, 0.3, 16, 8, Color::WHITE).unwrap(),
        ];
        let expected = [
            PrimitiveKind::Plane,
            PrimitiveKind::Cuboid,
            PrimitiveKind::Pyramid,
            PrimitiveKind::Sphere,
            PrimitiveKind::Cylinder,
            PrimitiveKind::Cone,
            PrimitiveKind::Capsule,
            PrimitiveKind::Torus,
        ];
        for (primitive, expected_kind) in primitives.iter().zip(expected) {
            assert_eq!(primitive.kind(), expected_kind);
            let mesh = primitive
                .to_mesh()
                .expect("primitive produces a valid mesh");
            assert!(mesh.vertices().len() >= 3);
            assert!(!mesh.indices().is_empty());
            assert!(mesh.indices().len().is_multiple_of(3));
        }
    }

    #[test]
    fn generated_topology_counts_are_predictable() {
        let sphere = Primitive::sphere(1.0, 12, 6, Color::WHITE)
            .unwrap()
            .to_mesh()
            .unwrap();
        assert_eq!(sphere.vertices().len(), 2 + 5 * 12);
        assert_eq!(sphere.indices().len(), 6 * 12 * 5);

        let torus = Primitive::torus(2.0, 0.5, 12, 7, Color::WHITE)
            .unwrap()
            .to_mesh()
            .unwrap();
        assert_eq!(torus.vertices().len(), 12 * 7);
        assert_eq!(torus.indices().len(), 6 * 12 * 7);
    }

    #[test]
    fn rejects_invalid_dimensions_subdivisions_and_torus_radii() {
        assert_eq!(
            Primitive::plane(0.0, 1.0, Color::WHITE),
            Err(GeometryError::InvalidDimensions)
        );
        assert_eq!(
            Primitive::sphere(1.0, 2, 4, Color::WHITE),
            Err(GeometryError::InvalidSubdivision {
                field: "sphere sectors",
                found: 2,
                minimum: 3,
                maximum: 512,
            })
        );
        assert_eq!(
            Primitive::capsule(1.0, 0.0, 12, 4, Color::WHITE),
            Err(GeometryError::InvalidDimensions)
        );
        assert_eq!(
            Primitive::torus(0.5, 0.5, 12, 8, Color::WHITE),
            Err(GeometryError::InvalidTorusRadii)
        );
    }
}
