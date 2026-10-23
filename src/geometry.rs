use std::error::Error;
use std::fmt;

use glam::Vec3;

use crate::Primitive;

const GEOMETRY_EPSILON: f32 = 1.0e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    components: [f32; 4],
}

impl Color {
    pub const WHITE: Self = Self::from_validated([1.0, 1.0, 1.0, 1.0]);
    pub const RED: Self = Self::from_validated([1.0, 0.15, 0.12, 1.0]);
    pub const GREEN: Self = Self::from_validated([0.15, 1.0, 0.35, 1.0]);
    pub const BLUE: Self = Self::from_validated([0.15, 0.45, 1.0, 1.0]);
    pub const YELLOW: Self = Self::from_validated([1.0, 0.78, 0.15, 1.0]);
    pub const CYAN: Self = Self::from_validated([0.10, 0.90, 1.0, 1.0]);
    pub const MAGENTA: Self = Self::from_validated([0.90, 0.25, 1.0, 1.0]);

    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Self, GeometryError> {
        let components = [red, green, blue, alpha];
        if components.iter().any(|component| !component.is_finite()) {
            return Err(GeometryError::NonFiniteColor);
        }
        if red < 0.0 || green < 0.0 || blue < 0.0 || !(0.0..=1.0).contains(&alpha) {
            return Err(GeometryError::ColorOutOfRange);
        }
        Ok(Self::from_validated(components))
    }

    pub fn components(self) -> [f32; 4] {
        self.components
    }

    const fn from_validated(components: [f32; 4]) -> Self {
        Self { components }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    position: Vec3,
    radius: f32,
    color: Color,
}

impl Point {
    pub fn new(position: Vec3, radius: f32, color: Color) -> Result<Self, GeometryError> {
        validate_position(position, 0)?;
        validate_radius(radius)?;
        Ok(Self {
            position,
            radius,
            color,
        })
    }

    pub fn position(self) -> Vec3 {
        self.position
    }

    pub fn radius(self) -> f32 {
        self.radius
    }

    pub fn color(self) -> Color {
        self.color
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    start: Vec3,
    end: Vec3,
    radius: f32,
    color: Color,
}

impl Line {
    pub fn new(start: Vec3, end: Vec3, radius: f32, color: Color) -> Result<Self, GeometryError> {
        validate_position(start, 0)?;
        validate_position(end, 1)?;
        validate_radius(radius)?;
        if start.distance_squared(end) <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
            return Err(GeometryError::CoincidentLineEndpoints);
        }
        Ok(Self {
            start,
            end,
            radius,
            color,
        })
    }

    pub fn start(self) -> Vec3 {
        self.start
    }

    pub fn end(self) -> Vec3 {
        self.end
    }

    pub fn radius(self) -> f32 {
        self.radius
    }

