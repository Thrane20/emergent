//! Native environment regression; temporary assets only, no external services.
use emergent::{
    Color, EnvironmentDefinition, Geometry, GeometryInstance, GeometryRenderer, LightInstance,
    Mat4, MaterialDefinition, MaterialShading, Primitive, Project, SceneDefinition, Vec3, World,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("emergent-lighting-smoke-{}", std::process::id()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    std::fs::create_dir_all(root.join("assets"))?;
    std::fs::create_dir_all(root.join("src/generated"))?;
    std::fs::write(root.join("emergent.toml"),"schema_version=1\nid='lighting-smoke'\nname='Lighting smoke'\nentry_scene='assets/main.scene.toml'\ngenerated_code='src/generated'\nasset_roots=['assets']\n[[outputs]]\nid='host'\nos='macos'\narchitecture='aarch64'\nrust_target='aarch64-apple-darwin'\nprofile='development'\n")?;
    let mut pixels = vec![0u8; 256 * 128 * 4];
    for z in 0..128 {
        for x in 0..256 {
            let i = (z * 256 + x) * 4;
            pixels[i..i + 4].copy_from_slice(if x < 128 {
                &[255, 0, 0, 255]
            } else {
                &[0, 255, 0, 255]
            });
        }
    }
    image::save_buffer(
        root.join("assets/sky.png"),
        &pixels,
        256,
        128,
        image::ColorType::Rgba8,
    )?;
    std::fs::write(root.join("assets/main.scene.toml"),"schema_version=1\nid='main'\nentities=[]\n[environment]\nambient_color=[0.8,0.2,0.1]\nambient_intensity=0.6\nskybox='assets/sky.png'\n")?;
    let project = Project::load(&root)?;
    let loaded =
        SceneDefinition::load(&project, "assets/main.scene.toml")?.instantiate(&mut World::new())?;
    assert_eq!(loaded.environment.settings.ambient_intensity, 0.6);
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(128, 128))?;
    let mut settings = EnvironmentDefinition {
        background_color: [0.25, 0., 0.],
        ..Default::default()
    };
    renderer.set_environment(&settings.resolve(&project)?);
    let center = |image: emergent::RenderedFrame| {
        image.rgba[(64 * 128 + 64) * 4..(64 * 128 + 64) * 4 + 4].to_vec()
    };
    let solid = center(renderer.render_lit_rgba(&[], &[])?);
    assert!(solid[0] > 130 && solid[0] < 145 && solid[1] < 3);
    settings.skybox = Some("assets/sky.png".into());
    renderer.set_environment(&settings.resolve(&project)?);
    renderer.set_view(Vec3::ZERO, -Vec3::Z, Vec3::Y)?;
    let red = center(renderer.render_lit_rgba(&[], &[])?);
    assert!(red[0] > 250 && red[1] < 5, "{red:?}");
    renderer.set_view(Vec3::new(100., 5., 9.), Vec3::new(100., 5., 8.), Vec3::Y)?;
    assert_eq!(
        red,
        center(renderer.render_lit_rgba(&[], &[])?),
        "translation moved the sky"
    );
    settings.skybox_rotation = 180.;
    renderer.set_environment(&settings.resolve(&project)?);
    let green = center(renderer.render_lit_rgba(&[], &[])?);
    assert!(
        green[1] > 250 && green[0] < 5,
        "rotation did not change panorama: {green:?}"
    );
    settings.skybox_intensity = 0.;
    renderer.set_environment(&settings.resolve(&project)?);
    assert_eq!(center(renderer.render_lit_rgba(&[], &[])?), [0, 0, 0, 255]);
    renderer.set_projection(emergent::CameraProjection::isometric(10., 0.1, 100.)?);
    settings.skybox_intensity = 1.;
    renderer.set_environment(&settings.resolve(&project)?);
    let orthographic = center(renderer.render_lit_rgba(&[], &[])?);
    assert!(orthographic[1] > 250 && orthographic[0] < 5);
    settings.skybox = None;
    settings.ambient_color = [1., 0., 0.];
    settings.ambient_intensity = 0.5;
    renderer.set_environment(&settings.resolve(&project)?);
    renderer.set_view(Vec3::new(3., 3., 3.), Vec3::ZERO, Vec3::Y)?;
    let geometry = Geometry::from(Primitive::cube(1., Color::WHITE)?);
    let material = MaterialDefinition {
        albedo: Some([1.; 3]),
        shading: MaterialShading::Smooth,
        ..Default::default()
    }
    .resolve(&project)?;
    let handle = renderer.insert_with_material(&geometry, &material)?;
    let light = LightInstance::new(settings.ambient_light()?, Mat4::IDENTITY);
    let ambient = center(
        renderer.render_lit_rgba(&[GeometryInstance::new(handle, Mat4::IDENTITY)], &[light])?,
    );
    assert!(
        ambient[0] > 170 && ambient[1] < 5 && ambient[2] < 5,
        "ambient colour/intensity did not reach surface: {ambient:?}"
    );
    println!("PASS scene environment roundtrip, background colour, panorama sampling/rotation/brightness, translation independence, orthographic sky and ambient illumination; {}",renderer.info().adapter_name);
    Ok(())
}
