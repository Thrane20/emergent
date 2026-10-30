//! Bounded surface extraction/render/collider experiment; no scene authoring changes.
use emergent::{
    CameraProjection, Color, Geometry, GeometryInstance, GeometryRenderer, LightInstance,
    LightSource, Mat4, Mesh, RaymarchedBiome, RaymarchedTerrain, Vec3,
};
use rapier3d::{
    math::{Pose, Vector},
    parry::query::Ray,
    prelude::ColliderBuilder,
};
use std::{collections::HashMap, path::PathBuf, time::Instant};

fn nearest_mesh_hit(mesh: &Mesh, origin: Vec3, direction: Vec3) -> Option<f32> {
    let d = glam::DVec3::from_array(direction.to_array().map(f64::from));
    let origin = glam::DVec3::from_array(origin.to_array().map(f64::from));
    let mut best = f64::INFINITY;
    for t in mesh.indices().as_chunks::<3>().0 {
        let [a, b, c] = std::array::from_fn(|i| {
            glam::DVec3::from_array(
                mesh.vertices()[t[i] as usize]
                    .position()
                    .to_array()
                    .map(f64::from),
            )
        });
        let e1 = b - a;
        let e2 = c - a;
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() <= 1e-12 {
            continue;
        }
        let delta = origin - a;
        let u = delta.dot(p) / det;
        if !(-1e-10..=1. + 1e-10).contains(&u) {
            continue;
        }
        let q = delta.cross(e1);
        let v = d.dot(q) / det;
        if v < -1e-10 || u + v > 1. + 1e-10 {
            continue;
        }
        let time = e2.dot(q) / det;
        if time >= 0. {
            best = best.min(time);
        }
    }
    best.is_finite().then_some(best as f32)
}