    pub fn color(self) -> Color {
        self.color
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Polygon {
    vertices: Vec<Vec3>,
    color: Color,
}

impl Polygon {
    pub fn new(vertices: Vec<Vec3>, color: Color) -> Result<Self, GeometryError> {
        if vertices.len() < 3 {
            return Err(GeometryError::TooFewPolygonVertices);
        }
        if vertices.len() > u32::MAX as usize {
            return Err(GeometryError::GeometryTooLarge);
        }
        for (index, &vertex) in vertices.iter().enumerate() {
            validate_position(vertex, index)?;
        }

        let origin = vertices[0];
        let scale = vertices
            .iter()
            .map(|vertex| vertex.distance(origin))
            .fold(1.0_f32, f32::max);
        let normal = (1..vertices.len() - 1)
            .find_map(|index| {
                let cross = (vertices[index] - origin).cross(vertices[index + 1] - origin);
                (cross.length_squared() > GEOMETRY_EPSILON * GEOMETRY_EPSILON)
                    .then(|| cross.normalize())
            })
            .ok_or(GeometryError::DegeneratePolygon)?;

        let plane_tolerance = GEOMETRY_EPSILON * scale;
        if vertices
            .iter()
            .any(|vertex| (*vertex - origin).dot(normal).abs() > plane_tolerance)
        {
            return Err(GeometryError::NonPlanarPolygon);
        }

        let turn_tolerance = GEOMETRY_EPSILON * scale * scale;
        for index in 0..vertices.len() {
            let a = vertices[index];
            let b = vertices[(index + 1) % vertices.len()];
            let c = vertices[(index + 2) % vertices.len()];
            if (b - a).cross(c - b).dot(normal) <= turn_tolerance {
                return Err(GeometryError::NonConvexPolygon);
            }
        }

        Ok(Self { vertices, color })
    }

    pub fn vertices(&self) -> &[Vec3] {
        &self.vertices
    }

    pub fn color(&self) -> Color {
        self.color
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    position: Vec3,
    color: Color,
}

impl MeshVertex {
    pub fn new(position: Vec3, color: Color) -> Result<Self, GeometryError> {
        validate_position(position, 0)?;
        Ok(Self { position, color })
    }

    pub fn position(self) -> Vec3 {
        self.position
    }

    pub fn color(self) -> Color {
        self.color
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
}

impl Mesh {
    pub fn new(vertices: Vec<MeshVertex>, indices: Vec<u32>) -> Result<Self, GeometryError> {
        if vertices.len() < 3 {
            return Err(GeometryError::TooFewMeshVertices);
        }
        if vertices.len() > u32::MAX as usize || indices.len() > u32::MAX as usize {
            return Err(GeometryError::GeometryTooLarge);
        }
        if indices.is_empty() || !indices.len().is_multiple_of(3) {
            return Err(GeometryError::InvalidTriangleIndexCount);
        }
        for (position, &index) in indices.iter().enumerate() {
            if index as usize >= vertices.len() {
                return Err(GeometryError::MeshIndexOutOfBounds {
                    position,
                    index,
                    vertex_count: vertices.len(),
                });
            }
        }
        for (triangle, indices) in indices.as_chunks::<3>().0.iter().enumerate() {
            let a = vertices[indices[0] as usize].position;
            let b = vertices[indices[1] as usize].position;
            let c = vertices[indices[2] as usize].position;
            if (b - a).cross(c - a).length_squared() <= GEOMETRY_EPSILON * GEOMETRY_EPSILON {
                return Err(GeometryError::DegenerateMeshTriangle { triangle });
            }
        }
        Ok(Self { vertices, indices })
    }

    pub fn vertices(&self) -> &[MeshVertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn cube(half_extent: f32) -> Result<Self, GeometryError> {
        if !half_extent.is_finite() || half_extent <= 0.0 {
            return Err(GeometryError::InvalidExtent);
        }
        let h = half_extent;
        let positions = [
            Vec3::new(-h, -h, h),
            Vec3::new(h, -h, h),
            Vec3::new(h, h, h),
            Vec3::new(-h, h, h),
            Vec3::new(-h, -h, -h),
            Vec3::new(h, -h, -h),
            Vec3::new(h, h, -h),
            Vec3::new(-h, h, -h),
        ];
        let colors = [
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::YELLOW,
            Color::MAGENTA,
            Color::CYAN,
            Color::WHITE,
            Color::new(0.55, 0.70, 0.95, 1.0).expect("constant colour is valid"),
        ];
        let vertices = positions
            .into_iter()
            .zip(colors)
            .map(|(position, color)| MeshVertex { position, color })
            .collect();
        let indices = vec![
            0, 1, 2, 0, 2, 3, 1, 5, 6, 1, 6, 2, 5, 4, 7, 5, 7, 6, 4, 0, 3, 4, 3, 7, 3, 2, 6, 3, 6,
            7, 4, 5, 1, 4, 1, 0,
        ];
        Self::new(vertices, indices)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Point(Point),
    Line(Line),
    Polygon(Polygon),
    Mesh(Mesh),
    Primitive(Primitive),
}

impl From<Point> for Geometry {
    fn from(value: Point) -> Self {
        Self::Point(value)
    }
}

impl From<Line> for Geometry {
    fn from(value: Line) -> Self {
        Self::Line(value)
    }
}

impl From<Polygon> for Geometry {
    fn from(value: Polygon) -> Self {
        Self::Polygon(value)
    }
}

impl From<Mesh> for Geometry {
    fn from(value: Mesh) -> Self {
        Self::Mesh(value)
    }
}

impl From<Primitive> for Geometry {
    fn from(value: Primitive) -> Self {
        Self::Primitive(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeometryError {
    NonFiniteColor,
    ColorOutOfRange,
    NonFinitePosition {
        index: usize,
    },
    InvalidRadius,
    InvalidExtent,
    CoincidentLineEndpoints,
    TooFewPolygonVertices,
    DegeneratePolygon,
    NonPlanarPolygon,
    NonConvexPolygon,
    TooFewMeshVertices,
    GeometryTooLarge,
    InvalidDimensions,
    InvalidSubdivision {
        field: &'static str,
        found: u32,
        minimum: u32,
        maximum: u32,
    },
    InvalidTorusRadii,
    InvalidTriangleIndexCount,
    MeshIndexOutOfBounds {
        position: usize,
        index: u32,
        vertex_count: usize,
    },
    DegenerateMeshTriangle {
        triangle: usize,
    },
}

impl fmt::Display for GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteColor => formatter.write_str("colour components must be finite"),
            Self::ColorOutOfRange => formatter.write_str(
                "RGB colour components must be non-negative and alpha must be between zero and one",
            ),
            Self::NonFinitePosition { index } => {
                write!(formatter, "geometry position {index} must be finite")
            }
            Self::InvalidRadius => formatter.write_str("geometry radius must be finite and positive"),
            Self::InvalidExtent => formatter.write_str("mesh extent must be finite and positive"),
            Self::CoincidentLineEndpoints => {
                formatter.write_str("line endpoints must be distinct")
            }
            Self::TooFewPolygonVertices => {
                formatter.write_str("a polygon requires at least three vertices")
            }
            Self::DegeneratePolygon => formatter.write_str("polygon has no usable surface area"),
            Self::NonPlanarPolygon => formatter.write_str("polygon vertices must be planar"),
            Self::NonConvexPolygon => formatter.write_str(
                "polygon vertices must form a strictly convex ordered boundary",
            ),
            Self::TooFewMeshVertices => {
                formatter.write_str("a mesh requires at least three vertices")
            }
            Self::GeometryTooLarge => {
                formatter.write_str("geometry exceeds the renderer's 32-bit index limits")
            }
            Self::InvalidDimensions => formatter.write_str(
                "primitive dimensions must be finite and large enough to form usable triangles",
            ),
            Self::InvalidSubdivision {
                field,
                found,
                minimum,
                maximum,
            } => write!(
                formatter,
                "{field} is {found}; expected a value from {minimum} through {maximum}"
            ),
            Self::InvalidTorusRadii => {
                formatter.write_str("torus major radius must be greater than its minor radius")
            }
            Self::InvalidTriangleIndexCount => {
                formatter.write_str("mesh indices must contain one or more complete triangles")
            }
            Self::MeshIndexOutOfBounds {
                position,
                index,
                vertex_count,
            } => write!(
                formatter,
                "mesh index {position} refers to vertex {index}, but the mesh has {vertex_count} vertices"
            ),
            Self::DegenerateMeshTriangle { triangle } => {
                write!(formatter, "mesh triangle {triangle} has no usable surface area")
            }
        }
    }
}

impl Error for GeometryError {}

fn validate_position(position: Vec3, index: usize) -> Result<(), GeometryError> {
    if !position.is_finite() {
        return Err(GeometryError::NonFinitePosition { index });
    }
    Ok(())
}

fn validate_radius(radius: f32) -> Result<(), GeometryError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(GeometryError::InvalidRadius);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(position: Vec3) -> MeshVertex {
        MeshVertex::new(position, Color::WHITE).expect("test vertex is valid")
    }

    #[test]
    fn validates_point_line_and_colour_inputs() {
        assert_eq!(
            Color::new(f32::NAN, 0.0, 0.0, 1.0),
            Err(GeometryError::NonFiniteColor)
        );
        assert_eq!(
            Color::new(1.0, 1.0, 1.0, 1.1),
            Err(GeometryError::ColorOutOfRange)
        );
        assert_eq!(
            Point::new(Vec3::ZERO, 0.0, Color::WHITE),
            Err(GeometryError::InvalidRadius)
        );
        assert_eq!(
            Line::new(Vec3::ZERO, Vec3::ZERO, 0.1, Color::WHITE),
            Err(GeometryError::CoincidentLineEndpoints)
        );
        assert!(Line::new(Vec3::ZERO, Vec3::X, 0.1, Color::WHITE).is_ok());
    }

    #[test]
    fn accepts_convex_planar_polygons_and_rejects_unsupported_shapes() {
        assert!(Polygon::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y],
            Color::GREEN
        )
        .is_ok());
        assert_eq!(
            Polygon::new(
                vec![
                    Vec3::ZERO,
                    Vec3::X,
                    Vec3::X + Vec3::Y,
                    Vec3::new(0.0, 1.0, 0.1),
                ],
                Color::GREEN
            ),
            Err(GeometryError::NonPlanarPolygon)
        );
        assert_eq!(
            Polygon::new(
                vec![Vec3::ZERO, Vec3::X, Vec3::new(0.5, 0.2, 0.0), Vec3::Y,],
                Color::GREEN
            ),
            Err(GeometryError::NonConvexPolygon)
        );
    }

    #[test]
    fn validates_indexed_meshes() {
        let vertices = vec![vertex(Vec3::ZERO), vertex(Vec3::X), vertex(Vec3::Y)];
        assert!(Mesh::new(vertices.clone(), vec![0, 1, 2]).is_ok());
        assert_eq!(
            Mesh::new(vertices.clone(), vec![0, 1]),
            Err(GeometryError::InvalidTriangleIndexCount)
        );
        assert_eq!(
            Mesh::new(vertices.clone(), vec![0, 1, 3]),
            Err(GeometryError::MeshIndexOutOfBounds {
                position: 2,
                index: 3,
                vertex_count: 3,
            })
        );
        assert_eq!(
            Mesh::new(
                vec![vertex(Vec3::ZERO), vertex(Vec3::X), vertex(Vec3::X * 2.0)],
                vec![0, 1, 2]
            ),
            Err(GeometryError::DegenerateMeshTriangle { triangle: 0 })
        );
        assert!(Mesh::cube(1.0).is_ok());
    }
}
