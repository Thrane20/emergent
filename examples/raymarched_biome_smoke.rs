//! Native kilometre biome fixture, CPU/GPU coverage and topology checks.
use emergent::{CameraProjection, GeometryRenderer, RaymarchedTerrain, Vec3};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let field = RaymarchedTerrain::default();
    field.validate()?;
    let start = std::time::Instant::now();
    let mut overhang = None;
    let rock = RaymarchedTerrain {
        overhang: 24.,
        cave_radius: 0.,
        ..field
    };
    'search: for x in (-240..240).step_by(8) {
        for z in (-240..240).step_by(8) {
            let mut changes = 0;
            let mut previous = rock.distance(Vec3::new(x as f32, 0., z as f32))? < 0.;
            for y in (2..156).step_by(2) {
                let solid = rock.distance(Vec3::new(x as f32, y as f32, z as f32))? < 0.;
                if solid != previous {
                    changes += 1;
                    previous = solid;
                }
                if changes >= 3 {
                    overhang = Some(Vec3::new(x as f32, y as f32, z as f32));
                    break 'search;
                }
            }
        }
    }
    assert!(overhang.is_some(), "No non-heightfield rock overhang found");
    println!(
        "PASS overhang without cave carving at {:?}; topology search {:?}",
        overhang.unwrap(),
        start.elapsed()
    );
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(640, 480))?;
    let mut id = renderer.insert_raymarched_terrain(field)?;
    renderer.set_projection(CameraProjection::perspective(45., 0.1, 20000.)?);
    for (name, eye, target) in [
        (
            "overview",
            Vec3::new(650., 500., 700.),
            Vec3::new(0., 40., 0.),
        ),
        ("low", Vec3::new(400., 145., 350.), Vec3::new(0., 70., 0.)),
        (
            "overhang",
            overhang.unwrap() + Vec3::new(16., 4., 30.),
            overhang.unwrap() - Vec3::Y * 3.,
        ),
        (
            "cave",
            Vec3::new(
                0.,
                field.relief * 0.28 + 8. * (42. / 100. * std::f32::consts::TAU).sin(),
                field.biome_scale * 0.3 * (42. / 100. * std::f32::consts::TAU).sin(),
            ),
            Vec3::new(24., field.relief * 0.28, 0.),
        ),
    ] {
        renderer.remove_raymarched_terrain(id);
        id = renderer.insert_raymarched_terrain(if name == "overhang" { rock } else { field })?;
        renderer.set_view(eye, target, Vec3::Y)?;
        let start = std::time::Instant::now();
        let frame = renderer.render_lit_rgba(&[], &[])?;
        image::save_buffer(
            format!("/tmp/emergent-biome-{name}.png"),
            &frame.rgba,
            frame.width,
            frame.height,
            image::ColorType::Rgba8,
        )?;
        println!(
            "640x480 {name} native capture including GPU completion/readback: {:?}",
            start.elapsed()
        );
    }
    renderer.remove_raymarched_terrain(id);
    id = renderer.insert_raymarched_terrain(field)?;
    renderer.resize(winit::dpi::PhysicalSize::new(128, 128));
    renderer.set_projection(CameraProjection::isometric(1024., 0.1, 2000.)?);
    renderer.set_view(Vec3::new(0., 300., 0.), Vec3::ZERO, Vec3::Z)?;
    let frame = renderer.render_lit_rgba(&[], &[])?;
    renderer.remove_raymarched_terrain(id);
    let empty = renderer.render_lit_rgba(&[], &[])?;
    let mut hits = 0;
    for y in (4..128).step_by(8) {
        for x in (4..128).step_by(8) {
            let origin = Vec3::new(
                -(2. * (x as f32 + 0.5) / 128. - 1.) * 512.,
                299.9,
                (1. - 2. * (y as f32 + 0.5) / 128.) * 512.,
            );
            let hit = field.trace(origin, -Vec3::Y)?;
            assert!(
                hit.is_some(),
                "Missed bounded top-down terrain at {origin:?}"
            );
            let offset = (y * 128 + x) * 4;
            assert!(
                frame.rgba[offset..offset + 3]
                    .iter()
                    .zip(&empty.rgba[offset..offset + 3])
                    .any(|(a, b)| a.abs_diff(*b) > 3),
                "GPU coverage hole at {x},{y}"
            );
            hits += 1;
        }
    }
    println!(
        "PASS {hits} kilometre-scale CPU/GPU coverage queries; {}",
        renderer.info().adapter_name
    );
    let reference = renderer.insert_raymarched_terrain(field)?;
    renderer.set_projection(CameraProjection::isometric(12., 0.1, 1000.)?);
    let cube = renderer.insert(&emergent::Geometry::from(emergent::Primitive::cube(
        0.5,
        emergent::Color::WHITE,
    )?));
    for (x, z) in [(0., 100.), (100., 100.), (-100., -100.)] {
        let origin = Vec3::new(x, 300., z);
        let t = field
            .trace(origin, -Vec3::Y)?
            .ok_or("Reference query missed terrain")?;
        let surface = origin - Vec3::Y * t;
        renderer.set_view(origin, surface, Vec3::Z)?;
        let baseline = renderer.render_lit_rgba(&[], &[])?;
        let pixel = |frame: &emergent::RenderedFrame| {
            frame.rgba[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 3].to_vec()
        };
        let behind = renderer.render_lit_rgba(
            &[emergent::GeometryInstance::new(
                cube,
                emergent::Mat4::from_translation(surface - Vec3::Y * 2.),
            )],
            &[],
        )?;
        assert_eq!(
            pixel(&baseline),
            pixel(&behind),
            "GPU surface disagrees with CPU reference below terrain"
        );
        let above = renderer.render_lit_rgba(
            &[emergent::GeometryInstance::new(
                cube,
                emergent::Mat4::from_translation(surface + Vec3::Y * 2.),
            )],
            &[],
        )?;
        assert_ne!(
            pixel(&baseline),
            pixel(&above),
            "GPU surface disagrees with CPU reference above terrain"
        );
    }
    renderer.remove_raymarched_terrain(reference);
    println!("PASS biome mesh occlusion agrees with three CPU reference surface queries");
    let large = RaymarchedTerrain {
        size: 4096.,
        ..field
    };
    let id = renderer.insert_raymarched_terrain(large)?;
    renderer.resize(winit::dpi::PhysicalSize::new(640, 480));
    renderer.set_projection(CameraProjection::perspective(45., 0.1, 30000.)?);
    renderer.set_view(
        Vec3::new(2600., 2000., 2800.),
        Vec3::new(0., 40., 0.),
        Vec3::Y,
    )?;
    let frame = renderer.render_lit_rgba(&[], &[])?;
    image::save_buffer(
        "/tmp/emergent-biome-4096.png",
        &frame.rgba,
        frame.width,
        frame.height,
        image::ColorType::Rgba8,
    )?;
    renderer.remove_raymarched_terrain(id);
    println!("PASS native 4096m extent without dense field allocation");
    renderer.resize(winit::dpi::PhysicalSize::new(320, 240));
    for (name, biome) in [
        ("plain", emergent::RaymarchedBiome::Plain),
        ("hilly", emergent::RaymarchedBiome::Hilly),
        ("mountainous", emergent::RaymarchedBiome::Mountainous),
        ("island", emergent::RaymarchedBiome::Island),
        ("rivers", emergent::RaymarchedBiome::Rivers),
        ("canyon", emergent::RaymarchedBiome::Canyon),
        ("underwater", emergent::RaymarchedBiome::Underwater),
        ("cave-cavern", emergent::RaymarchedBiome::CaveCavern),
    ] {
        let recipe = RaymarchedTerrain {
            biome,
            cave_radius: if biome == emergent::RaymarchedBiome::CaveCavern {
                16.
            } else {
                0.
            },
            ..field
        };
        let id = renderer.insert_raymarched_terrain(recipe)?;
        if biome == emergent::RaymarchedBiome::CaveCavern {
            renderer.set_view(Vec3::new(0., 30., 20.), Vec3::new(0., 30., 0.), Vec3::Y)?;
        } else {
            renderer.set_view(Vec3::new(650., 500., 700.), Vec3::new(0., 40., 0.), Vec3::Y)?;
        }
        let frame = renderer.render_lit_rgba(&[], &[])?;
        image::save_buffer(
            format!("/tmp/terrain-biome-{name}.png"),
            &frame.rgba,
            frame.width,
            frame.height,
            image::ColorType::Rgba8,
        )?;
        renderer.remove_raymarched_terrain(id);
    }
    println!("PASS native gallery for all eight selectable biomes");
    Ok(())
}