fn topology(mesh: &Mesh) -> (usize, usize, usize) {
    let mut edges = HashMap::<(u32, u32), (usize, i32)>::new();
    for t in mesh.indices().as_chunks::<3>().0 {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let edge = edges.entry((a.min(b), a.max(b))).or_default();
            edge.0 += 1;
            edge.1 += if a < b { 1 } else { -1 };
        }
    }
    (
        edges.values().filter(|e| e.0 == 1).count(),
        edges.values().filter(|e| e.0 > 2).count(),
        edges.values().filter(|e| e.0 == 2 && e.1 != 0).count(),
    )
}
fn camera(
    renderer: &mut GeometryRenderer,
    field: RaymarchedTerrain,
    cavern: bool,
    sample: usize,
) -> Result<(), emergent::GraphicsError> {
    let angle = 0.7 + sample as f32 / 32. * std::f32::consts::TAU;
    let target = Vec3::new(
        0.,
        if cavern {
            field.relief * 0.25
        } else if field.version == 1 {
            1.
        } else {
            field.relief * 0.3
        },
        0.,
    );
    let radius = if cavern { 20. } else { field.size * 0.85 };
    renderer.set_view(
        target
            + Vec3::new(
                angle.sin() * radius,
                if cavern { 0. } else { field.size * 0.55 },
                angle.cos() * radius,
            ),
        target,
        Vec3::Y,
    )
}
fn gpu(
    renderer: &mut GeometryRenderer,
    field: RaymarchedTerrain,
    cavern: bool,
    instances: &[GeometryInstance],
    lights: &[LightInstance],
    key: &str,
    rows: &mut String,
) -> Result<(f64, f64, f64), Box<dyn std::error::Error>> {
    for sample in 0..8 {
        camera(renderer, field, cavern, sample * 4)?;
        renderer.profile_lit_frame(instances, lights)?;
    }
    let mut times = Vec::new();
    for sample in 0..32 {
        camera(renderer, field, cavern, sample)?;
        let p = renderer
            .profile_lit_frame(instances, lights)?
            .ok_or("GPU timestamps unsupported")?;
        times.push(p.main_gpu_us);
        rows.push_str(&format!("{key},{sample},{:.3}\n", p.main_gpu_us));
    }
    times.sort_by(f64::total_cmp);
    Ok((times[15], times[30], times[31]))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(std::env::args().nth(1).ok_or("Output directory required")?);
    std::fs::create_dir_all(&output)?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(960, 540))?;
    if !renderer.gpu_timestamps_supported() {
        return Err("Comparison requires optional GPU pass timing support".into());
    }
    renderer.set_projection(CameraProjection::perspective(45., 0.1, 20000.)?);
    println!(
        "{} / {}; release 960x540, 8 warmups + 32 GPU views",
        renderer.info().adapter_name,
        renderer.info().backend
    );
    let lights = [
        LightInstance::new(LightSource::ambient(Color::WHITE, 0.3)?, Mat4::IDENTITY),
        LightInstance::new(
            LightSource::directional(Color::WHITE, 0.8)?,
            Mat4::from_rotation_x(-0.8),
        ),
    ];
    let mut gpu_rows = String::from("fixture_cell,representation,sample,main_gpu_us\n");
    let mut csv=String::from("fixture,cell_m,samples,vertices,triangles,working_buffer_bytes,uploaded_mesh_bytes,sampling_ms,extract_ms,convert_ms,collider_ms,open_edges,nonmanifold_edges,winding_edges,vertex_field_max_m,centroid_field_max_m,matched_rays,field_only_rays,mesh_only_rays,hit_error_p95_m,hit_error_max_m,ray_gpu_p50_us,ray_gpu_p95_us,ray_gpu_p99_us,mesh_gpu_p50_us,mesh_gpu_p95_us,mesh_gpu_p99_us,rapier_disagreements,rapier_max_delta_m\n");
    let legacy = RaymarchedTerrain {
        version: 1,
        size: 32.,
        ..Default::default()
    };
    let overhang = RaymarchedTerrain {
        size: 256.,
        overhang: 24.,
        cave_radius: 0.,
        ..Default::default()
    };
    let cavern = RaymarchedTerrain {
        size: 128.,
        biome: RaymarchedBiome::CaveCavern,
        cave_radius: 16.,
        ..Default::default()
    };
    for (name, field, step) in [
        ("legacy", legacy, 0.5),
        ("legacy", legacy, 0.25),
        ("overhang", overhang, 4.),
        ("overhang", overhang, 2.),
        ("cavern", cavern, 2.),
        ("cavern", cavern, 1.),
        ("kilometre", RaymarchedTerrain::default(), 8.),
    ] {
        let prepared = field.extract_mesh(step)?;
        let mesh = prepared.mesh.as_ref().ok_or("Fixture unexpectedly empty")?;
        let (open, nonmanifold, winding) = topology(mesh);
        let mut vertex_error = 0f32;
        let mut centroid_error = 0f32;
        for v in mesh.vertices() {
            vertex_error = vertex_error.max(field.distance(v.position())?.abs());
        }
        for t in mesh.indices().as_chunks::<3>().0 {
            let center = t
                .iter()
                .map(|&i| mesh.vertices()[i as usize].position())
                .sum::<Vec3>()
                / 3.;
            centroid_error = centroid_error.max(field.distance(center)?.abs());
        }
        let start = Instant::now();
        let collider = ColliderBuilder::trimesh(
            mesh.vertices()
                .iter()
                .map(|v| Vector::from_array(v.position().to_array()))
                .collect(),
            mesh.indices()
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| [t[0], t[1], t[2]])
                .collect(),
        )?
        .build();
        let collider_ms = start.elapsed().as_secs_f64() * 1000.;
        let mut matched = 0;
        let mut field_only = 0;
        let mut mesh_only = 0;
        let mut errors = Vec::<f32>::new();
        let mut rapier_disagreements = 0;
        let mut rapier_max_delta = 0f32;
        let (lower, upper) = field.bounds();
        let mut rays = Vec::new();
        for z in 0..13 {
            for x in 0..13 {
                rays.push((
                    Vec3::new(
                        lower.x + (x as f32 + 0.5) * field.size / 13.,
                        upper.y + step * 2.,
                        lower.z + (z as f32 + 0.5) * field.size / 13.,
                    ),
                    -Vec3::Y,
                ));
            }
        }
        let inside = if name == "legacy" {
            Vec3::new(7., 1., 0.)
        } else {
            Vec3::new(0., field.relief * 0.25, 0.)
        };
        for d in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
            rays.push((inside, d));
        }
        for (origin, direction) in rays {
            let reference = field.trace(origin, direction)?;
            let collider_hit = collider.shape().cast_ray(
                &Pose::IDENTITY,
                &Ray::new(
                    Vector::from_array(origin.to_array()),
                    Vector::from_array(direction.to_array()),
                ),
                20000.,
                false,
            );
            let hit = nearest_mesh_hit(mesh, origin, direction);
            match (hit, collider_hit) {
                (Some(a), Some(b)) => {
                    rapier_max_delta = rapier_max_delta.max((a - b).abs());
                    if (a - b).abs() > step * 0.05 {
                        rapier_disagreements += 1;
                    }
                }
                (None, None) => {}
                _ => rapier_disagreements += 1,
            }
            match (reference, hit) {
                (Some(a), Some(b)) => {
                    matched += 1;
                    errors.push((a - b).abs());
                }
                (Some(_), None) => field_only += 1,
                (None, Some(_)) => mesh_only += 1,
                _ => {}
            }
        }
        if matches!(name, "legacy" | "cavern") {
            // Distinct roof/floor/walls must be represented at the same X/Z.
            let mut chamber_hits = 0;
            for direction in [Vec3::Y, -Vec3::Y, Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z] {
                let Some(reference) = field.trace(inside, direction)? else {
                    continue;
                };
                let hit = collider
                    .shape()
                    .cast_ray(
                        &Pose::IDENTITY,
                        &Ray::new(
                            Vector::from_array(inside.to_array()),
                            Vector::from_array(direction.to_array()),
                        ),
                        20000.,
                        false,
                    )
                    .ok_or("Missing chamber roof/floor/wall")?;
                chamber_hits += 1;
                assert!(
                    (hit - reference).abs() < step * 2.,
                    "Chamber error exceeds two cells"
                );
            }
            assert!(
                chamber_hits >= 4,
                "Need roof, floor and at least two chamber walls"
            );
        }
        errors.sort_by(f32::total_cmp);
        let max = errors.last().copied().unwrap_or(0.);
        let p95 = errors[((errors.len() as f32 * 0.95).ceil() as usize).saturating_sub(1)];
        let ray = renderer.insert_raymarched_terrain(field)?;
        let ray_gpu = gpu(
            &mut renderer,
            field,
            name == "cavern",
            &[],
            &lights,
            &format!("{name}-{step},ray"),
            &mut gpu_rows,
        )?;
        camera(&mut renderer, field, name == "cavern", 0)?;
        let frame = renderer.render_lit_rgba(&[], &lights)?;
        image::save_buffer(
            output.join(format!("{name}-{step}-ray.png")),
            &frame.rgba,
            frame.width,
            frame.height,
            image::ColorType::Rgba8,
        )?;
        assert!(renderer.remove_raymarched_terrain(ray));
        let geometry = renderer.insert(&Geometry::from(mesh.clone()));
        let uploaded = renderer.geometry_buffer_bytes(geometry).unwrap();
        let instances = [GeometryInstance::new(geometry, Mat4::IDENTITY)];
        let mesh_gpu = gpu(
            &mut renderer,
            field,
            name == "cavern",
            &instances,
            &lights,
            &format!("{name}-{step},mesh"),
            &mut gpu_rows,
        )?;
        camera(&mut renderer, field, name == "cavern", 0)?;
        let frame = renderer.render_lit_rgba(&instances, &lights)?;
        image::save_buffer(
            output.join(format!("{name}-{step}-mesh.png")),
            &frame.rgba,
            frame.width,
            frame.height,
            image::ColorType::Rgba8,
        )?;
        assert!(renderer.remove(geometry));
        csv.push_str(&format!("{name},{step},{},{},{},{},{uploaded},{:.3},{:.3},{:.3},{collider_ms:.3},{open},{nonmanifold},{winding},{vertex_error:.6},{centroid_error:.6},{matched},{field_only},{mesh_only},{p95:.6},{max:.6},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{rapier_disagreements},{rapier_max_delta:.6}\n",prepared.samples,mesh.vertices().len(),mesh.indices().len()/3,prepared.counted_buffer_bytes,prepared.sampling_time.as_secs_f64()*1000.,prepared.extraction_time.as_secs_f64()*1000.,prepared.conversion_time.as_secs_f64()*1000.,ray_gpu.0,ray_gpu.1,ray_gpu.2,mesh_gpu.0,mesh_gpu.1,mesh_gpu.2));
        println!("{name} {step}m: {} triangles, topology {open}/{nonmanifold}/{winding}, rays {matched}/{field_only}/{mesh_only}, hit max {max:.3}m, Rapier deviations {rapier_disagreements}/{rapier_max_delta:.3}m; GPU ray {:.3}ms mesh {:.3}ms",mesh.indices().len()/3,ray_gpu.0/1000.,mesh_gpu.0/1000.);
    }
    std::fs::write(output.join("comparison.csv"), csv)?;
    std::fs::write(output.join("gpu-samples.csv"), gpu_rows)?;
    println!("PASS comparison harness and collider roof/floor/wall probes. Surface Nets fails collision acceptance: inspect nonmanifold edges and missed-thin-feature errors.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_ray_check_includes_shared_triangle_edges() {
        let vertices = [
            Vec3::new(-1., 0., -1.),
            Vec3::new(1., 0., -1.),
            Vec3::new(-1., 0., 1.),
            Vec3::new(1., 0., 1.),
        ]
        .into_iter()
        .map(|p| emergent::MeshVertex::new(p, Color::WHITE).unwrap())
        .collect();
        let mesh = Mesh::new(vertices, vec![0, 1, 2, 2, 1, 3]).unwrap();
        assert_eq!(nearest_mesh_hit(&mesh, Vec3::Y, -Vec3::Y), Some(1.));
        assert_eq!(nearest_mesh_hit(&mesh, Vec3::Y, Vec3::Y), None);
        assert_eq!(
            nearest_mesh_hit(&mesh, Vec3::new(2., 1., 0.), -Vec3::Y),
            None
        );
    }
}
