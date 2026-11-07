//! `--map-layout <scene-or-prefab>`: what the map editor draws underneath its items. Every
//! instance's world matrix, plus the outline seen from above (XZ convex hull) of anything
//! with geometry or terrain bounds.
use std::error::Error;

use crate::component::ComponentRegistry;
use crate::{Geometry, PrefabDefinition, Project, SceneDefinition, Vec3, World};
use serde::Serialize;

/// Points kept per outline; denser hulls are thinned evenly.
const OUTLINE_POINTS: usize = 32;
/// Vertices considered per mesh; larger meshes (terrain) are sampled evenly.
const SAMPLE_VERTICES: usize = 50_000;
/// Longest side of the top-down mesh image, in pixels.
const TOP_DOWN_PIXELS: usize = 1024;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    top_down: Option<TopDown>,
}

/// Every mesh triangle seen from above, height-shaded, covering world XZ `min`..`max`
/// (image row 0 is `min[1]`, the north edge).
#[derive(Serialize)]
struct TopDown {
    min: [f32; 2],
    max: [f32; 2],
    /// `data:image/png;base64,...`
    image: String,
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
    let scene = if path.ends_with(".prefab.toml") {
        PrefabDefinition::load_with(project, path, registry)?.instantiate(&mut world)?
    } else {
        SceneDefinition::load_with(project, path, registry)?.instantiate(&mut world)?
    };
    world.propagate_transforms();
    let ids_by_entity: std::collections::HashMap<crate::EntityId, String> = scene
        .entities
        .iter()
        .map(|(name, &id)| (id, name.clone()))
        .collect();
    let mut ids: Vec<_> = scene.entities.iter().collect();
    ids.sort_by(|a, b| a.0.cmp(b.0));
    let mut instances = Vec::with_capacity(ids.len());
    let mut triangles: Vec<[Vec3; 3]> = Vec::new();
    for (name, &entity) in ids {
        let matrix = world.global_transform(entity)?.matrix();
        let mut local: Vec<Vec3> = Vec::new();
        let mut add_mesh =
            |mesh: &crate::Mesh, local: &mut Vec<Vec3>| {
                let vertices = mesh.vertices();
                local.extend(vertices.iter().map(|v| v.position()));
                triangles.extend(
                    mesh.indices().as_chunks::<3>().0.iter().map(|t| {
                        t.map(|i| matrix.transform_point3(vertices[i as usize].position()))
                    }),
                );
            };
        if let Some((_, geometry)) = scene.renderables.iter().find(|(id, _)| *id == entity) {
            match geometry {
                Geometry::Mesh(mesh) => add_mesh(mesh, &mut local),
                Geometry::Primitive(primitive) => {
                    if let Ok(mesh) = primitive.to_mesh() {
                        add_mesh(&mesh, &mut local);
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
    out!(
        "{}",
        toml::to_string(&Layout {
            instances,
            wind,
            airports: airport_views(&world, &ids_by_entity)?,
            top_down: top_down(&triangles)?,
        })?
    );
    Ok(())
}

/// Rasterises triangles into a top-down height map (highest surface per pixel), then shades
/// it by height and by a north-west light so roofs, edges and separate meshes stand apart.
fn top_down(triangles: &[[Vec3; 3]]) -> Result<Option<TopDown>, Box<dyn Error>> {
    let finite = |t: &&[Vec3; 3]| t.iter().all(|p| p.is_finite());
    let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for t in triangles.iter().filter(finite) {
        for p in t {
            min = min.min(*p);
            max = max.max(*p);
        }
    }
    let span = (max.x - min.x).max(max.z - min.z);
    if !span.is_finite() || span <= 0.0 {
        return Ok(None);
    }
    let cell = span / TOP_DOWN_PIXELS as f32;
    let width = (((max.x - min.x) / cell).ceil() as usize).clamp(1, TOP_DOWN_PIXELS);
    let height = (((max.z - min.z) / cell).ceil() as usize).clamp(1, TOP_DOWN_PIXELS);
    let mut heights = vec![f32::NEG_INFINITY; width * height];
    for t in triangles.iter().filter(finite) {
        // Pixel-space XZ; walls seen edge-on have no area and are skipped.
        let p = t.map(|v| [(v.x - min.x) / cell, (v.z - min.z) / cell]);
        let area =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
        if area.abs() < 1e-9 {
            continue;
        }
        let lo = |axis: usize, n: usize| {
            (p.iter()
                .map(|v| v[axis])
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.0) as usize)
                .min(n - 1)
        };
        let hi = |axis: usize, n: usize| {
            (p.iter()
                .map(|v| v[axis])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .max(0.0) as usize)
                .min(n - 1)
        };
        for row in lo(1, height)..=hi(1, height) {
            for col in lo(0, width)..=hi(0, width) {
                let (x, z) = (col as f32 + 0.5, row as f32 + 0.5);
                let w0 = ((p[1][0] - x) * (p[2][1] - z) - (p[1][1] - z) * (p[2][0] - x)) / area;
                let w1 = ((p[2][0] - x) * (p[0][1] - z) - (p[2][1] - z) * (p[0][0] - x)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let y = w0 * t[0].y + w1 * t[1].y + w2 * t[2].y;
                let slot = &mut heights[row * width + col];
                *slot = slot.max(y);
            }
        }
    }
    let rise = (max.y - min.y).max(1e-3);
    let mut pixels = vec![0u8; width * height * 4];
    for row in 0..height {
        for col in 0..width {
            let h = heights[row * width + col];
            if h == f32::NEG_INFINITY {
                continue;
            }
            let at = |c: usize, r: usize| {
                Some(heights[r * width + c])
                    .filter(|v| v.is_finite())
                    .unwrap_or(h)
            };
            // Light from the north-west: surfaces rising towards it are lit, the far sides of edges shaded.
            let slope = (at(col.saturating_sub(1), row) - h) + (at(col, row.saturating_sub(1)) - h);
            let light = 0.5 - 0.5 * (slope / cell).tanh();
            let value = 0.25 + 0.45 * (h - min.y) / rise + 0.3 * light;
            let i = (row * width + col) * 4;
            pixels[i..i + 4].copy_from_slice(&[
                (value * 205.0) as u8,
                (value * 220.0) as u8,
                (value * 205.0) as u8,
                235,
            ]);
        }
    }
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_raw(width as u32, height as u32, pixels)
        .ok_or("top-down image size")?
        .write_to(&mut png, image::ImageFormat::Png)?;
    Ok(Some(TopDown {
        min: [round(min.x), round(min.z)],
        max: [
            round(min.x + width as f32 * cell),
            round(min.z + height as f32 * cell),
        ],
        image: format!("data:image/png;base64,{}", base64(png.get_ref())),
    }))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                TABLE[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/// Airports with the circuit the scene's aircraft would fly: the widest-turning physical aircraft
/// in the scene (the room the airport needs), or a default one when the scene has none.
fn airport_views(
    world: &World,
    names: &std::collections::HashMap<crate::EntityId, String>,
) -> Result<Vec<AirportView>, Box<dyn Error>> {
    let scene_aircraft = names
        .keys()
        .filter_map(|&id| match world.component::<crate::LocomotionAgent>(id) {
            Ok(Some(agent)) => match &agent.model {
                crate::locomotion::Model::FixedWingPhysics(params) => Some(*params),
                _ => None,
            },
            _ => None,
        })
        .max_by(|a, b| a.liftoff_speed.total_cmp(&b.liftoff_speed));
    let params = match scene_aircraft {
        Some(params) => params,
        None => {
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
            params
        }
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

    /// The overlay's circuit is the one the scene's aircraft would fly: a slow aircraft's is tighter.
    #[cfg(feature = "physics")]
    #[test]
    fn the_airport_overlay_uses_the_scenes_aircraft() {
        let root =
            std::env::temp_dir().join(format!("emergent-map-aircraft-{}", std::process::id()));
        for dir in ["assets/locomotion", "assets/scenes", "src/generated"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        let write = |path: &str, text: &str| std::fs::write(root.join(path), text).unwrap();
        write(
            "emergent.toml",
            include_str!("../../templates/standard/emergent.toml"),
        );
        write(
            "assets/point.entity.toml",
            "schema_version=1\nid='point'\nkind='entity'\n",
        );
        write("assets/cube.entity.toml", "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n");
        write("assets/locomotion/mini.locomotion.toml", "schema_version=1\nid='mini'\nkind='locomotion'\ntype='fixed_wing_physics'\nliftoff_speed=6\ncruise_speed=9\nmax_speed=14\n");
        let airfield = "schema_version=1\nid='main'\n[[entities]]\nid='field'\nasset='assets/point.entity.toml'\n[[entities.components]]\ntype='emergent.zone'\nshape='circle'\nradius=300\n[[entities.components]]\ntype='emergent.metadata'\nvalues={aeroway='aerodrome',scale=0.1}\n[[entities]]\nid='runway'\nasset='assets/point.entity.toml'\n[[entities.components]]\ntype='emergent.zone'\nshape='rect'\nwidth=8\ndepth=90\n[[entities.components]]\ntype='emergent.metadata'\nvalues={aeroway='runway'}\n";
        let plane = "[[entities]]\nid='plane'\nasset='assets/cube.entity.toml'\n[entities.physics]\nbody='dynamic'\nmass=20\nfriction=0\ncollider={shape='box',half_extents=[1.2,0.25,1]}\n[[entities.components]]\ntype='emergent.locomotion'\nprofile='assets/locomotion/mini.locomotion.toml'\n";
        let base_turn = |scene: String| {
            write("assets/scenes/main.scene.toml", &scene);
            let project = crate::Project::load(&root).unwrap();
            let mut world = crate::World::new();
            let entities = crate::SceneDefinition::load(&project, "assets/scenes/main.scene.toml")
                .unwrap()
                .instantiate(&mut world)
                .unwrap()
                .entities;
            world.propagate_transforms();
            let names = entities.into_iter().map(|(name, id)| (id, name)).collect();
            let views = super::airport_views(&world, &names).unwrap();
            let circuit = views[0].runways[0].circuit.clone();
            circuit[4][0].abs()
        };
        let default_aircraft = base_turn(airfield.to_string());
        let slow_aircraft = base_turn(format!("{airfield}{plane}"));
        let _ = std::fs::remove_dir_all(&root);
        // Scaled to 0.1 the default aircraft's circuit is still set by its ~240 m turns.
        assert!(default_aircraft > 500.0, "default {default_aircraft}");
        assert!(slow_aircraft < 100.0, "slow aircraft {slow_aircraft}");
    }

    #[test]
    fn base64_pads_like_the_standard() {
        assert_eq!(super::base64(b"M"), "TQ==");
        assert_eq!(super::base64(b"Ma"), "TWE=");
        assert_eq!(super::base64(b"Man"), "TWFu");
    }

    #[test]
    fn top_down_draws_the_highest_surface_inside_its_bounds() {
        use crate::Vec3;
        let quad = |y: f32, x0: f32, x1: f32| {
            let (a, b, c, d) = (
                Vec3::new(x0, y, 0.0),
                Vec3::new(x1, y, 0.0),
                Vec3::new(x1, y, 4.0),
                Vec3::new(x0, y, 4.0),
            );
            [[a, b, c], [a, c, d]]
        };
        let mut triangles = quad(0.0, 0.0, 4.0).to_vec();
        triangles.extend(quad(2.0, 1.0, 2.0));
        let view = super::top_down(&triangles).unwrap().unwrap();
        assert_eq!((view.min, view.max), ([0.0, 0.0], [4.0, 4.0]));
        assert!(view.image.starts_with("data:image/png;base64,iVBOR"));
        assert!(super::top_down(&[]).unwrap().is_none());
    }
}
