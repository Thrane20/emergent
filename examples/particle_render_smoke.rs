//! Native regression for texture alpha, frame selection, depth ordering and resize.
use emergent::{
    GeometryRenderer, Mat4, ParticleBlend, ParticleDefinition, ParticleEmitter, ParticleShape,
    ParticleStore, Project, Vec3,
};
use std::{fs, path::PathBuf};
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Temporary(
        std::env::temp_dir().join(format!("emergent-particle-gpu-{}", std::process::id())),
    );
    fs::create_dir_all(dir.0.join("assets"))?;
    fs::create_dir_all(dir.0.join("src/generated"))?;
    fs::write(dir.0.join("emergent.toml"),"schema_version=1\nid='particle-gpu'\nname='Particle GPU'\nentry_scene='assets/main.scene.toml'\ngenerated_code='src/generated'\nasset_roots=['assets']\n[[outputs]]\nid='host'\nos='macos'\narchitecture='aarch64'\nrust_target='aarch64-apple-darwin'\nprofile='development'\n")?;
    fs::write(
        dir.0.join("assets/main.scene.toml"),
        "schema_version=1\nid='main'\nentities=[]\n",
    )?;
    image::save_buffer(
        dir.0.join("assets/atlas.png"),
        &[255, 0, 0, 255, 0, 0, 255, 255],
        2,
        1,
        image::ColorType::Rgba8,
    )?;
    image::save_buffer(
        dir.0.join("assets/invisible.png"),
        &[255, 0, 0, 0],
        1,
        1,
        image::ColorType::Rgba8,
    )?;
    image::save_buffer(
        dir.0.join("assets/ping-pong.png"),
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ],
        4,
        1,
        image::ColorType::Rgba8,
    )?;
    let project = Project::load(&dir.0)?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(128, 128))?;
    renderer.set_view(Vec3::new(0., 0., 4.), Vec3::ZERO, Vec3::Y)?;
    let emitter = ParticleEmitter {
        shape: ParticleShape::Point,
        rate: 0.,
        burst: 1,
        max_particles: 1,
        speed: 0.,
        gravity: [0.; 3],
        drag: 0.,
        lifetime: [10.; 2],
        looping: false,
        size: [1.; 2],
        color_start: [1.; 4],
        color_end: [1.; 4],
        soft_distance: 0.,
        texture: Some("assets/atlas.png".into()),
        columns: 2,
        frames: 2,
        fps: 1.,
        ..Default::default()
    };
    let make = |emitters| -> Result<ParticleStore, Box<dyn std::error::Error>> {
        let definition = ParticleDefinition {
            schema_version: 1,
            id: "test".into(),
            name: "Test".into(),
            emitters,
        };
        let mut store = ParticleStore::default();
        store.insert(definition.resolve(&project)?, Mat4::IDENTITY)?;
        store.step(emergent::PARTICLE_STEP)?;
        Ok(store)
    };
    let center = |bytes: &[u8], w: usize| -> [u8; 4] {
        bytes[(w / 2 * w + w / 2) * 4..(w / 2 * w + w / 2) * 4 + 4]
            .try_into()
            .unwrap()
    };
    let mut atlas = make(vec![emitter.clone()])?;
    renderer.set_particles(&atlas);
    let red = renderer.render_lit_rgba(&[], &[])?;
    let pixel = center(&red.rgba, 128);
    assert!(
        pixel[0] > 200 && pixel[2] < 100,
        "frame zero is red: {pixel:?}"
    );
    for _ in 0..61 {
        atlas.step(emergent::PARTICLE_STEP)?;
    }
    renderer.set_particles(&atlas);
    let blue = renderer.render_lit_rgba(&[], &[])?;
    let pixel = center(&blue.rgba, 128);
    assert!(
        pixel[2] > 200 && pixel[0] < 100,
        "frame one is blue: {pixel:?}"
    );
    let mut reflected = make(vec![ParticleEmitter {
        texture: Some("assets/ping-pong.png".into()),
        columns: 4,
        frames: 4,
        ping_pong: true,
        ..emitter.clone()
    }])?;
    // Samples at phases ~1.1..6.6 prove the GPU turns back through blue and green.
    for expected in [
        [0u8, 255, 0],
        [0, 0, 255],
        [255, 255, 0],
        [0, 0, 255],
        [0, 255, 0],
        [255, 0, 0],
    ] {
        for _ in 0..66 {
            reflected.step(emergent::PARTICLE_STEP)?;
        }
        renderer.set_particles(&reflected);
        let capture = renderer.render_lit_rgba(&[], &[])?;
        let actual = center(&capture.rgba, 128);
        for channel in 0..3 {
            assert!(
                (i16::from(actual[channel]) - i16::from(expected[channel])).abs() < 35,
                "native ping-pong cell: {actual:?}, expected {expected:?}"
            );
        }
    }
    renderer.clear_particles();
    let background = renderer.render_lit_rgba(&[], &[])?;
    let invisible = make(vec![ParticleEmitter {
        texture: Some("assets/invisible.png".into()),
        columns: 1,
        frames: 1,
        ..emitter.clone()
    }])?;
    renderer.set_particles(&invisible);
    assert_eq!(
        renderer.render_lit_rgba(&[], &[])?.rgba,
        background.rgba,
        "zero texture alpha must leave no quad"
    );
    let far = ParticleEmitter {
        texture: None,
        columns: 1,
        frames: 1,
        position: [0., 0., -0.5],
        color_start: [0., 0., 1., 0.7],
        color_end: [0., 0., 1., 0.7],
        ..emitter.clone()
    };
    let near = ParticleEmitter {
        position: [0.; 3],
        color_start: [1., 0., 0., 0.7],
        color_end: [1., 0., 0., 0.7],
        ..far.clone()
    };
    let first = make(vec![near.clone(), far.clone()])?;
    let second = make(vec![far, near])?;
    renderer.set_particles(&first);
    let ordered = renderer.render_lit_rgba(&[], &[])?.rgba;
    renderer.set_particles(&second);
    assert_eq!(
        ordered,
        renderer.render_lit_rgba(&[], &[])?.rgba,
        "alpha particles sort across emitters"
    );
    let black = ParticleEmitter {
        color_start: [0., 0., 0., 1.],
        color_end: [0., 0., 0., 1.],
        fps: 0.,
        ..emitter.clone()
    };
    let glow = ParticleEmitter {
        texture: None,
        columns: 1,
        frames: 1,
        position: [0., 0., -0.5],
        blend: ParticleBlend::Additive,
        color_start: [0., 0., 1., 1.],
        color_end: [0., 0., 1., 1.],
        ..emitter.clone()
    };
    let mixed = make(vec![black, glow])?;
    renderer.set_particles(&mixed);
    let pixel = center(&renderer.render_lit_rgba(&[], &[])?.rgba, 128);
    assert!(
        pixel[2] < 5,
        "opaque alpha must occlude additive particles behind it: {pixel:?}"
    );
    // Same sprite, different blend pipelines: batching must retain sorted order.
    let base = ParticleEmitter {
        texture: None,
        columns: 1,
        frames: 1,
        fps: 0.,
        position: [0., 0., -0.5],
        color_start: [0.4, 0.2, 0.1, 1.],
        color_end: [0.4, 0.2, 0.1, 1.],
        ..emitter.clone()
    };
    let base_store = make(vec![base.clone()])?;
    renderer.set_particles(&base_store);
    let base_pixel = center(&renderer.render_lit_rgba(&[], &[])?.rgba, 128);
    for blend in [ParticleBlend::Screen, ParticleBlend::Multiply] {
        let overlay = ParticleEmitter {
            blend,
            position: [0.; 3],
            color_start: [0.5; 4],
            color_end: [0.5; 4],
            ..base.clone()
        };
        let store = make(vec![overlay.clone(), base.clone()])?;
        renderer.set_particles(&store);
        let result = renderer.render_lit_rgba(&[], &[])?.rgba;
        let pixel = center(&result, 128);
        for channel in 0..3 {
            assert!(
                if blend == ParticleBlend::Screen {
                    pixel[channel] > base_pixel[channel]
                } else {
                    pixel[channel] < base_pixel[channel]
                },
                "{blend:?}: {pixel:?} vs {base_pixel:?}"
            );
        }
        let reversed = make(vec![base.clone(), overlay.clone()])?;
        renderer.set_particles(&reversed);
        assert_eq!(
            renderer.render_lit_rgba(&[], &[])?.rgba,
            result,
            "blend sorting across emitters"
        );
        let transparent = make(vec![
            base.clone(),
            ParticleEmitter {
                color_start: [1., 1., 1., 0.],
                color_end: [1., 1., 1., 0.],
                ..overlay
            },
        ])?;
        renderer.set_particles(&transparent);
        assert_eq!(
            center(&renderer.render_lit_rgba(&[], &[])?.rgba, 128),
            base_pixel,
            "zero opacity is neutral for {blend:?}"
        );
    }
    let plane = emergent::Primitive::plane(3., 3., emergent::Color::WHITE)?;
    let material = emergent::MaterialDefinition {
        shading: emergent::MaterialShading::Unlit,
        albedo: Some([0.1; 3]),
        ..Default::default()
    }
    .constant()?;
    let plane = renderer.insert_with_material(&plane.into(), &material)?;
    let instances = [emergent::GeometryInstance::new(
        plane,
        Mat4::from_translation(Vec3::new(0., 0., -0.1))
            * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2),
    )];
    let hard = make(vec![ParticleEmitter {
        fps: 0.,
        ..emitter.clone()
    }])?;
    renderer.set_particles(&hard);
    let hard_pixel = center(&renderer.render_lit_rgba(&instances, &[])?.rgba, 128);
    let soft = make(vec![ParticleEmitter {
        fps: 0.,
        soft_distance: 1.,
        ..emitter.clone()
    }])?;
    renderer.set_particles(&soft);
    let soft_pixel = center(&renderer.render_lit_rgba(&instances, &[])?.rgba, 128);
    assert!(
        soft_pixel[0] + 20 < hard_pixel[0],
        "soft intersections fade near opaque depth: {soft_pixel:?} / {hard_pixel:?}"
    );
    renderer.clear_particles();
    let plane_only = renderer.render_lit_rgba(&instances, &[])?.rgba;
    let behind = make(vec![ParticleEmitter {
        position: [0., 0., -0.5],
        ..emitter.clone()
    }])?;
    renderer.set_particles(&behind);
    assert_eq!(
        renderer.render_lit_rgba(&instances, &[])?.rgba,
        plane_only,
        "opaque surfaces occlude particles"
    );
    renderer.resize(winit::dpi::PhysicalSize::new(96, 96));
    renderer.set_particles(&atlas);
    assert_eq!(renderer.render_lit_rgba(&[], &[])?.rgba.len(), 96 * 96 * 4);
    // Bounded end-to-end capture timings include upload, draw, GPU wait and readback.
    renderer.resize(winit::dpi::PhysicalSize::new(512, 512));
    renderer.set_view(Vec3::new(0., 2., 8.), Vec3::ZERO, Vec3::Y)?;
    for count in [0u32, 1000, 10000] {
        let emitters = (0..3)
            .map(|i| {
                let n = count / 3 + u32::from(i == 0) * (count % 3);
                ParticleEmitter {
                    max_particles: n,
                    burst: n,
                    rate: 0.,
                    seed: i,
                    shape: ParticleShape::Box,
                    extent: [3., 2., 3.],
                    speed: 0.,
                    gravity: [0.; 3],
                    lifetime: [120.; 2],
                    duration: 120.,
                    size: [0.15, 0.35],
                    blend: if i == 0 {
                        ParticleBlend::Alpha
                    } else {
                        ParticleBlend::Additive
                    },
                    ..Default::default()
                }
            })
            .collect();
        let mut store = ParticleStore::default();
        if count > 0 {
            store.insert(
                ParticleDefinition {
                    schema_version: 1,
                    id: "bench".into(),
                    name: "Bench".into(),
                    emitters,
                }
                .procedural()?,
                Mat4::IDENTITY,
            )?;
        }
        store.step(emergent::PARTICLE_STEP)?;
        renderer.set_particles(&store);
        let _ = renderer.render_lit_rgba(&[], &[])?;
        let mut times = Vec::new();
        for _ in 0..20 {
            let start = std::time::Instant::now();
            store.step(emergent::PARTICLE_STEP)?;
            renderer.set_particles(&store);
            let frame = renderer.render_lit_rgba(&[], &[])?;
            std::hint::black_box(frame);
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        println!("{count} live / {} emitters / 512x512 / 20 captures: wall p50 {:.3} ms p95 {:.3} ms (includes GPU wait/readback)",if count == 0 {0} else {3},times[9],times[18]);
    }
    println!(
        "Particle GPU checks passed on {} / {}",
        renderer.info().adapter_name,
        renderer.info().backend
    );
    Ok(())
}
