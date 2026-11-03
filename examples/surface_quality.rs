//! Reproducible native surface captures and scoped timing measurements.
use emergent::{
    Color, Geometry, GeometryInstance, GeometryRenderer, LightInstance, LightSource, Mat4,
    MaterialDefinition, MaterialNoiseMap, MaterialShading, Primitive, Project, Vec3,
};
use std::{
    error::Error,
    fs::{self, File},
    io::{BufWriter, Write},
    time::Instant,
};
fn main() -> Result<(), Box<dyn Error>> {
    let stage = std::env::args().nth(1).unwrap_or("current".into());
    if stage == "before" {
        return Err("Use the preserved baseline-renderer executable for original captures; this renderer contains the fixes.".into());
    }
    let directory = std::env::args()
        .nth(2)
        .unwrap_or("/tmp/emergent-surface-quality".into());
    fs::create_dir_all(&directory)?;
    let project = Project::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-project"),
    )?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(640, 640))?;
    renderer.set_view(Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO, Vec3::Y)?;
    let lights = [
        LightInstance::new(LightSource::ambient(Color::WHITE, 0.12)?, Mat4::IDENTITY),
        LightInstance::new(
            LightSource::directional(Color::WHITE, 0.85)?,
            Mat4::from_rotation_y(0.5) * Mat4::from_rotation_x(-0.6),
        ),
    ];
    println!(
        "adapter={}, backend={}, capture=640x640, profile=release",
        renderer.info().adapter_name,
        renderer.info().backend
    );
    for (label, sectors, stacks) in [("low", 24, 12), ("standard", 64, 32), ("high", 128, 64)] {
        let geometry = Geometry::from(Primitive::sphere(1.0, sectors, stacks, Color::WHITE)?);
        let definition = MaterialDefinition {
            shading: MaterialShading::Smooth,
            albedo: Some([0.22, 0.45, 0.65]),
            specular: [0.55; 3],
            shininess: 64.0,
            bump_strength: 0.15,
            noise_resolution: match label {
                "standard" => 64,
                "high" => 128,
                _ => 32,
            },
            bump_map: Some(MaterialNoiseMap {
                asset: "assets/filters/material-grain.noise.toml".into(),
                strength: 1.0,
            }),
            ..Default::default()
        };
        let material = definition.resolve(&project)?;
        let upload = Instant::now();
        let id = renderer.insert_with_material(&geometry, &material)?;
        let upload_ms = upload.elapsed().as_secs_f64() * 1000.0;
        let instances = [GeometryInstance::new(id, Mat4::IDENTITY)];
        for _ in 0..2 {
            renderer.render_lit_rgba(&instances, &lights)?;
        }
        let mut timings = Vec::new();
        let mut frame = None;
        for _ in 0..7 {
            let start = Instant::now();
            frame = Some(renderer.render_lit_rgba(&instances, &lights)?);
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        timings.sort_by(f64::total_cmp);
        let frame = frame.unwrap();
        let target = std::path::Path::new(&directory).join(format!("{stage}-{label}.ppm"));
        let mut output = BufWriter::new(File::create(&target)?);
        write!(output, "P6\n{} {}\n255\n", frame.width, frame.height)?;
        for pixel in frame.rgba.as_chunks::<4>().0 {
            output.write_all(&pixel[..3])?;
        }
        output.flush()?;
        println!("{stage}-{label}: sphere={sectors}x{stacks}, triangles={}, upload_bake_cpu_ms={upload_ms:.3}, captured_frame_median_ms={:.3}, file={}",2*sectors*(stacks-1),timings[3],target.display());
        renderer.remove(id);
    }
    Ok(())
}
