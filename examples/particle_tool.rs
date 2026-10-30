//! Bounded editor validation/capture bridge. Normal simulation remains headless.
use emergent::{
    Color, GeometryRenderer, Mat4, MaterialDefinition, MaterialShading, ParticleDefinition,
    ParticleStore, Vec3,
};
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    std::io::stdin()
        .take(128 * 1024 + 1)
        .read_to_string(&mut source)?;
    let definition = ParticleDefinition::from_source(&source)?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("validated");
        return Ok(());
    }
    if args.len() < 2 {
        return Err("Pass project root, output PNG and optional seconds.".into());
    }
    let project = emergent::Project::load(&args[0])?;
    let effect = definition.resolve(&project)?;
    let mut store = ParticleStore::default();
    store.insert(effect, Mat4::IDENTITY)?;
    let seconds: f32 = args.get(2).map_or(Ok(2.), |s| s.parse())?;
    if !seconds.is_finite() || !(0. ..=30.).contains(&seconds) {
        return Err("Preview time must be 0–30 seconds.".into());
    }
    for _ in 0..(seconds * 60.).round() as usize {
        store.step(emergent::PARTICLE_STEP)?;
    }
    let thumbnail = args.get(3).is_some_and(|s| s == "--thumbnail");
    let (width, height) = if thumbnail { (192, 128) } else { (768, 768) };
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(width, height))?;
    renderer.set_view(Vec3::new(4., 3., 6.), Vec3::new(0., 1.5, 0.), Vec3::Y)?;
    if thumbnail {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        store.visit(|e, _, model, particles| {
            for p in particles {
                let position = if e.local_space {
                    model.transform_point3(p.position)
                } else {
                    p.position
                };
                let radius = Vec3::splat(e.size[0].max(e.size[1]) * 0.5 * p.scale);
                min = min.min(position - radius);
                max = max.max(position + radius);
            }
        });
        if min.is_finite() && max.is_finite() {
            let target = (min + max) * 0.5;
            let distance = ((max - min).length() * 1.6).max(0.5);
            renderer.set_projection(emergent::CameraProjection::perspective(
                45.,
                0.01,
                (distance * 5.).max(100.),
            )?);
            renderer.set_view(
                target + Vec3::new(7., 5., 9.).normalize() * distance,
                target,
                Vec3::Y,
            )?;
        }
    }
    let ground = emergent::Primitive::plane(8., 8., Color::WHITE)?;
    let material = MaterialDefinition {
        shading: MaterialShading::Unlit,
        albedo: Some([0.09, 0.12, 0.16]),
        ..Default::default()
    }
    .constant()?;
    let ground = renderer.insert_with_material(&ground.into(), &material)?;
    renderer.set_particles(&store);
    let instances = [emergent::GeometryInstance::new(ground, Mat4::IDENTITY)];
    let frame = renderer.render_lit_rgba(if thumbnail { &[] } else { &instances }, &[])?;
    image::save_buffer(
        &args[1],
        &frame.rgba,
        frame.width,
        frame.height,
        image::ColorType::Rgba8,
    )?;
    println!(
        "{} live particles; adapter {}",
        store.live_count(),
        renderer.info().adapter_name
    );
    Ok(())
}
