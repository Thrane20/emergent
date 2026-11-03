//! `--map-layout <scene>`: what the scene map editor draws underneath its items. Every
//! instance's world matrix, plus the outline seen from above (XZ convex hull) of anything
//! with geometry or terrain bounds.
use std::error::Error;

use crate::component::ComponentRegistry;
use crate::{Geometry, Project, SceneDefinition, Vec3, World};
use serde::Serialize;

/// Points kept per outline; denser hulls are thinned evenly.
const OUTLINE_POINTS: usize = 32;
/// Vertices considered per mesh; larger meshes (terrain) are sampled evenly.
const SAMPLE_VERTICES: usize = 50_000;

#[derive(Serialize)]
struct Instance {
    id: String,
    /// World matrix, column-major.
    matrix: Vec<f32>,
    /// XZ outline in world space, counter-clockwise; empty when the object has no shape.
    outline: Vec<[f32; 2]>,
    /// Highest point of the outline's geometry, for ordering and labels.
    #[serde(skip_serializing_if = "Option::is_none")]
    top: Option<f32>,
}

#[derive(Serialize)]
struct Layout {
    instances: Vec<Instance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind: Option<WindView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    airports: Vec<AirportView>,
}

#[derive(Serialize)]
struct WindView {
    speed: f32,
    /// Compass degrees the wind blows from.
    from: f32,
}

/// An airport for the map overlay: points are world [x, z].
#[derive(Serialize)]
struct AirportView {
    #[serde(skip_serializing_if = "Option::is_none")]
    aerodrome: Option<String>,
    towered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    holding_fix: Option<[f32; 2]>,
    runways: Vec<RunwayView>,
}

#[derive(Serialize)]
struct RunwayView {
    id: String,
    /// Compass degrees of the landing direction in use (or the forward one when not in use).
    heading: f32,
    active: bool,
    right_circuit: bool,
    threshold: [f32; 2],
    far_end: [f32; 2],
    /// Active runway only: final turn, threshold, far end, entry, base turn, final turn.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    circuit: Vec<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern_altitude: Option<f32>,
}

fn round(value: f32) -> f32 {
    (value * 1000.0).round() / 1000.0
}

