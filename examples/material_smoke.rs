//! Native render regression: each material channel must affect actual output.
use emergent::{
    Color, Geometry, GeometryInstance, GeometryRenderer, LightInstance, LightSource, Mat4,
    MaterialDefinition, MaterialNoiseMap, MaterialShading, Primitive, Project, Vec3,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("projects/first_project"),
    )?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(256, 256))?;
    renderer.set_view(Vec3::new(3.0, 2.0, 4.0), Vec3::ZERO, Vec3::Y)?;
    let geometry = Geometry::from(Primitive::sphere(1.0, 24, 16, Color::WHITE)?);
    let lights = [
        LightInstance::new(LightSource::ambient(Color::WHITE, 0.1)?, Mat4::IDENTITY),
        LightInstance::new(LightSource::directional(Color::WHITE, 0.8)?, Mat4::IDENTITY),
    ];
    let mut capture =
        |definition: MaterialDefinition| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let material = definition.resolve(&project)?;
            let handle = renderer.insert_with_material(&geometry, &material)?;
            let image = renderer
                .render_lit_rgba(&[GeometryInstance::new(handle, Mat4::IDENTITY)], &lights)?;
            renderer.remove(handle);
            Ok(image.rgba)
        };
    let base = MaterialDefinition {
        shading: MaterialShading::Smooth,
        albedo: Some([0.3, 0.4, 0.5]),
        ..Default::default()
    };
    let baseline = capture(base.clone())?;
    let difference = |a: &[u8], b: &[u8]| a.iter().zip(b).filter(|(a, b)| a != b).count();
    for (name, definition) in [
        (
            "albedo",
            MaterialDefinition {
                albedo: Some([0.8, 0.05, 0.02]),
                ..base.clone()
            },
        ),
        (
            "flat shading",
            MaterialDefinition {
                shading: MaterialShading::Flat,
                ..base.clone()
            },
        ),
        (
            "specular",
            MaterialDefinition {
                specular: [1.0; 3],
                shininess: 16.0,
                ..base.clone()
            },
        ),
        (
            "emission",
            MaterialDefinition {
                emission: [0.2, 0.1, 0.05],
                emission_intensity: 2.0,
                ..base.clone()
            },
        ),
        (
            "unlit",
            MaterialDefinition {
                shading: MaterialShading::Unlit,
                ..base.clone()
            },
        ),
    ] {
        let image = capture(definition)?;
        let changed = difference(&baseline, &image);
        assert!(
            changed > 100,
            "{name} did not visibly change the native frame: {changed}"
        );
        println!("{name}: {changed} changed bytes");
    }
    for channel in 0..4 {
        let mut constant = base.clone();
        constant.specular = [0.8; 3];
        constant.shininess = 16.0;
        constant.emission = [0.1, 0.1, 0.1];
        constant.emission_intensity = 1.0;
        let before = capture(constant.clone())?;
        let map = Some(MaterialNoiseMap {
            asset: "assets/filters/material-grain.noise.toml".into(),
            strength: 1.0,
        });
        match channel {
            0 => constant.albedo_map = map,
            1 => constant.specular_map = map,
            2 => constant.emission_map = map,
            _ => constant.bump_map = map,
        }
        let after = capture(constant.clone())?;
        let changed = difference(&before, &after);
        assert!(
            changed > 100,
            "noise channel {channel} did not change the native frame: {changed}"
        );
        assert_eq!(
            after,
            capture(constant)?,
            "noise bake/render must repeat for fixed inputs"
        );
        println!("noise channel {channel}: {changed} changed bytes");
    }
    println!(
        "Material smoke passed on {} / {}",
        renderer.info().adapter_name,
        renderer.info().backend
    );
    Ok(())
}
