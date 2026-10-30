//! Bounded, blocking preparation of a triangle approximation of an implicit field.
use crate::{Color, GeometryError, Mesh, MeshVertex, RaymarchedTerrain, Vec3};
use fast_surface_nets::{
    ndshape::{RuntimeShape, Shape},
    surface_nets, SurfaceNetsBuffer,
};
use std::time::{Duration, Instant};
pub const DENSITY_MAX_SAMPLES: usize = 4_000_000;
const MAX_SURFACE_CELLS: usize = 250_000;

#[derive(Debug)]
pub enum DensityMeshError {
    InvalidSettings(&'static str),
    Geometry(GeometryError),
}
impl std::fmt::Display for DensityMeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSettings(message) => f.write_str(message),
            Self::Geometry(error) => write!(f, "Density mesh: {error}"),
        }
    }
}
impl std::error::Error for DensityMeshError {}
impl From<GeometryError> for DensityMeshError {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

/// Prepared local-space geometry. This does not attach collision or own runtime identity.
pub struct DensityMesh {
    pub mesh: Option<Mesh>,
    pub samples: usize,
    pub dimensions: [u32; 3],
    /// Sum of retained sample, extractor and converted-vertex buffer capacities at conversion.
    /// Excludes allocator/driver overhead; not a process memory measurement.
    pub counted_buffer_bytes: usize,
    pub sampling_time: Duration,
    pub extraction_time: Duration,
    pub conversion_time: Duration,
}
impl RaymarchedTerrain {
    /// Extracts a Surface Nets approximation of the full field, with exterior padding.
    /// Blocking experimental preparation; metre spacing must be 0.125..=64 and fit sample/output caps.
    /// Output may be nonmanifold or miss sub-cell features; not a validated physics collider.
    pub fn extract_mesh(&self, cell_size: f32) -> Result<DensityMesh, DensityMeshError> {
        self.validate().map_err(DensityMeshError::InvalidSettings)?;
        let (mut lower, upper) = self.bounds();
        // Legacy tracing clips the lower sphere/torus; extraction must enclose their actual field.
        if self.version == 1 {
            lower.y = -self.size * 0.1875;
        }
        extract(lower, upper, cell_size, |point| self.distance(point))
    }
}

fn extract(
    lower: Vec3,
    upper: Vec3,
    step: f32,
    field: impl Fn(Vec3) -> Result<f32, &'static str>,
) -> Result<DensityMesh, DensityMeshError> {
    let bad = DensityMeshError::InvalidSettings;
    if !step.is_finite()
        || !(0.125..=64.).contains(&step)
        || !lower.is_finite()
        || !upper.is_finite()
        || !upper.cmpgt(lower).all()
    {
        return Err(bad(
            "Density mesh requires finite ordered bounds and 0.125–64m spacing",
        ));
    }
    let cells = ((upper - lower) / step).ceil();
    if !cells.is_finite() || cells.max_element() > DENSITY_MAX_SAMPLES as f32 {
        return Err(bad("Density dimensions exceed the sampling budget"));
    }
    let dimensions = cells.to_array().map(|n| n as u32 + 4);
    let samples = dimensions
        .iter()
        .try_fold(1usize, |total, &n| total.checked_mul(n as usize))
        .filter(|&n| n <= DENSITY_MAX_SAMPLES)
        .ok_or(bad(
            "Density mesh exceeds four million samples; use a coarser spacing or smaller field",
        ))?;
    let shape = RuntimeShape::<u32, 3>::new(dimensions);
    // Avoid putting the clipped box exactly on zero-valued grid planes.
    let origin = lower - Vec3::splat(step * 1.25);
    let start = Instant::now();
    let mut values = Vec::with_capacity(samples);
    for index in 0..samples as u32 {
        let point = origin + Vec3::from_array(shape.delinearize(index).map(|n| n as f32)) * step;
        let value = field(point).map_err(bad)?;
        if !value.is_finite() {
            return Err(bad("Density samples must be finite"));
        }
        values.push(value);
    }
    let sampling_time = start.elapsed();
    let start = Instant::now();
    // Bound output before the dependency can allocate its surface buffers.
    let mut active = 0usize;
    for z in 0..dimensions[2] - 1 {
        for y in 0..dimensions[1] - 1 {
            for x in 0..dimensions[0] - 1 {
                let negative = values[shape.linearize([x, y, z]) as usize] < 0.;
                if (0..8).any(|i| {
                    (values[shape.linearize([x + (i & 1), y + ((i >> 1) & 1), z + ((i >> 2) & 1)])
                        as usize]
                        < 0.)
                        != negative
                }) {
                    active += 1;
                }
                if active > MAX_SURFACE_CELLS {
                    return Err(bad(
                        "Density mesh exceeds 250,000 surface cells; use a coarser spacing",
                    ));
                }
            }
        }
    }
    let mut output = SurfaceNetsBuffer::default();
    if active > 0 {
        surface_nets(
            &values,
            &shape,
            [0; 3],
            dimensions.map(|n| n - 1),
            &mut output,
        );
    }
    let extraction_time = start.elapsed();
    let start = Instant::now();
    let mut vertices = Vec::with_capacity(output.positions.len());
    for (&position, &normal) in output.positions.iter().zip(&output.normals) {
        vertices.push(MeshVertex::with_normal(
            origin + Vec3::from_array(position) * step,
            Vec3::from_array(normal),
            Color::WHITE,
        )?);
    }
    let counted_buffer_bytes = values.capacity() * 4
        + (output.positions.capacity()
            + output.normals.capacity()
            + output.surface_points.capacity())
            * 12
        + (output.indices.capacity()
            + output.surface_strides.capacity()
            + output.stride_to_index.capacity())
            * 4
        + vertices.capacity() * std::mem::size_of::<MeshVertex>();
    let mesh = if output.indices.is_empty() {
        None
    } else {
        Some(Mesh::new(vertices, output.indices)?)
    };
    Ok(DensityMesh {
        mesh,
        samples,
        dimensions,
        counted_buffer_bytes,
        sampling_time,
        extraction_time,
        conversion_time: start.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn validates_before_sampling_and_handles_empty_full() {
        for step in [f32::NAN, 0., 0.01, 65.] {
            assert!(extract(-Vec3::ONE, Vec3::ONE, step, |_| panic!(
                "must validate first"
            ))
            .is_err());
        }
        assert!(RaymarchedTerrain::default().extract_mesh(0.125).is_err());
        for value in [-1., 1.] {
            assert!(extract(-Vec3::ONE, Vec3::ONE, 0.5, |_| Ok(value))
                .unwrap()
                .mesh
                .is_none());
        }
        assert!(extract(-Vec3::ONE, Vec3::ONE, 0.5, |_| Ok(f32::NAN)).is_err());
        assert!(matches!(
            extract(Vec3::ZERO, Vec3::splat(80.), 1., |p| Ok(
                if (p.x as i32 + p.y as i32 + p.z as i32) & 1 == 0 {
                    -1.
                } else {
                    1.
                }
            )),
            Err(DensityMeshError::InvalidSettings(
                "Density mesh exceeds 250,000 surface cells; use a coarser spacing"
            ))
        ));
    }
    #[test]
    fn sphere_is_deterministic_closed_and_outward() {
        let prepare = || {
            extract(Vec3::splat(-2.), Vec3::splat(2.), 0.25, |p| {
                Ok(p.length() - 1.37)
            })
            .unwrap()
        };
        let mesh = prepare().mesh.unwrap();
        assert_eq!(Some(mesh.clone()), prepare().mesh);
        let mut edges = HashMap::<(u32, u32), (usize, i32)>::new();
        for triangle in mesh.indices().as_chunks::<3>().0 {
            let [a, b, c] =
                std::array::from_fn(|i| mesh.vertices()[triangle[i] as usize].position());
            assert!((b - a).cross(c - a).dot((a + b + c) / 3.) > 0.);
            for (a, b) in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                let edge = edges.entry((a.min(b), a.max(b))).or_default();
                edge.0 += 1;
                edge.1 += if a < b { 1 } else { -1 };
            }
        }
        assert!(edges
            .values()
            .all(|&(uses, winding)| uses == 2 && winding == 0));
        for vertex in mesh.vertices() {
            assert!(vertex.normal().unwrap().is_finite());
            assert!((vertex.position().length() - 1.37).abs() < 0.1);
        }
    }
}