/// Andrew's monotone chain over XZ points.
fn hull(mut points: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut lower: Vec<[f32; 2]> = Vec::new();
    for &point in &points {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], point) <= 0.0
        {
            lower.pop();
        }
        lower.push(point);
    }
    let mut upper: Vec<[f32; 2]> = Vec::new();
    for &point in points.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], point) <= 0.0
        {
            upper.pop();
        }
        upper.push(point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

pub(super) fn layout(
    project: &Project,
    registry: &ComponentRegistry,
    path: &str,
) -> Result<(), Box<dyn Error>> {
    let mut world = World::new();
    let scene = SceneDefinition::load_with(project, path, registry)?.instantiate(&mut world)?;
    world.propagate_transforms();
    let ids_by_entity: std::collections::HashMap<crate::EntityId, String> = scene
        .entities
        .iter()
        .map(|(name, &id)| (id, name.clone()))
        .collect();
    let mut ids: Vec<_> = scene.entities.iter().collect();
    ids.sort_by(|a, b| a.0.cmp(b.0));
    let mut instances = Vec::with_capacity(ids.len());
    for (name, &entity) in ids {
        let matrix = world.global_transform(entity)?.matrix();
        let mut local: Vec<Vec3> = Vec::new();
        if let Some((_, geometry)) = scene.renderables.iter().find(|(id, _)| *id == entity) {
            match geometry {
                Geometry::Mesh(mesh) => local.extend(mesh.vertices().iter().map(|v| v.position())),
                Geometry::Primitive(primitive) => {
                    if let Ok(mesh) = primitive.to_mesh() {
                        local.extend(mesh.vertices().iter().map(|v| v.position()));
                    }
                }
                Geometry::Polygon(polygon) => local.extend_from_slice(polygon.vertices()),
                Geometry::Point(_) | Geometry::Line(_) => {}
            }
        }
        if let Some(field) = world.component::<crate::RaymarchedTerrain>(entity)? {
            let (lower, upper) = field.bounds();
            for corner in 0..8 {
                local.push(Vec3::new(
                    if corner & 1 == 0 { lower.x } else { upper.x },
                    if corner & 2 == 0 { lower.y } else { upper.y },
                    if corner & 4 == 0 { lower.z } else { upper.z },
                ));
            }
        }
        let step = local.len().div_ceil(SAMPLE_VERTICES).max(1);
        let points: Vec<Vec3> = local
            .iter()
            .step_by(step)
            .map(|&point| matrix.transform_point3(point))
            .filter(|point| point.is_finite())
            .collect();
        let top = points.iter().map(|p| p.y).reduce(f32::max).map(round);
        let mut outline = hull(points.iter().map(|p| [p.x, p.z]).collect());
        if outline.len() > OUTLINE_POINTS {
            let stride = outline.len() as f32 / OUTLINE_POINTS as f32;
            outline = (0..OUTLINE_POINTS)
                .map(|i| outline[(i as f32 * stride) as usize])
                .collect();
        }
        instances.push(Instance {
            id: name.clone(),
            matrix: matrix.to_cols_array().map(round).to_vec(),
            outline: outline.into_iter().map(|p| p.map(round)).collect(),
            top,
        });
    }
    let wind = world.resource::<crate::Wind>().map(|wind| {
        let (speed, from) = wind.compass();
        WindView {
            speed: round(speed),
            from: round(from),
        }
    });
    print!(
        "{}",
        toml::to_string(&Layout {
            instances,
            wind,
            airports: airport_views(&world, &ids_by_entity)?
        })?
    );
    Ok(())
}

/// Airports with the circuit a default `fixed_wing_physics` aircraft would fly.
fn airport_views(
    world: &World,
    names: &std::collections::HashMap<crate::EntityId, String>,
) -> Result<Vec<AirportView>, Box<dyn Error>> {
    let profile = crate::LocomotionProfile::from_source(
        "map.locomotion.toml",
        "schema_version=1\nid='map'\nkind='locomotion'\ntype='fixed_wing_physics'\n",
    );
    let Ok(crate::LocomotionProfile {
        model: crate::locomotion::Model::FixedWingPhysics(params),
        ..
    }) = profile
    else {
        // Without the physics feature there are no physical aircraft to fly a circuit.
        return Ok(Vec::new());
    };
    let xz = |p: Vec3| [round(p.x), round(p.z)];
    Ok(crate::tower::airport_views(world, &params)
        .into_iter()
        .map(|airport| AirportView {
            aerodrome: airport.aerodrome.and_then(|id| names.get(&id).cloned()),
            towered: airport.towered,
            holding_fix: airport.holding_fix.map(xz),
            runways: airport
                .runways
                .iter()
                .filter_map(|view| {
                    let direction = crate::fixed_wing::bearing_direction(view.runway.heading);
                    let threshold = view.runway.center - direction * (view.runway.length / 2.0);
                    Some(RunwayView {
                        id: names.get(&view.zone)?.clone(),
                        heading: round(view.runway.heading.to_degrees()),
                        active: view.active,
                        right_circuit: view.runway.right_circuit,
                        threshold: xz(threshold),
                        far_end: xz(threshold + direction * view.runway.length),
                        circuit: view.circuit.map_or(Vec::new(), |c| {
                            [
                                c.final_turn,
                                c.threshold,
                                c.far_end,
                                c.entry,
                                c.base_turn,
                                c.final_turn,
                            ]
                            .map(xz)
                            .to_vec()
                        }),
                        pattern_altitude: view.circuit.map(|c| round(c.pattern_altitude)),
                    })
                })
                .collect(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn hull_keeps_the_outer_points_counter_clockwise() {
        let square = super::hull(vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.5, 0.5],
            [1.0, 0.0],
        ]);
        assert_eq!(square, vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    }
}
