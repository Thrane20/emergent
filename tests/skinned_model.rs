//! SKEL-1: skinned glTF import and GPU skinning, against a synthetic two-joint bar.
use emergent::ImportedModel;

/// A 0.3 m wide, 2 m tall bar in the XY plane: rows at y = 0, 1, 2. The bottom two rows
/// follow `hips` (at the origin), the top row `spine` (at y = 1). Skin joints are listed
/// child-first so import must reorder them.
fn bar_glb(with_weights: bool) -> Vec<u8> {
    let mut bin = Vec::new();
    let rows = [0.0f32, 1.0, 2.0];
    for y in rows {
        for x in [-0.15f32, 0.15] {
            for v in [x, y, 0.0] {
                bin.extend(v.to_le_bytes());
            }
        }
    }
    for _ in 0..6 {
        for v in [0.0f32, 0.0, 1.0] {
            bin.extend(v.to_le_bytes());
        }
    }
    // File joint order: 0 = spine, 1 = hips.
    for row in 0..3 {
        let joint: u16 = if row == 2 { 0 } else { 1 };
        for _ in 0..2 {
            for j in [joint, 0, 0, 0] {
                bin.extend(j.to_le_bytes());
            }
        }
    }
    for _ in 0..6 {
        for w in [1.0f32, 0.0, 0.0, 0.0] {
            bin.extend(w.to_le_bytes());
        }
    }
    for i in [0u16, 1, 3, 0, 3, 2, 2, 3, 5, 2, 5, 4] {
        bin.extend(i.to_le_bytes());
    }
    // Inverse bind matrices (column-major): spine at y = 1, hips at the origin.
    let spine = [
        1.0f32, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., -1., 0., 1.,
    ];
    let hips = [
        1.0f32, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    for v in spine.iter().chain(&hips) {
        bin.extend(v.to_le_bytes());
    }
    // "bend": spine turns 0° → 90° about Z over one second.
    for t in [0.0f32, 1.0] {
        bin.extend(t.to_le_bytes());
    }
    let half = std::f32::consts::FRAC_1_SQRT_2;
    for v in [0.0f32, 0., 0., 1., 0., 0., half, half] {
        bin.extend(v.to_le_bytes());
    }
    assert_eq!(bin.len(), 480);
    let weights = if with_weights {
        r#","WEIGHTS_0":3"#
    } else {
        ""
    };
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0,2]}}],
"nodes":[{{"name":"hips","children":[1]}},{{"name":"spine","translation":[0,1,0]}},{{"name":"bar","mesh":0,"skin":0}}],
"skins":[{{"joints":[1,0],"inverseBindMatrices":5}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1,"JOINTS_0":2{weights}}},"indices":4}}]}}],
"animations":[{{"name":"bend","samplers":[{{"input":6,"output":7}}],"channels":[{{"sampler":0,"target":{{"node":1,"path":"rotation"}}}}]}}],
"buffers":[{{"byteLength":480}}],
"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":72,"target":34962}},{{"buffer":0,"byteOffset":72,"byteLength":72,"target":34962}},{{"buffer":0,"byteOffset":144,"byteLength":48,"target":34962}},{{"buffer":0,"byteOffset":192,"byteLength":96,"target":34962}},{{"buffer":0,"byteOffset":288,"byteLength":24,"target":34963}},{{"buffer":0,"byteOffset":312,"byteLength":128}},{{"buffer":0,"byteOffset":440,"byteLength":8}},{{"buffer":0,"byteOffset":448,"byteLength":32}}],
"accessors":[{{"bufferView":0,"componentType":5126,"count":6,"type":"VEC3","min":[-0.15,0,0],"max":[0.15,2,0]}},{{"bufferView":1,"componentType":5126,"count":6,"type":"VEC3"}},{{"bufferView":2,"componentType":5123,"count":6,"type":"VEC4"}},{{"bufferView":3,"componentType":5126,"count":6,"type":"VEC4"}},{{"bufferView":4,"componentType":5123,"count":12,"type":"SCALAR"}},{{"bufferView":5,"componentType":5126,"count":2,"type":"MAT4"}},{{"bufferView":6,"componentType":5126,"count":2,"type":"SCALAR","min":[0],"max":[1]}},{{"bufferView":7,"componentType":5126,"count":2,"type":"VEC4"}}]}}"#
    );
    let mut json = json.into_bytes();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut glb = Vec::new();
    glb.extend(b"glTF");
    glb.extend(2u32.to_le_bytes());
    glb.extend(((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
    glb.extend((json.len() as u32).to_le_bytes());
    glb.extend(b"JSON");
    glb.extend(json);
    glb.extend((bin.len() as u32).to_le_bytes());
    glb.extend(b"BIN\0");
    glb.extend(bin);
    glb
}

#[test]
fn a_skinned_glb_imports_a_sorted_skeleton_and_remapped_influences() {
    let model = ImportedModel::from_glb(&bar_glb(true)).unwrap();
    let skeleton = model.skeleton.expect("skinned model has a skeleton");
    assert_eq!(skeleton.names(), ["hips", "spine"]);
    assert_eq!(skeleton.parents(), [None, Some(0)]);
    assert_eq!(skeleton.joint("spine"), Some(1));
    let skin = model.mesh.skin().expect("mesh carries its skin");
    let top: Vec<_> = skin.joints[4..].iter().map(|j| j[0]).collect();
    assert_eq!(top, [1, 1], "file joint 0 (spine) is skeleton joint 1");
    assert!(skin.joints[..4].iter().all(|j| j[0] == 0));
    for m in &skin.rest_palette {
        assert!(
            m.abs_diff_eq(emergent::Mat4::IDENTITY, 1e-6),
            "bind matches rest"
        );
    }
}

#[test]
fn glb_animations_import_as_clips_on_the_skeleton() {
    let model = ImportedModel::from_glb(&bar_glb(true)).unwrap();
    assert_eq!(model.clips.len(), 1);
    let clip = &model.clips[0];
    assert_eq!((clip.name(), clip.duration()), ("bend", 1.0));
    assert_eq!(
        clip.channels()[0].joint,
        1,
        "targets spine by skeleton index"
    );
}

/// A template-shaped project with the bar as a model asset, animated by `emergent.animator`.
struct Project(std::path::PathBuf);
impl Project {
    fn new(component: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "emergent-skin-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("assets/models")).unwrap();
        std::fs::create_dir_all(root.join("src/generated")).unwrap();
        std::fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
        std::fs::write(root.join("assets/models/bar.glb"), bar_glb(true)).unwrap();
        std::fs::write(
            root.join("assets/bar.entity.toml"),
            "schema_version = 1\nid = 'bar'\nkind = 'model'\nmodel = 'assets/models/bar.glb'\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/main.scene.toml"),
            format!("schema_version = 1\nid = 'main'\n[[entities]]\nid = 'bar'\nasset = 'assets/bar.entity.toml'\n{component}"),
        )
        .unwrap();
        Self(root)
    }
    fn load(&self) -> Result<emergent::SceneDefinition, emergent::SceneError> {
        emergent::SceneDefinition::load(
            &emergent::Project::load(&self.0).unwrap(),
            "assets/main.scene.toml",
        )
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn an_animator_component_plays_its_clip_on_the_fixed_tick() {
    struct Idle;
    impl emergent::Game for Idle {
        fn fixed_update(&mut self, _: &mut emergent::EngineContext<'_>) {}
    }
    let project = Project::new(
        "[[entities.components]]\ntype = 'emergent.animator'\nclip = 'bend'\nloop = false\n",
    );
    let mut engine = emergent::Engine::new(Default::default()).unwrap();
    let loaded = project
        .load()
        .unwrap()
        .instantiate(engine.world_mut())
        .unwrap();
    let bar = loaded.entities["bar"];
    let spine_tip = |engine: &emergent::Engine| {
        let animator = engine
            .world()
            .component::<emergent::Animator>(bar)
            .unwrap()
            .unwrap();
        animator.palette()[1].transform_point3(emergent::Vec3::new(0., 2., 0.))
    };
    assert!(spine_tip(&engine).abs_diff_eq(emergent::Vec3::new(0., 2., 0.), 1e-5));
    // Default fixed step is 1/60 s; two seconds clamps a non-looping one-second clip at its end.
    engine.run_ticks(&mut Idle, 120).unwrap();
    let tip = spine_tip(&engine);
    assert!(
        tip.abs_diff_eq(emergent::Vec3::new(-1., 1., 0.), 1e-4),
        "{tip}"
    );
}

#[test]
fn animator_components_need_a_skinned_model_and_a_known_clip() {
    let error = |component: &str| Project::new(component).load().err().map(|e| e.to_string());
    assert!(
        error("[[entities.components]]\ntype = 'emergent.animator'\nclip = 'run'\n")
            .is_some_and(|e| e.contains("no animation clip named"))
    );
    assert!(
        error("[[entities.components]]\ntype = 'emergent.animator'\n").is_none(),
        "empty clip plays the first"
    );
}

#[test]
fn skinned_meshes_without_weights_are_rejected() {
    let error = ImportedModel::from_glb(&bar_glb(false)).unwrap_err();
    assert!(error.to_string().contains("WEIGHTS_0"), "{error}");
}

#[cfg(feature = "graphics")]
#[test]
fn posing_a_joint_bends_the_rendered_mesh() {
    use emergent::{
        CameraProjection, GeometryInstance, GeometryRenderer, Mat4, MaterialDefinition, Quat, Vec3,
    };
    let model = ImportedModel::from_glb(&bar_glb(true)).unwrap();
    let skeleton = model.skeleton.unwrap();
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(96, 96)).unwrap();
    let material = MaterialDefinition {
        shading: emergent::MaterialShading::Unlit,
        ..Default::default()
    }
    .constant()
    .unwrap();
    let bar = renderer
        .insert_with_material(&model.mesh.clone().into(), &material)
        .unwrap();
    renderer
        .set_camera(
            Vec3::new(0., 1., 5.),
            Vec3::new(0., 1., 0.),
            Vec3::Y,
            CameraProjection::perspective(45., 0.1, 20.).unwrap(),
        )
        .unwrap();
    let view_projection = renderer.view_projection();
    // A directional light runs the skinned shadow pipeline too.
    let sun = emergent::LightInstance::new(
        emergent::LightSource::directional(emergent::Color::WHITE, 1.).unwrap(),
        Mat4::from_rotation_x(-1.),
    );
    let covered = |renderer: &mut GeometryRenderer, points: &[Vec3]| {
        let frame = renderer
            .render_lit_rgba(&[GeometryInstance::new(bar, Mat4::IDENTITY)], &[sun])
            .unwrap();
        let background = &frame.rgba[..4].to_vec();
        points
            .iter()
            .map(|&p| {
                let ndc = view_projection.project_point3(p);
                let x = ((ndc.x + 1.) * 0.5 * 96.) as usize;
                let y = ((1. - ndc.y) * 0.5 * 96.) as usize;
                let i = (y * 96 + x) * 4;
                &frame.rgba[i..i + 4] != background.as_slice()
            })
            .collect::<Vec<_>>()
    };
    // Lower half, upper half, and where the upper half lands after a 90° bend.
    let probes = [
        Vec3::new(0., 0.5, 0.),
        Vec3::new(0., 1.6, 0.),
        Vec3::new(-0.6, 1., 0.),
    ];
    assert_eq!(covered(&mut renderer, &probes), [true, true, false]);

    let mut pose = skeleton.rest().to_vec();
    pose[1].rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let mut palette = Vec::new();
    skeleton.palette(&pose, &mut palette).unwrap();
    renderer.set_skin_palette(bar, &palette).unwrap();
    assert_eq!(covered(&mut renderer, &probes), [true, false, true]);

    assert!(renderer.set_skin_palette(bar, &palette[..1]).is_err());
    let plain = renderer.insert(&emergent::Mesh::cube(0.5).unwrap().into());
    assert!(renderer.set_skin_palette(plain, &palette).is_err());
}
