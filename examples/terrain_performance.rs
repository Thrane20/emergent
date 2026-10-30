//! Warm capture benchmark. Wall time includes GPU completion/readback, not PNG.
use emergent::{CameraProjection, GeometryRenderer, RaymarchedBiome, RaymarchedTerrain, Vec3};
use std::{path::PathBuf, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(std::env::args().nth(1).ok_or("Output directory required")?);
    std::fs::create_dir_all(&output)?;
    let (width, height) = if std::env::args().any(|arg| arg == "--half") {
        (480, 270)
    } else {
        (960, 540)
    };
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(width, height))?;
    renderer.set_raymarched_render_options(emergent::RaymarchedRenderOptions {
        bounds_culling: !std::env::args().any(|arg| arg == "--no-culling"),
        ..Default::default()
    });
    println!(
        "adapter={}, profile=release, viewport={width}x{height}, warmup=8, samples=40",
        renderer.info().adapter_name
    );
    renderer.set_projection(CameraProjection::perspective(45., 0.1, 30000.)?);
    let mut csv = String::from("view,sample,capture_us\n");
    for (name, field, eye, target) in [
        (
            "overview",
            RaymarchedTerrain::default(),
            Vec3::new(650., 500., 700.),
            Vec3::new(0., 40., 0.),
        ),
        (
            "grazing",
            RaymarchedTerrain::default(),
            Vec3::new(400., 145., 350.),
            Vec3::new(0., 70., 0.),
        ),
        (
            "cavern",
            RaymarchedTerrain {
                biome: RaymarchedBiome::CaveCavern,
                cave_radius: 16.,
                ..Default::default()
            },
            Vec3::new(8., 30., 8.),
            Vec3::new(0., 30., 0.),
        ),
        (
            "small",
            RaymarchedTerrain::default(),
            Vec3::new(2600., 2000., 2800.),
            Vec3::new(0., 40., 0.),
        ),
        (
            "offscreen",
            RaymarchedTerrain::default(),
            Vec3::new(650., 500., 700.),
            Vec3::new(1300., 900., 1400.),
        ),
    ] {
        let id = renderer.insert_raymarched_terrain(field)?;
        renderer.set_view(eye, target, Vec3::Y)?;
        let cropped = renderer.render_lit_rgba(&[], &[])?;
        renderer.set_raymarched_render_options(emergent::RaymarchedRenderOptions {
            bounds_culling: false,
            step_heatmap: false,
        });
        let full = renderer.render_lit_rgba(&[], &[])?;
        let difference = cropped
            .rgba
            .iter()
            .zip(&full.rgba)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        // Metal can round an sRGB clear differently when the entire draw is culled.
        assert!(
            difference <= u8::from(name == "offscreen"),
            "Bounds changed {name}: max channel error {difference}"
        );
        renderer.set_raymarched_render_options(emergent::RaymarchedRenderOptions {
            bounds_culling: !std::env::args().any(|arg| arg == "--no-culling"),
            step_heatmap: false,
        });
        for _ in 0..8 {
            renderer.render_lit_rgba(&[], &[])?;
        }
        let mut times = Vec::with_capacity(40);
        for sample in 0..40 {
            let start = Instant::now();
            let frame = renderer.render_lit_rgba(&[], &[])?;
            let us = start.elapsed().as_micros();
            times.push(us);
            csv.push_str(&format!("{name},{sample},{us}\n"));
            if sample == 0 {
                if let Some(reference) = std::env::args()
                    .find_map(|arg| arg.strip_prefix("--reference=").map(str::to_owned))
                {
                    let original =
                        image::open(PathBuf::from(reference).join(format!("{name}.png")))?
                            .to_rgba8();
                    assert_eq!(original.dimensions(), (frame.width, frame.height));
                    let error = original
                        .as_raw()
                        .iter()
                        .zip(&frame.rgba)
                        .map(|(a, b)| a.abs_diff(*b))
                        .max()
                        .unwrap_or(0);
                    assert!(
                        error <= 1,
                        "Shader changed {name}: max channel error {error}"
                    );
                    println!("{name}: reference max channel error {error}/255");
                }
                image::save_buffer(
                    output.join(format!("{name}.png")),
                    &frame.rgba,
                    frame.width,
                    frame.height,
                    image::ColorType::Rgba8,
                )?;
            }
        }
        times.sort_unstable();
        println!("{name}: {:?}", renderer.raymarched_frame_stats());
        println!(
            "{name}: p50={} p95={} p99={} us",
            times[19], times[37], times[39]
        );
        assert!(renderer.remove_raymarched_terrain(id));
    }
    std::fs::write(output.join("capture.csv"), csv)?;
    Ok(())
}
