//! glTF binary import against a real TRELLIS-generated model.
use emergent::ImportedModel;

const ROBOT_ARM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/models/robot-arm.glb"
);

#[test]
fn a_generated_glb_loads_its_mesh_texture_coordinates_and_base_colour_image() {
    let model = ImportedModel::from_glb(&std::fs::read(ROBOT_ARM).unwrap()).unwrap();
    assert_eq!(model.mesh.vertices().len(), 7596);
    assert_eq!(model.mesh.indices().len(), 26250);
    assert!(
        model.mesh.vertices().iter().all(|v| v.uv().is_some()),
        "every vertex has UVs"
    );
    let texture = model.texture.as_ref().expect("embedded base-colour PNG");
    assert_eq!((texture.width(), texture.height()), (1024, 1024));
}

#[test]
fn malformed_and_non_glb_input_is_rejected() {
    assert!(ImportedModel::from_glb(b"not a model").is_err());
    let mut truncated = std::fs::read(ROBOT_ARM).unwrap();
    truncated.truncate(truncated.len() / 2);
    assert!(ImportedModel::from_glb(&truncated).is_err());
}

#[cfg(feature = "graphics")]
#[test]
fn the_model_renders_with_its_texture() {
    use emergent::{GeometryInstance, GeometryRenderer, Mat4, Material, MaterialDefinition, Vec3};
    let model = ImportedModel::from_glb(&std::fs::read(ROBOT_ARM).unwrap()).unwrap();
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(128, 128)).unwrap();
    let material = Material::with_uv_image(
        MaterialDefinition {
            shading: emergent::MaterialShading::Unlit,
            ..Default::default()
        },
        model.texture.clone().unwrap(),
    )
    .unwrap();
    let textured = renderer
        .insert_with_material(&model.mesh.clone().into(), &material)
        .unwrap();
    let plain = renderer
        .insert_with_material(
            &model.mesh.clone().into(),
            &MaterialDefinition {
                shading: emergent::MaterialShading::Unlit,
                ..Default::default()
            }
            .constant()
            .unwrap(),
        )
        .unwrap();
    renderer
        .set_camera(
            Vec3::new(0., 0.2, 2.2),
            Vec3::ZERO,
            Vec3::Y,
            emergent::CameraProjection::perspective(45., 0.05, 20.).unwrap(),
        )
        .unwrap();
    let mut colours = |geometry| {
        let frame = renderer
            .render_lit_rgba(&[GeometryInstance::new(geometry, Mat4::IDENTITY)], &[])
            .unwrap();
        frame
            .rgba
            .chunks(4)
            .map(|p| [p[0], p[1], p[2]])
            .collect::<std::collections::HashSet<_>>()
            .len()
    };
    let (with_texture, without) = (colours(textured), colours(plain));
    assert!(
        with_texture > without * 4,
        "texture adds detail: {with_texture} vs {without} colours"
    );
}

mod scenes {
    use emergent::{
        Engine, EngineConfig, EngineContext, Game, Project, Quat, SceneDefinition, Vec3,
    };
    use std::{fs, path::PathBuf, time::Duration};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("emergent-model-{name}-{}", std::process::id()));
            fs::create_dir_all(root.join("assets/models")).unwrap();
            fs::create_dir_all(root.join("src/generated")).unwrap();
            fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
            fs::copy(super::ROBOT_ARM, root.join("assets/models/arm.glb")).unwrap();
            fs::write(
                root.join("assets/arm.entity.toml"),
                "schema_version=1\nid='arm'\nkind='model'\nmodel='assets/models/arm.glb'\n",
            )
            .unwrap();
            Self(root)
        }
        fn scene(&self, source: &str) -> Result<SceneDefinition, emergent::SceneError> {
            fs::write(
                self.0.join("assets/main.scene.toml"),
                format!("schema_version = 1\nid = 'main'\n{source}"),
            )
            .unwrap();
            SceneDefinition::load(&Project::load(&self.0).unwrap(), "assets/main.scene.toml")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    struct Idle;
    impl Game for Idle {
        fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
    }

    #[test]
    fn a_model_asset_loads_as_textured_renderable_geometry() {
        let f = Fixture::new("load");
        let scene = f
            .scene("[[entities]]\nid='arm'\nasset='assets/arm.entity.toml'\n")
            .unwrap();
        let mut world = emergent::World::new();
        let loaded = scene.instantiate(&mut world).unwrap();
        let arm = loaded.entities["arm"];
        assert!(loaded.renderables.iter().any(|(id, _)| *id == arm));
        let material = world.component::<emergent::Material>(arm).unwrap().unwrap();
        assert!(
            material.uv_mapped() && material.diffuse_image().is_some(),
            "renders with its own texture"
        );
        fs::write(
            f.0.join("assets/bad.entity.toml"),
            "schema_version=1\nid='bad'\nkind='primitive'\nmodel='assets/models/arm.glb'\n",
        )
        .unwrap();
        let error = f
            .scene("[[entities]]\nid='bad'\nasset='assets/bad.entity.toml'\n")
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("kind=model"), "{error}");
    }

    /// The helicopter case: a part spins about its own pivot while its parent moves.
    #[test]
    fn a_child_part_spins_about_its_pivot_under_a_moving_parent() {
        let f = Fixture::new("rotor");
        let scene = f.scene("[[entities]]\nid='body'\nasset='assets/arm.entity.toml'\n[[entities]]\nid='rotor'\nparent='body'\nasset='assets/arm.entity.toml'\n[entities.transform]\ntranslation=[0,1.5,0]\n[[entities.components]]\ntype='emergent.spin'\naxis=[0,1,0]\ndegrees_per_second=90\n").unwrap();
        let mut engine = Engine::new(EngineConfig {
            fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
            ..Default::default()
        })
        .unwrap();
        let loaded = scene.instantiate(engine.world_mut()).unwrap();
        let (body, rotor) = (loaded.entities["body"], loaded.entities["rotor"]);
        for _ in 0..60 {
            // Fly the body forward 5 cm per tick.
            let mut local = *engine.world().local_transform(body).unwrap();
            local.translation.z -= 0.05;
            engine.world_mut().set_local_transform(body, local).unwrap();
            engine.run_ticks(&mut Idle, 1).unwrap();
        }
        let global = engine.world().global_transform(rotor).unwrap();
        assert!(
            global
                .translation()
                .abs_diff_eq(Vec3::new(0., 1.5, -3.), 1e-3),
            "rotor rides on the body: {}",
            global.translation()
        );
        let turned = engine.world().local_transform(rotor).unwrap().rotation;
        assert!(
            turned.abs_diff_eq(Quat::from_rotation_y(90f32.to_radians()), 1e-3),
            "a quarter turn in one second: {turned}"
        );
        assert_eq!(
            engine.world().local_transform(body).unwrap().rotation,
            Quat::IDENTITY,
            "the body does not turn"
        );
    }
}
