use emergent::{MaterialDefinition, Project, ShaderDefinition};
use std::collections::BTreeMap;
fn project() -> Project {
    Project::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/standard"))
        .unwrap()
}
fn graph(kind: &str) -> ShaderDefinition {
    toml::from_str(
        &std::fs::read_to_string(
            project()
                .asset_path(format!("assets/shaders/{kind}.shader.toml"))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn bounded_graph_validation_and_material_parameters() {
    for kind in ["force-field", "water", "dissolve"] {
        let g = graph(kind);
        g.validate().unwrap();
        assert!(g.wgsl().unwrap().contains("fn surface_graph"));
        let material = MaterialDefinition {
            shader: Some(format!("assets/shaders/{kind}.shader.toml")),
            cast_shadows: false,
            ..Default::default()
        }
        .resolve(&project())
        .unwrap();
        assert!(material.shader().is_some());
    }
    let g = graph("water");
    assert!(g
        .parameter_values(&BTreeMap::from([("missing".into(), 1.)]))
        .is_err());
    assert!(g
        .parameter_values(&BTreeMap::from([("height".into(), f32::NAN)]))
        .is_err());
    assert!(g
        .parameter_values(&BTreeMap::from([("height".into(), 100.)]))
        .is_err());
    let mut bad = g.clone();
    bad.nodes[0].inputs.push("missing".into());
    assert!(bad.validate().is_err());
    let mut bad = g.clone();
    bad.nodes[0].id = bad.nodes[1].id.clone();
    assert!(bad.validate().is_err());
    let mut bad = g.clone();
    bad.outputs.opacity = "missing".into();
    assert!(bad.validate().is_err());
    let mut bad = g.clone();
    bad.nodes[0].value = f32::INFINITY;
    assert!(bad.validate().is_err());
    let mut bad = g.clone();
    bad.parameters[0].min = 2.;
    assert!(bad.validate().is_err());
    // Editor layout is accepted and ignored by compilation, but must be finite.
    let mut laid_out = g.clone();
    laid_out.nodes[0].position = Some([120., -40.]);
    laid_out.outputs.position = Some([900., 0.]);
    laid_out.validate().unwrap();
    assert_eq!(laid_out.wgsl().unwrap(), g.wgsl().unwrap());
    let mut bad = g.clone();
    bad.nodes[0].position = Some([f32::NAN, 0.]);
    assert!(bad.validate().is_err());
    let mut bad = graph("force-field");
    bad.outputs.displacement = "rim".into();
    assert!(bad.validate().is_err());
    assert!(MaterialDefinition {
        shader: Some("assets/shaders/water.shader.toml".into()),
        ..Default::default()
    }
    .resolve(&project())
    .is_err());
}
#[cfg(feature = "graphics")]
#[test]
fn native_graph_pipelines_render_time_opacity_and_material_replacement() {
    use emergent::{
        CameraProjection, Color, Geometry, GeometryInstance, GeometryRenderer, Mat4, Primitive,
        Vec3,
    };
    let project = project();
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(96, 96)).unwrap();
    renderer
        .set_camera(
            Vec3::new(0., 0., 3.),
            Vec3::ZERO,
            Vec3::Y,
            CameraProjection::isometric(2.5, 0.1, 10.).unwrap(),
        )
        .unwrap();
    let geometry: Geometry = Primitive::sphere(0.8, 48, 32, Color::WHITE).unwrap().into();
    for kind in ["force-field", "water", "dissolve"] {
        let definition = MaterialDefinition {
            shader: Some(format!("assets/shaders/{kind}.shader.toml")),
            cast_shadows: false,
            ..Default::default()
        };
        let material = definition.clone().resolve(&project).unwrap();
        let handle = renderer.insert_with_material(&geometry, &material).unwrap();
        let center = if kind == "force-field" {
            Vec3::new(3., 0., 0.)
        } else {
            Vec3::ZERO
        };
        renderer
            .set_camera(
                center + Vec3::new(0., 0., 3.),
                center,
                Vec3::Y,
                CameraProjection::isometric(2.5, 0.1, 10.).unwrap(),
            )
            .unwrap();
        let instance = GeometryInstance::new(handle, Mat4::from_translation(center));
        renderer.set_shader_time(0.);
        let first = renderer.render_lit_rgba(&[instance], &[]).unwrap().rgba;
        if kind == "force-field" {
            assert!(
                first[(48 * 96 + 48) * 4 + 2] < 220,
                "translated shield centre should remain translucent, not become a bright rim"
            );
        }
        renderer.set_shader_time(0.7);
        let second = renderer.render_lit_rgba(&[instance], &[]).unwrap().rgba;
        if kind != "dissolve" {
            assert_ne!(first, second, "{kind} must animate in native rendering");
        }
        if kind == "dissolve" {
            let mut definition = definition;
            definition.shader_parameters.insert("progress".into(), 1.);
            let hidden = definition.resolve(&project).unwrap();
            renderer
                .replace_material(handle, &geometry, &hidden)
                .unwrap();
            assert!(renderer.contains(handle));
            let invisible = renderer.render_lit_rgba(&[instance], &[]).unwrap().rgba;
            let empty = renderer.render_lit_rgba(&[], &[]).unwrap().rgba;
            // Metal may round an sRGB attachment clear one code point differently after a draw.
            assert!(
                invisible
                    .iter()
                    .zip(&empty)
                    .all(|(&a, &b)| a.abs_diff(b) <= 1),
                "fully dissolved object must reveal the background"
            );
            assert!(
                first.iter().zip(&empty).any(|(&a, &b)| a.abs_diff(b) > 10),
                "partly dissolved object must remain visible"
            );
        }
        assert!(renderer.remove(handle));
        assert!(!renderer.contains(handle));
        assert!(renderer
            .replace_material(handle, &geometry, &material)
            .is_err());
    }
    assert!(!renderer.set_shader_time(f32::NAN));
}
