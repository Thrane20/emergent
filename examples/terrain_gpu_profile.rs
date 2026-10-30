//! Native pass timings with no colour readback, PNG, browser or presentation.
use emergent::{
    CameraProjection, Color, Geometry, GeometryInstance, GeometryRenderer, LightInstance,
    LightSource, Mat4, Primitive, RaymarchedBiome, RaymarchedTerrain, ShadowCasting,
    TerrainSettings, Vec3,
};
use std::{path::PathBuf, time::Instant};

fn camera(
    renderer: &mut GeometryRenderer,
    sample: usize,
    scale: f32,
    cavern: bool,
) -> Result<(), emergent::GraphicsError> {
    let angle = sample as f32 / 128. * std::f32::consts::TAU;
    let target = Vec3::new(0., if cavern { 30. } else { 40. }, 0.);
    let radius = if cavern { 20. } else { 650. * scale };
    renderer.set_view(
        target
            + Vec3::new(
                angle.sin() * radius,
                if cavern {
                    0.
                } else {
                    105. * scale + 80. * angle.sin().abs()
                },
                angle.cos() * radius,
            ),
        target,
        Vec3::Y,
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(std::env::args().nth(1).ok_or("Output directory required")?);
    std::fs::create_dir_all(&output)?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(960, 540))?;
    println!(
        "adapter={} backend={} release viewport=960x540 warmup=32 samples=128",
        renderer.info().adapter_name,
        renderer.info().backend
    );
    if !renderer.gpu_timestamps_supported() {
        println!("SKIP: timestamp queries unsupported; renderer remains usable");
        return Ok(());
    }
    renderer.set_projection(CameraProjection::perspective(45., 0.1, 30000.)?);
    let mut csv=String::from("workload,sample,main_gpu_us,shadow_gpu_us,prepare_cpu_us,encode_cpu_us,completion_cpu_us,total_cpu_us,regions,submitted,scissor_pixels,triangles,mesh_buffer_bytes,diagnostic_bytes\n");
    for name in [
        "ray-1km",
        "ray-4km",
        "cavern",
        "ray-eight-regions",
        "heightfield-128",
        "heightfield-512",
        "heightfield-1024",
    ] {
        let mut regions = Vec::new();
        let mut instances = Vec::new();
        let mut triangles = 0;
        let mut mesh_buffer_bytes = 0;
        let setup = Instant::now();
        if name.starts_with("heightfield") {
            let resolution = name.rsplit('-').next().unwrap().parse()?;
            let field = TerrainSettings {
                size: 1024.,
                height: 120.,
                resolution,
                ..Default::default()
            }
            .generate()?;
            let mesh = field.mesh(Color::WHITE)?;
            triangles = mesh.indices().len() / 3;
            instances.push(GeometryInstance::new(
                renderer.insert(&Geometry::from(mesh)),
                Mat4::IDENTITY,
            ));
            mesh_buffer_bytes = renderer
                .geometry_buffer_bytes(instances[0].geometry)
                .unwrap();
        } else {
            let field = RaymarchedTerrain {
                size: if name == "ray-4km" { 4096. } else { 1024. },
                biome: if name == "cavern" {
                    RaymarchedBiome::CaveCavern
                } else {
                    RaymarchedBiome::Hilly
                },
                cave_radius: if name == "cavern" { 16. } else { 8. },
                ..Default::default()
            };
            for index in 0..if name == "ray-eight-regions" { 8 } else { 1 } {
                let id = renderer.insert_raymarched_terrain(field)?;
                if name == "ray-eight-regions" {
                    renderer.transform_raymarched_terrain(
                        id,
                        Mat4::from_translation(Vec3::new(
                            (index % 4) as f32 * 1024. - 1536.,
                            0.,
                            (index / 4) as f32 * 1024. - 512.,
                        )),
                    )?;
                }
                regions.push(id);
            }
        }
        let scale = if name == "ray-4km" {
            3.
        } else if name == "ray-eight-regions" {
            2.
        } else {
            1.
        };
        camera(&mut renderer, 0, scale, name == "cavern")?;
        let before = renderer.render_lit_rgba(&instances, &[])?;
        println!("{name}: preparation/upload/first capture {:?}; triangles={triangles} mesh_bytes={mesh_buffer_bytes}",setup.elapsed());
        for sample in 0..32 {
            camera(&mut renderer, sample * 4, scale, name == "cavern")?;
            renderer.profile_lit_frame(&instances, &[])?;
        }
        let mut gpu = Vec::new();
        let mut cpu = Vec::new();
        for sample in 0..128 {
            camera(&mut renderer, sample, scale, name == "cavern")?;
            let p = renderer
                .profile_lit_frame(&instances, &[])?
                .ok_or("Timestamp capability disappeared")?;
            assert!(p.main_gpu_us.is_finite() && p.main_gpu_us >= 0. && p.shadow_passes == 0);
            let stats = renderer.raymarched_frame_stats();
            csv.push_str(&format!("{name},{sample},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{triangles},{mesh_buffer_bytes},{}\n",p.main_gpu_us,p.shadows_gpu_us,p.prepare_cpu_us,p.encode_cpu_us,p.completion_cpu_us,p.total_cpu_us,stats.regions,stats.submitted,stats.scissor_pixels,p.diagnostic_bytes));
            gpu.push(p.main_gpu_us);
            cpu.push(p.total_cpu_us);
        }
        gpu.sort_by(f64::total_cmp);
        cpu.sort_by(f64::total_cmp);
        println!("{name}: main GPU p50/p95/p99 {:.3}/{:.3}/{:.3} ms; serial CPU wall {:.3}/{:.3}/{:.3} ms",gpu[63]/1000.,gpu[121]/1000.,gpu[126]/1000.,cpu[63]/1000.,cpu[121]/1000.,cpu[126]/1000.);
        camera(&mut renderer, 0, scale, name == "cavern")?;
        let after = renderer.render_lit_rgba(&instances, &[])?;
        assert!(
            before.rgba == after.rgba,
            "Profiling changed capture for {name}"
        );
        for id in regions {
            assert!(renderer.remove_raymarched_terrain(id));
        }
        for instance in instances {
            assert!(renderer.remove(instance.geometry));
            assert_eq!(renderer.geometry_buffer_bytes(instance.geometry), None);
        }
    }
    std::fs::write(output.join("routes.csv"), csv)?;
    // Exercise every query slot and transitions to fewer/zero shadow passes.
    let cube = renderer.insert(&Geometry::from(Primitive::cube(1., Color::WHITE)?));
    let instances = [GeometryInstance::new(cube, Mat4::IDENTITY)];
    renderer.set_view(Vec3::new(5., 5., 5.), Vec3::ZERO, Vec3::Y)?;
    let dir = LightInstance::new(
        LightSource::directional(Color::WHITE, 1.)?.with_shadow_casting(ShadowCasting::Enabled)?,
        Mat4::from_rotation_x(-0.7),
    );
    let point = |x| {
        Ok::<_, Box<dyn std::error::Error>>(LightInstance::new(
            LightSource::point(Color::WHITE, 1., 20.)?
                .with_shadow_casting(ShadowCasting::Enabled)?,
            Mat4::from_translation(Vec3::new(x, 3., 0.)),
        ))
    };
    for (lights, count) in [
        (vec![dir, point(-3.)?, point(3.)?], 13),
        (vec![point(3.)?], 6),
        (vec![dir], 1),
        (vec![], 0),
    ] {
        let p = renderer.profile_lit_frame(&instances, &lights)?.unwrap();
        assert_eq!(p.shadow_passes, count);
        assert!(p.shadows_gpu_us.is_finite());
        if count == 0 {
            assert_eq!(p.shadows_gpu_us, 0.);
        }
    }
    for (width, height) in [(320, 180), (960, 540)] {
        renderer.resize(winit::dpi::PhysicalSize::new(width, height));
        let p = renderer.profile_lit_frame(&instances, &[])?.unwrap();
        assert_eq!(
            p.diagnostic_bytes,
            u64::from(width) * u64::from(height) * 4 + 7168
        );
    }
    assert!(renderer.remove(cube));
    println!("PASS timestamp reuse, 13/6/1/0 shadow pass transitions, resize, unchanged captures and seven traversal workloads");
    Ok(())
}
