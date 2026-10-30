//! Native implicit terrain depth regression and bounded capture workload.
use emergent::{
    CameraProjection, Color, Geometry, GeometryInstance, GeometryRenderer, Mat4, Primitive,
    RaymarchedTerrain, Vec3,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(128, 128))?;
    renderer.set_projection(CameraProjection::perspective(45., 0.1, 200.)?);
    renderer.set_view(Vec3::new(0., 10., 10.), Vec3::new(0., 0., 10.), Vec3::Z)?;
    let center = |frame: &emergent::RenderedFrame| {
        frame.rgba[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 4].to_vec()
    };
    renderer.set_raymarched_terrain(Some(RaymarchedTerrain {
        version: 1,
        size: 32.,
        ..Default::default()
    }))?;
    let field = renderer.render_lit_rgba(&[], &[])?;
    let surface = center(&field);
    let cube = renderer.insert(&Geometry::from(Primitive::cube(0.5, Color::WHITE)?));
    let behind = renderer.render_lit_rgba(
        &[GeometryInstance::new(
            cube,
            Mat4::from_translation(Vec3::new(0., -4., 10.)),
        )],
        &[],
    )?;
    assert_eq!(
        surface,
        center(&behind),
        "mesh behind field leaked through depth"
    );
    let front = renderer.render_lit_rgba(
        &[GeometryInstance::new(
            cube,
            Mat4::from_translation(Vec3::new(0., 4., 10.)),
        )],
        &[],
    )?;
    assert_ne!(
        surface,
        center(&front),
        "foreground mesh was hidden by field"
    );
    renderer.set_raymarched_terrain(None)?;
    assert_ne!(surface, center(&renderer.render_lit_rgba(&[], &[])?));
    let id = renderer.insert_raymarched_terrain(RaymarchedTerrain {
        version: 1,
        size: 32.,
        ..Default::default()
    })?;
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(2., 1.5, 0.75),
        emergent::Quat::from_rotation_y(0.4),
        Vec3::new(30., 3., 2.),
    );
    renderer.transform_raymarched_terrain(id, model)?;
    let target = model.transform_point3(Vec3::new(0., 0., 10.));
    renderer.set_view(target + Vec3::Y * 10., target, Vec3::Z)?;
    assert_eq!(
        surface,
        center(&renderer.render_lit_rgba(&[], &[])?),
        "transformed field did not follow scene transform"
    );
    assert!(renderer.remove_raymarched_terrain(id));
    assert!(!renderer.remove_raymarched_terrain(id));
    assert!(renderer
        .transform_raymarched_terrain(id, Mat4::IDENTITY)
        .is_err());
    let replacement = renderer.insert_raymarched_terrain(RaymarchedTerrain {
        version: 1,
        size: 32.,
        ..Default::default()
    })?;
    assert_ne!(replacement, id);
    let mut foreign = pollster::block_on(GeometryRenderer::new_offscreen(16, 16))?;
    assert!(!foreign.remove_raymarched_terrain(replacement));
    assert!(foreign
        .transform_raymarched_terrain(replacement, Mat4::IDENTITY)
        .is_err());
    renderer.remove_raymarched_terrain(replacement);
    renderer.set_view(Vec3::new(0., 10., 10.), Vec3::new(0., 0., 10.), Vec3::Z)?;
    renderer.set_raymarched_terrain(Some(RaymarchedTerrain {
        version: 1,
        size: 32.,
        ..Default::default()
    }))?;
    renderer.set_projection(CameraProjection::isometric(12., 0.1, 200.)?);
    assert_eq!(
        surface,
        center(&renderer.render_lit_rgba(&[], &[])?),
        "orthographic field differed"
    );
    renderer.set_projection(CameraProjection::isometric(40., 0.1, 200.)?);
    renderer.set_view(Vec3::new(0., 20., 0.), Vec3::ZERO, Vec3::Z)?;
    let overhead = renderer.render_lit_rgba(&[], &[])?;
    renderer.set_raymarched_terrain(None)?;
    let background = renderer.render_lit_rgba(&[], &[])?;
    renderer.set_raymarched_terrain(Some(RaymarchedTerrain {
        version: 1,
        size: 32.,
        ..Default::default()
    }))?;
    for y in (4..128).step_by(8) {
        for x in (4..128).step_by(8) {
            let origin = Vec3::new(
                -(2. * (x as f32 + 0.5) / 128. - 1.) * 20.,
                19.9,
                (1. - 2. * (y as f32 + 0.5) / 128.) * 20.,
            );
            let expected = RaymarchedTerrain {
                version: 1,
                size: 32.,
                ..Default::default()
            }
            .trace(origin, -Vec3::Y)?
            .is_some();
            let offset = (y * 128 + x) * 4;
            assert_eq!(
                expected,
                overhead.rgba[offset..offset + 3].iter().zip(&background.rgba[offset..offset + 3]).any(|(a,b)| a.abs_diff(*b)>3),
                "CPU/GPU silhouette mismatch at {x},{y}: origin={origin:?}, pixel={:?}, background={:?}", &overhead.rgba[offset..offset+3], &background.rgba[offset..offset+3]
            );
        }
    }
    renderer.set_view(Vec3::new(0., -1., 10.), Vec3::new(0., 1., 10.), Vec3::Z)?;
    let inside = renderer.render_lit_rgba(&[], &[])?;
    assert!(
        center(&inside)[1] > 20,
        "inside-solid camera did not reach exit"
    );
    renderer.set_projection(CameraProjection::perspective(45., 0.1, 200.)?);
    renderer.set_view(Vec3::new(7., 2., 12.), Vec3::new(7., 2., 0.), Vec3::Y)?;
    let arch = renderer.render_lit_rgba(&[], &[])?;
    image::save_buffer(
        "/tmp/emergent-raymarch-arch.png",
        &arch.rgba,
        128,
        128,
        image::ColorType::Rgba8,
    )?;
    renderer.set_view(Vec3::new(-5., 1., 0.), Vec3::new(-5., 1., -10.), Vec3::Y)?;
    let cave = renderer.render_lit_rgba(&[], &[])?;
    image::save_buffer(
        "/tmp/emergent-raymarch-cave.png",
        &cave.rgba,
        128,
        128,
        image::ColorType::Rgba8,
    )?;
    println!(
        "PASS native field depth/mesh occlusion, disable, orthographic and inside-solid camera; {}",
        renderer.info().adapter_name
    );
    for (width, height) in [
        (128, 128),
        (256, 256),
        (512, 512),
        (1280, 720),
        (1920, 1080),
    ] {
        renderer.resize(winit::dpi::PhysicalSize::new(width, height));
        for (name, eye, target) in [
            ("overview", Vec3::new(24., 16., 28.), Vec3::new(0., 1., 0.)),
            ("cave", Vec3::new(-5., 1., 0.), Vec3::new(-5., 1., -10.)),
            ("arch", Vec3::new(7., 2., 12.), Vec3::new(7., 2., 0.)),
        ] {
            renderer.set_view(eye, target, Vec3::Y)?;
            let warm = renderer.render_lit_rgba(&[], &[])?;
            if width == 512 && name == "overview" {
                image::save_buffer(
                    "/tmp/emergent-raymarch-overview.png",
                    &warm.rgba,
                    width,
                    height,
                    image::ColorType::Rgba8,
                )?;
            }
            let mut timings = [0.; 5];
            for value in &mut timings {
                let start = std::time::Instant::now();
                let _ = renderer.render_lit_rgba(&[], &[])?;
                *value = start.elapsed().as_secs_f64() * 1000.;
            }
            timings.sort_by(f64::total_cmp);
            println!("{width}x{height} {name}: capture median {:.3}ms range {:.3}..{:.3}ms (GPU completion + readback, not GPU timestamps)",timings[2],timings[0],timings[4]);
        }
    }
    Ok(())
}
