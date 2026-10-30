use emergent::{
    Camera, CameraProjection, Color, Geometry, Primitive, Project, SceneDefinition, Vec3, World,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
const CUBE: &str = "schema_version = 1\nid = 'cube'\nkind = 'primitive'\n[transform]\ntranslation = [2.0, 0.0, 0.0]\n[geometry]\nprimitive = 'cube'\nhalf_extent = 0.75\ncolor = [1.0, 0.0, 0.0, 1.0]\n";

#[test]
fn raymarched_scene_assets_use_world_identity_and_reject_fake_collision() {
    let f = Fixture::new();
    let recipe="schema_version=1\nid='field'\nkind='raymarched-terrain'\n[raymarched_terrain]\nseed=42\nsize=32\n";
    f.asset(recipe);
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    assert!(loaded.renderables.is_empty());
    for id in loaded.entities.values() {
        assert_eq!(
            world
                .component::<emergent::RaymarchedTerrain>(*id)
                .unwrap()
                .unwrap()
                .size,
            32.
        );
    }
    assert_ne!(loaded.entities["parent"], loaded.entities["child"]);
    for extra in [
        "\n[physics]\nenabled=false\nbody='static'\n",
        "\n[material]\nalbedo=[1,1,1]\n",
        "\n[geometry]\nprimitive='cube'\nhalf_extent=1\ncolor=[1,1,1,1]\n",
    ] {
        f.asset(&format!("{recipe}{extra}"));
        assert!(f.load().is_err());
    }
    f.asset(&recipe.replace("size=32", "size=128"));
    assert!(f.load().is_err());
    f.asset(recipe);
    let instances = (0..9)
        .map(|i| format!("[[entities]]\nid='f{i}'\nasset='assets/cube.entity.toml'\n"))
        .collect::<String>();
    f.scene(&instances);
    assert!(f
        .load()
        .err()
        .unwrap()
        .to_string()
        .contains("eight ray-marched"));
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "emergent-scene-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("src/generated")).unwrap();
        fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
        let f = Self(root);
        f.scene("[[entities]]\nid = 'parent'\nasset = 'assets/cube.entity.toml'\n[[entities]]\nid = 'child'\nasset = 'assets/cube.entity.toml'\nparent = 'parent'\n");
        f.asset(CUBE);
        f
    }
    fn asset(&self, source: &str) {
        fs::write(self.0.join("assets/cube.entity.toml"), source).unwrap();
    }
    fn scene(&self, source: &str) {
        fs::write(
            self.0.join("assets/main.scene.toml"),
            format!("schema_version = 1\nid = 'main'\n{source}"),
        )
        .unwrap();
    }
    fn load(&self) -> Result<SceneDefinition, emergent::SceneError> {
        SceneDefinition::load(&Project::load(&self.0).unwrap(), "assets/main.scene.toml")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn asset_values_multiple_instances_and_parent_transforms_drive_construction() {
    let f = Fixture::new();
    let mut world = World::new();
    let first = f.load().unwrap().instantiate(&mut world).unwrap();
    world.propagate_transforms();
    assert_eq!(
        world
            .global_transform(first.entities["child"])
            .unwrap()
            .translation(),
        Vec3::new(4., 0., 0.)
    );
    assert_eq!(
        first.renderables[0].1,
        Geometry::from(Primitive::cube(0.75, Color::new(1., 0., 0., 1.).unwrap()).unwrap())
    );
    f.asset(&CUBE.replace("2.0, 0.0", "3.0, 0.0").replace("0.75", "0.25"));
    let next = f.load().unwrap().instantiate(&mut world).unwrap();
    world.propagate_transforms();
    assert_ne!(first.entities["parent"], next.entities["parent"]);
    assert_eq!(
        world
            .global_transform(next.entities["child"])
            .unwrap()
            .translation(),
        Vec3::new(6., 0., 0.)
    );
    assert_ne!(first.renderables[0].1, next.renderables[0].1);
    world.remove_subtree(first.root).unwrap();
    assert!(world.get(first.entities["child"]).is_err());
    assert!(world.get(next.entities["child"]).is_ok());
}

#[test]
fn instance_transform_vectors_override_asset_defaults_without_changing_shared_assets() {
    let f = Fixture::new();
    f.asset(&CUBE.replace("[geometry]", "scale = [2.0, 3.0, 4.0]\n[geometry]"));
    f.scene("[[entities]]\nid='parent'\nasset='assets/cube.entity.toml'\n[[entities]]\nid='child'\nasset='assets/cube.entity.toml'\nparent='parent'\n[entities.transform]\ntranslation=[5.0,6.0,7.0]\nrotation_degrees=[0.0,0.0,90.0]");
    let mut world = World::new();
    let scene = f.load().unwrap().instantiate(&mut world).unwrap();
    world.propagate_transforms();
    let parent = world.local_transform(scene.entities["parent"]).unwrap();
    assert_eq!(parent.translation, Vec3::new(2., 0., 0.));
    let child = world.local_transform(scene.entities["child"]).unwrap();
    assert_eq!(child.translation, Vec3::new(5., 6., 7.));
    assert_eq!(child.scale, Vec3::new(2., 3., 4.));
    assert!((child.rotation * Vec3::X).abs_diff_eq(Vec3::Y, 1e-5));
    assert_eq!(
        world
            .global_transform(scene.entities["child"])
            .unwrap()
            .translation(),
        Vec3::new(12., 18., 28.)
    );
    assert!(fs::read_to_string(f.0.join("assets/cube.entity.toml"))
        .unwrap()
        .contains("translation = [2.0, 0.0, 0.0]"));
}

#[test]
fn explicit_active_camera_resolves_to_a_typed_world_component() {
    let f = Fixture::new();
    fs::write(
        f.0.join("assets/camera.entity.toml"),
        "schema_version=1\nid='camera-asset'\nkind='camera'\n[transform]\ntranslation=[7.0,5.0,9.0]\nrotation_degrees=[-20.0,38.0,0.0]\nscale=[1.0,1.0,1.0]\n[camera]\nprojection='perspective'\nfield_of_view_degrees=60.0\nnear=0.1\nfar=1000.0\n",
    )
    .unwrap();
    f.scene(
        "active_camera='camera'\n[[entities]]\nid='camera'\nasset='assets/camera.entity.toml'\n",
    );
    let mut world = World::new();
    let scene = f.load().unwrap().instantiate(&mut world).unwrap();
    let camera = scene.active_camera.unwrap();
    assert_eq!(scene.cameras, vec![camera]);
    assert!(matches!(
        world
            .component::<Camera>(camera)
            .unwrap()
            .unwrap()
            .projection(),
        CameraProjection::Perspective { .. }
    ));
    f.scene("active_camera='camera'\n[[entities]]\nid='camera'\nasset='assets/camera.entity.toml'\n[entities.camera]\nprojection='orthographic'\nview_size=8.0\nnear=0.5\nfar=12.0\n[[entities]]\nid='other'\nasset='assets/camera.entity.toml'\n");
    let override_scene = f.load().unwrap().instantiate(&mut world).unwrap();
    assert_eq!(
        world
            .component::<Camera>(override_scene.entities["camera"])
            .unwrap()
            .unwrap()
            .projection(),
        CameraProjection::isometric(8., 0.5, 12.).unwrap()
    );
    assert_eq!(
        world
            .component::<Camera>(override_scene.entities["other"])
            .unwrap()
            .unwrap()
            .projection(),
        CameraProjection::perspective(60., 0.1, 1000.).unwrap()
    );
    f.scene("[[entities]]\nid='bad'\nasset='assets/camera.entity.toml'\n[entities.camera]\nprojection='perspective'\nfield_of_view_degrees=60.0\nnear=10.0\nfar=1.0\n");
    assert!(f.load().is_err());
    f.scene("[[entities]]\nid='bad'\nasset='assets/cube.entity.toml'\n[entities.camera]\nprojection='isometric'\nview_size=8.0\nnear=0.5\nfar=12.0\n");
    assert!(f
        .load()
        .err()
        .unwrap()
        .to_string()
        .contains("Camera overrides require a camera asset"));
    f.scene(
        "active_camera='missing'\n[[entities]]\nid='camera'\nasset='assets/camera.entity.toml'\n",
    );
    assert!(f
        .load()
        .err()
        .unwrap()
        .to_string()
        .contains("active camera 'missing' is missing"));
}

#[test]
fn candidate_validation_and_reset_do_not_modify_authored_files() {
    let f = Fixture::new();
    let project = Project::load(&f.0).unwrap();
    let path = "assets/main.scene.toml";
    let original = fs::read_to_string(f.0.join(path)).unwrap();
    let candidate = format!("{original}\n[entities.transform]\nscale=[0.0,1.0,1.0]\n");
    assert!(SceneDefinition::from_source(&project, path, &candidate).is_err());
    assert_eq!(fs::read_to_string(f.0.join(path)).unwrap(), original);
    let candidate = candidate.replace("0.0,1.0,1.0", "2.0,2.0,2.0");
    let mut world = World::new();
    let edited = SceneDefinition::from_source(&project, path, &candidate)
        .unwrap()
        .instantiate(&mut world)
        .unwrap();
    assert_eq!(
        world
            .local_transform(edited.entities["child"])
            .unwrap()
            .scale,
        Vec3::splat(2.)
    );
    let reset = f.load().unwrap().instantiate(&mut world).unwrap();
    assert_eq!(
        world
            .local_transform(reset.entities["child"])
            .unwrap()
            .scale,
        Vec3::ONE
    );
}

#[test]
fn invalid_graphs_and_paths_fail_before_mutating_a_world() {
    let f = Fixture::new();
    let mut world = World::new();
    let valid = f.load().unwrap().instantiate(&mut world).unwrap();
    let count = world.len();
    for (source, expected) in [
        ("[[entities]]\nid='x'\nasset='assets/cube.entity.toml'\n[[entities]]\nid='x'\nasset='assets/cube.entity.toml'", "duplicate"),
        ("[[entities]]\nid='x'\nasset='assets/cube.entity.toml'\nparent='missing'", "missing parent"),
        ("[[entities]]\nid='x'\nasset='assets/cube.entity.toml'\nparent='y'\n[[entities]]\nid='y'\nasset='assets/cube.entity.toml'\nparent='x'", "cycle"),
        ("[[entities]]\nid='x'\nasset='../escape.toml'", "project-relative"),
        ("[[entities]]\nid='x'\nasset='assets/missing.toml'", "does not exist"),
    ] {
        f.scene(source);
        assert!(f.load().err().unwrap().to_string().contains(expected),"{expected}");
        assert_eq!(world.len(),count);
        assert!(world.get(valid.root).is_ok());
    }
}

#[test]
fn unsupported_schemas_fields_and_bad_values_are_rejected() {
    let f = Fixture::new();
    for (asset, expected) in [
        (
            CUBE.replace("schema_version = 1", "schema_version = 99"),
            "schema_version",
        ),
        (
            CUBE.replace("half_extent = 0.75", "half_extent = -1.0"),
            "dimension",
        ),
        (
            CUBE.replace("half_extent = 0.75", "half_extent = 3e38"),
            "dimension",
        ),
        (
            CUBE.replace(
                "translation = [2.0, 0.0, 0.0]",
                "translation = [nan, 0.0, 0.0]",
            ),
            "finite",
        ),
        (
            CUBE.replace("[geometry]", "scale = [0.0, 1.0, 1.0]\n[geometry]"),
            "scale",
        ),
        (
            CUBE.replace("primitive = 'cube'", "primitive = 'glb'"),
            "variant",
        ),
        (
            CUBE.replace(
                "half_extent = 0.75",
                "half_extent = 0.75\nunrecognized = true",
            ),
            "unknown field",
        ),
    ] {
        f.asset(&asset);
        let diagnostic = f.load().err().unwrap().to_string();
        assert!(diagnostic.contains(expected), "{diagnostic}");
        assert!(diagnostic.contains("cube.entity.toml"));
    }
    f.scene("entities = []\nunknown = true");
    assert!(f
        .load()
        .err()
        .unwrap()
        .to_string()
        .contains("unknown field"));
}

#[test]
fn lights_are_validated_and_attached_as_components() {
    let f = Fixture::new();
    let source = "schema_version=1\nid='light'\nkind='directional-light'\n[light]\ntype='directional'\ncolor=[1.0,1.0,1.0]\nintensity=3.0\ncasts_shadows=true";
    f.asset(source);
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    assert!(loaded.renderables.is_empty());
    assert_eq!(loaded.lights.len(), 2);
    let light = world
        .component::<emergent::LightSource>(loaded.lights[0])
        .unwrap()
        .unwrap();
    assert_eq!(light.kind(), emergent::LightKind::Directional);
    assert_eq!(light.shadow_casting(), emergent::ShadowCasting::Enabled);
    f.asset(&source.replace("intensity=3.0", "intensity=-3.0"));
    assert!(f.load().is_err());
}

#[test]
fn light_components_override_one_instance_disable_and_reject_bad_entries() {
    let f = Fixture::new();
    f.asset("schema_version=1\nid='lamp'\nkind='point-light'\n[light]\ntype='point'\ncolor=[1.0,1.0,1.0]\nintensity=8.0\nrange=24.0\n");
    let scene = |component: &str| {
        f.scene(&format!("[[entities]]\nid='a'\nasset='assets/cube.entity.toml'\n{component}[[entities]]\nid='b'\nasset='assets/cube.entity.toml'\n"));
    };
    scene("[[entities.components]]\ntype='emergent.light'\ncolor=[1.0,0.0,0.0]\nintensity=2\n");
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    let light = |id: &str| {
        world
            .component::<emergent::LightSource>(loaded.entities[id])
            .unwrap()
    };
    let a = light("a").unwrap();
    assert_eq!(
        (a.intensity(), a.range(), a.color()),
        (2.0, Some(24.0), Color::new(1., 0., 0., 1.).unwrap())
    );
    assert_eq!(
        light("b").unwrap().intensity(),
        8.0,
        "the shared asset is unchanged"
    );
    scene("[[entities.components]]\ntype='emergent.light'\nenabled=false\n");
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    assert!(world
        .component::<emergent::LightSource>(loaded.entities["a"])
        .unwrap()
        .is_none());
    assert_eq!(loaded.lights.len(), 1);
    for (component, message) in [
        ("type='emergent.wobble'\n", "unknown component type"),
        ("type='emergent.light'\nbrightness=2\n", "no field"),
        ("type='emergent.light'\nintensity=-1\n", "must be a number"),
        (
            "type='emergent.light'\ninner_half_angle=0.2\n",
            "cone angles",
        ),
    ] {
        scene(&format!("[[entities.components]]\n{component}"));
        let error = f.load().err().unwrap().to_string();
        assert!(error.contains(message), "{error}");
    }
    scene("[[entities.components]]\ntype='emergent.light'\n[[entities.components]]\ntype='emergent.light'\n");
    assert!(f
        .load()
        .err()
        .unwrap()
        .to_string()
        .contains("more than once"));
    f.asset(CUBE);
    scene("[[entities.components]]\ntype='emergent.light'\n");
    assert!(f
        .load()
        .err()
        .unwrap()
        .to_string()
        .contains("requires a light asset"));
}

#[test]
fn file_instance_light_and_subdivision_limits_are_enforced() {
    let f = Fixture::new();
    f.asset(&" ".repeat(256 * 1024 + 1));
    assert!(f.load().err().unwrap().to_string().contains("256 KiB"));
    f.asset(CUBE);
    let mut entries = String::new();
    for n in 0..1025 {
        entries.push_str(&format!(
            "[[entities]]\nid='e{n}'\nasset='assets/cube.entity.toml'\n"
        ));
    }
    f.scene(&entries);
    assert!(f.load().err().unwrap().to_string().contains("1024"));
    f.scene("[[entities]]\nid='one'\nasset='assets/cube.entity.toml'");
    f.asset(&CUBE.replace(
        "primitive = 'cube'\nhalf_extent = 0.75",
        "primitive = 'sphere'\nradius=1.0\nsectors=512\nstacks=512",
    ));
    assert!(f.load().err().unwrap().to_string().contains("128 sectors"));
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_rejected() {
    let f = Fixture::new();
    let outside = Fixture::new();
    fs::remove_file(f.0.join("assets/cube.entity.toml")).unwrap();
    std::os::unix::fs::symlink(
        outside.0.join("assets/cube.entity.toml"),
        f.0.join("assets/cube.entity.toml"),
    )
    .unwrap();
    assert!(f.load().is_err());
}

#[test]
fn every_primitive_and_light_template_loads_and_deep_transform_overflow_fails() {
    let f = Fixture::new();
    f.scene("[[entities]]\nid='one'\nasset='assets/cube.entity.toml'");
    for geometry in [
        "primitive='plane'\nhalf_width=1.0\nhalf_depth=1.0",
        "primitive='pyramid'\nhalf_extent=0.5\nheight=1.0",
        "primitive='sphere'\nradius=0.5\nsectors=24\nstacks=12",
        "primitive='cylinder'\nradius=0.5\nhalf_height=0.5\nsegments=24",
        "primitive='cone'\nradius=0.5\nheight=1.0\nsegments=24",
        "primitive='capsule'\nradius=0.5\nhalf_segment=0.5\nsegments=24\nhemisphere_rings=8",
        "primitive='torus'\nmajor_radius=1.0\nminor_radius=0.25\nmajor_segments=32\nminor_segments=16",
    ] {
        f.asset(&format!("schema_version=1\nid='object'\nkind='primitive'\n[geometry]\n{geometry}\ncolor=[1.0,1.0,1.0,1.0]"));
        assert_eq!(f.load().unwrap().instantiate(&mut World::new()).unwrap().renderables.len(),1);
    }
    for (kind, parameters) in [
        ("ambient", ""),
        ("directional", ""),
        ("point", "range=24.0"),
        (
            "spot",
            "range=24.0\ninner_half_angle=0.25\nouter_half_angle=0.5",
        ),
    ] {
        f.asset(&format!("schema_version=1\nid='light'\nkind='{kind}-light'\n[light]\ntype='{kind}'\ncolor=[1.0,1.0,1.0]\nintensity=1.0\n{parameters}"));
        assert!(f.load().is_ok(), "{kind}");
    }
    let mut entries = String::new();
    for n in 0..9 {
        entries.push_str(&format!(
            "[[entities]]\nid='l{n}'\nasset='assets/cube.entity.toml'\n"
        ));
    }
    f.scene(&entries);
    assert!(f.load().err().unwrap().to_string().contains("eight lights"));
    f.scene("[[entities]]\nid='one'\nasset='assets/cube.entity.toml'\n[[entities]]\nid='two'\nasset='assets/cube.entity.toml'\nparent='one'");
    f.asset("schema_version=1\nid='node'\nkind='entity'\n[transform]\nscale=[1e30,1e30,1e30]");
    assert!(f.load().err().unwrap().to_string().contains("overflowing"));
}

#[cfg(feature = "physics")]
#[test]
fn physics_defaults_fitting_overrides_and_hierarchy_validation() {
    let f = Fixture::new();
    f.asset(&format!("{CUBE}\n[physics]\nbody='static'\n"));
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    let settings = world
        .component::<emergent::PhysicsBody>(loaded.entities["parent"])
        .unwrap()
        .unwrap();
    assert_eq!(settings.collider.as_box().unwrap().half_extents, [0.75; 3]);
    assert_eq!(settings.collider.as_box().unwrap().center, [0.0; 3]);
    f.scene("[[entities]]\nid='parent'\nasset='assets/cube.entity.toml'\n[entities.physics]\nenabled=false\n[[entities]]\nid='child'\nasset='assets/cube.entity.toml'\n[entities.physics]\nbody='dynamic'\n[entities.physics.collider]\nshape='box'\ncenter=[0.0,0.5,0.0]\nhalf_extents=[0.5,0.25,1.0]\n");
    let mut overridden_world = World::new();
    let scene = f
        .load()
        .unwrap()
        .instantiate(&mut overridden_world)
        .unwrap();
    assert!(overridden_world
        .component::<emergent::PhysicsBody>(scene.entities["parent"])
        .unwrap()
        .is_none());
    let child = overridden_world
        .component::<emergent::PhysicsBody>(scene.entities["child"])
        .unwrap()
        .unwrap();
    assert_eq!(child.body, emergent::BodyKind::Dynamic);
    assert_eq!(
        child.collider.as_box().unwrap().half_extents,
        [0.5, 0.25, 1.0]
    );
    assert_eq!(child.collider.as_box().unwrap().center, [0.0, 0.5, 0.0]);
    f.scene("[[entities]]\nid='parent'\nasset='assets/cube.entity.toml'\n[[entities]]\nid='child'\nasset='assets/cube.entity.toml'\nparent='parent'\n[entities.physics]\nbody='dynamic'\n");
    assert!(f.load().err().unwrap().to_string().contains("scene root"));
}

#[cfg(not(feature = "physics"))]
#[test]
fn enabled_physics_requires_feature() {
    let f = Fixture::new();
    f.asset(&format!("{CUBE}\n[physics]\nbody='static'\n"));
    assert!(f
        .load()
        .err()
        .unwrap()
        .message
        .contains("physics Cargo feature"));
    f.asset(&format!("{CUBE}\n[physics]\nenabled=false\n"));
    assert!(f.load().is_ok());
}

#[test]
fn materials_resolve_noise_defaults_complete_overrides_and_reject_invalid_references() {
    let f = Fixture::new();
    let noise = emergent::NoiseDefinition {
        schema_version: 1,
        id: "grain".into(),
        name: "Grain".into(),
        kind: "noise-function".into(),
        template: "procedural.noise".into(),
        runtime_status: "preview".into(),
        noise: emergent::NoiseSettings::default(),
    };
    fs::write(
        f.0.join("assets/grain.noise.toml"),
        toml::to_string(&noise).unwrap(),
    )
    .unwrap();
    let source=format!("{CUBE}\n[material]\nshading='smooth'\nalbedo=[1.0,0.0,0.0]\n[material.bump_map]\nasset='assets/grain.noise.toml'\n");
    f.asset(&source);
    let mut world = World::new();
    let scene = f.load().unwrap().instantiate(&mut world).unwrap();
    let inherited = world
        .component::<emergent::Material>(scene.entities["child"])
        .unwrap()
        .unwrap();
    assert_eq!(
        inherited.definition().shading,
        emergent::MaterialShading::Smooth
    );
    assert_eq!(inherited.definition().albedo, Some([1.0, 0.0, 0.0]));
    assert!(inherited.noise_settings()[3].is_some());
    f.scene("[[entities]]\nid='only'\nasset='assets/cube.entity.toml'\n[entities.material]\nshading='unlit'\nemission=[0.0,1.0,0.0]\nemission_intensity=2.0\n");
    let override_scene = f.load().unwrap().instantiate(&mut world).unwrap();
    let overridden = world
        .component::<emergent::Material>(override_scene.entities["only"])
        .unwrap()
        .unwrap();
    assert_eq!(
        overridden.definition().shading,
        emergent::MaterialShading::Unlit
    );
    assert!(overridden.noise_settings().iter().all(Option::is_none));
    assert_eq!(overridden.definition().albedo, None);
    f.scene("[[entities]]\nid='only'\nasset='assets/cube.entity.toml'\n");
    f.asset(&source.replace("assets/grain.noise.toml", "assets/missing.noise.toml"));
    assert!(f
        .load()
        .err()
        .unwrap()
        .message
        .contains("missing.noise.toml"));
    f.asset("schema_version=1\nid='empty'\nkind='entity'\n[material]\nshading='flat'\n");
    assert!(f.load().err().unwrap().message.contains("renderable"));
}

#[test]
fn authored_high_resolution_spheres_remain_bounded() {
    let f = Fixture::new();
    for (sectors, stacks, valid) in [
        (64, 32, true),
        (128, 64, true),
        (129, 64, false),
        (128, 65, false),
    ] {
        f.asset(&CUBE.replace(
            "primitive = 'cube'\nhalf_extent = 0.75",
            &format!("primitive = 'sphere'\nradius=1.0\nsectors={sectors}\nstacks={stacks}"),
        ));
        assert_eq!(f.load().is_ok(), valid);
    }
}

#[test]
fn material_bake_budget_rejects_expensive_map_combinations_before_construction() {
    let f = Fixture::new();
    let noise = emergent::NoiseDefinition {
        schema_version: 1,
        id: "grain".into(),
        name: "Grain".into(),
        kind: "noise-function".into(),
        template: "procedural.noise".into(),
        runtime_status: "preview".into(),
        noise: emergent::NoiseSettings::default(),
    };
    fs::write(
        f.0.join("assets/grain.noise.toml"),
        toml::to_string(&noise).unwrap(),
    )
    .unwrap();
    f.asset(&format!("{CUBE}\n[material]\nnoise_resolution=128\n[material.bump_map]\nasset='assets/grain.noise.toml'\n[material.albedo_map]\nasset='assets/grain.noise.toml'\n"));
    assert!(f
        .load()
        .err()
        .unwrap()
        .message
        .contains("octave evaluations"));
}

#[test]
fn instance_names_override_shared_names_without_changing_identity() {
    let f = Fixture::new();
    f.asset(&CUBE.replace(
        "kind = 'primitive'",
        "name = 'Shared cube'\nkind = 'primitive'",
    ));
    f.scene("[[entities]]\nid = 'parent'\nname = 'Local cube'\nasset = 'assets/cube.entity.toml'\n[[entities]]\nid = 'child'\nasset = 'assets/cube.entity.toml'\nparent = 'parent'\n");
    let mut world = World::new();
    let built = f.load().unwrap().instantiate(&mut world).unwrap();
    assert_eq!(
        world.get(built.entities["parent"]).unwrap().name(),
        "Local cube"
    );
    assert_eq!(
        world.get(built.entities["child"]).unwrap().name(),
        "Shared cube"
    );
    assert_eq!(
        world.parent(built.entities["child"]).unwrap(),
        Some(built.entities["parent"])
    );
    for name in ["", " ", "bad\nname"] {
        f.scene(&format!(
            "[[entities]]\nid='parent'\nname={}\nasset='assets/cube.entity.toml'\n",
            toml::Value::String(name.into())
        ));
        assert!(f.load().err().unwrap().message.contains("instance name"));
    }
}

#[test]
fn terrain_asset_loads_as_mesh_and_rejects_inconsistent_colliders() {
    let f = Fixture::new();
    let settings = emergent::TerrainSettings {
        resolution: 32,
        ..Default::default()
    };
    let source = format!(
        "schema_version=1\nid='terrain'\nkind='terrain'\n[terrain]\n{}",
        toml::to_string(&settings).unwrap()
    );
    f.asset(&source);
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    assert_eq!(loaded.renderables.len(), 2);
    assert!(matches!(&loaded.renderables[0].1,Geometry::Mesh(mesh) if mesh.vertices().len()==1024));
    #[cfg(feature = "physics")]
    {
        let physics = "\n[physics]\nbody='static'\n[physics.collider]\nshape='terrain'\n";
        f.asset(&(source.clone() + physics));
        assert!(f.load().is_ok());
        f.asset(&(source.clone() + &physics.replace("'static'", "'dynamic'")));
        assert!(f.load().is_err());
        f.asset(&(CUBE.to_owned() + physics));
        assert!(f.load().is_err());
    }
}

#[test]
fn sculpted_terrain_loads_frozen_heights_and_rejects_invalid_snapshots() {
    let f = Fixture::new();
    let source = "schema_version=1\nid='sculpted'\nkind='terrain'\nterrain_heights='assets/sculpt.heights.toml'\n[terrain]\nresolution=32\nheight=20.0\n";
    f.asset(source);
    let path = f.0.join("assets/sculpt.heights.toml");
    let values: Vec<f32> = (0..1024).map(|i| (i % 32) as f32 / 2.).collect();
    fs::write(&path, format!("schema_version=1\nvalues={values:?}\n")).unwrap();
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    let Geometry::Mesh(mesh) = &loaded.renderables[0].1 else {
        panic!("expected terrain mesh")
    };
    for (vertex, height) in mesh.vertices().iter().zip(&values) {
        assert_eq!(vertex.position().y, *height);
    }
    for invalid in [
        "schema_version=2\nvalues=[]",
        "schema_version=1\nvalues=[1.0]",
        "schema_version=1\nvalues=[nan]",
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(f.load().is_err());
    }
    fs::write(
        &path,
        format!("schema_version=1\nvalues={:?}\n", vec![21.; 1024]),
    )
    .unwrap();
    assert!(f.load().is_err());
    f.asset(&source.replace("assets/sculpt.heights.toml", "../escape.toml"));
    assert!(f.load().is_err());
}

#[cfg(feature = "physics")]
#[test]
fn frozen_terrain_collision_uses_authored_heights_instead_of_regenerating_recipe() {
    struct Idle;
    impl emergent::Game for Idle {
        fn fixed_update(&mut self, _: &mut emergent::EngineContext<'_>) {}
    }
    let f = Fixture::new();
    f.scene("[[entities]]\nid='ground'\nasset='assets/cube.entity.toml'\n");
    f.asset("schema_version=1\nid='ground'\nkind='terrain'\nterrain_heights='assets/flat.heights.toml'\n[terrain]\nresolution=32\nheight=20.0\n[physics]\nbody='static'\n[physics.collider]\nshape='terrain'\n");
    fs::write(
        f.0.join("assets/flat.heights.toml"),
        format!("schema_version=1\nvalues={:?}\n", vec![5.; 1024]),
    )
    .unwrap();
    let mut engine = emergent::Engine::new(Default::default()).unwrap();
    f.load().unwrap().instantiate(engine.world_mut()).unwrap();
    let cube = engine.world_mut().spawn("falling cube");
    engine
        .world_mut()
        .set_local_transform(
            cube,
            emergent::Transform::from_translation(Vec3::new(0., 12., 0.)),
        )
        .unwrap();
    engine
        .world_mut()
        .insert_component(
            cube,
            emergent::PhysicsBody::new(
                emergent::BodyKind::Dynamic,
                emergent::BoxCollider {
                    center: [0.; 3],
                    half_extents: [0.5; 3],
                },
            )
            .unwrap(),
        )
        .unwrap();
    engine.run_ticks(&mut Idle, 240).unwrap();
    let y = engine
        .world()
        .global_transform(cube)
        .unwrap()
        .translation()
        .y;
    assert!(
        (y - 5.5).abs() < 0.03,
        "cube rested at {y}, expected the frozen 5m surface"
    );
}

#[test]
fn default_plane_is_one_metre_and_legacy_dimensions_still_load() {
    let f = Fixture::new();
    f.scene("[[entities]]\nid='plane'\nasset='assets/cube.entity.toml'\n");
    let source = "schema_version=1\nid='plane'\nkind='primitive'\n[geometry]\nprimitive='plane'\ncolor=[1.0,1.0,1.0,1.0]\n";
    for (fields, expected, world_size) in [
        ("", Vec3::new(1., 0., 1.), Vec3::new(1., 0., 1.)),
        (
            "half_width=6.0\nhalf_depth=6.0\n",
            Vec3::new(12., 0., 12.),
            Vec3::new(12., 0., 12.),
        ),
        (
            "\n[transform]\nscale=[12.0,1.0,12.0]\n",
            Vec3::new(1., 0., 1.),
            Vec3::new(12., 0., 12.),
        ),
    ] {
        f.asset(&(source.to_owned() + fields));
        let mut world = World::new();
        let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
        let Geometry::Primitive(primitive) = &loaded.renderables[0].1 else {
            panic!("expected plane")
        };
        let mesh = primitive.to_mesh().unwrap();
        let min = mesh
            .vertices()
            .iter()
            .map(|v| v.position())
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let max = mesh
            .vertices()
            .iter()
            .map(|v| v.position())
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        assert_eq!(max - min, expected);
        world.propagate_transforms();
        let matrix = world
            .global_transform(loaded.entities["plane"])
            .unwrap()
            .matrix();
        let min = mesh
            .vertices()
            .iter()
            .map(|v| matrix.transform_point3(v.position()))
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let max = mesh
            .vertices()
            .iter()
            .map(|v| matrix.transform_point3(v.position()))
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        assert_eq!(max - min, world_size);
    }
}

#[test]
fn shared_material_assets_resolve_and_reject_cycles_and_inline_overrides() {
    let f = Fixture::new();
    let library = "schema_version=1\nid='stone'\nname='Stone'\n[material]\nalbedo=[0.2,0.3,0.4]\nshading='smooth'\n";
    fs::write(f.0.join("assets/stone.material.toml"), library).unwrap();
    f.asset(&format!(
        "{CUBE}\n[material]\nasset='assets/stone.material.toml'\n"
    ));
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    assert_eq!(
        world
            .component::<emergent::Material>(loaded.entities["parent"])
            .unwrap()
            .unwrap()
            .definition()
            .albedo,
        Some([0.2, 0.3, 0.4])
    );
    f.asset(&format!(
        "{CUBE}\n[material]\nasset='assets/stone.material.toml'\nalbedo=[1.0,0.0,0.0]\n"
    ));
    assert!(f.load().is_err());
    f.asset(&format!(
        "{CUBE}\n[material]\nasset='assets/stone.material.toml'\n"
    ));
    fs::write(f.0.join("assets/stone.material.toml"),"schema_version=1\nid='cycle'\nname='Cycle'\n[material]\nasset='assets/stone.material.toml'\n").unwrap();
    assert!(f.load().is_err());
    fs::remove_file(f.0.join("assets/stone.material.toml")).unwrap();
    assert!(f.load().is_err());
}

#[test]
fn painted_terrain_validates_shape_materials_and_normalized_weights() {
    let f = Fixture::new();
    fs::write(
        f.0.join("assets/stone.material.toml"),
        "schema_version=1\nid='stone'\nname='Stone'\n[material]\nalbedo=[0.2,0.3,0.4]\n",
    )
    .unwrap();
    let paint = emergent::TerrainPaint {
        schema_version: 1,
        resolution: 32,
        materials: vec!["assets/stone.material.toml".into()],
        transitions: [2., 5., 9.],
        blend: 1.,
        weights: (0..32 * 32).flat_map(|_| [255, 0, 0, 0]).collect(),
    };
    fs::write(
        f.0.join("assets/ground.paint.toml"),
        toml::to_string(&paint).unwrap(),
    )
    .unwrap();
    let source="schema_version=1\nid='ground'\nkind='terrain'\nterrain_paint='assets/ground.paint.toml'\n[terrain]\nresolution=32\n";
    f.asset(source);
    assert!(f.load().is_ok());
    f.asset(&source.replace("resolution=32", "resolution=64"));
    assert!(f.load().is_err());
    f.asset(source);
    let mut invalid = paint.clone();
    invalid.weights[0] = 254;
    assert!(invalid.validate().is_err());
    invalid = paint.clone();
    invalid.weights[1] = 255;
    invalid.weights[0] = 0;
    assert!(invalid.validate().is_err());
    invalid = paint.clone();
    invalid.transitions = [5., 2., 9.];
    assert!(invalid.validate().is_err());
    invalid = paint;
    invalid.materials = vec!["assets/missing.material.toml".into()];
    fs::write(
        f.0.join("assets/ground.paint.toml"),
        toml::to_string(&invalid).unwrap(),
    )
    .unwrap();
    assert!(f.load().is_err());
    f.asset(&CUBE.replace(
        "[transform]",
        "terrain_paint='assets/ground.paint.toml'\n[transform]",
    ));
    assert!(f.load().is_err());
}

fn editable_structure() -> emergent::StructureDefinition {
    use emergent::{PolygonMesh, StructureDefinition, StructurePart, StructureTransform};
    StructureDefinition {
        schema_version: 1,
        id: "building".into(),
        name: "Building".into(),
        parts: vec![StructurePart {
            id: "base".into(),
            name: "Base".into(),
            parent: None,
            transform: StructureTransform::default(),
            mesh: PolygonMesh {
                vertices: vec![
                    [-0.5, 0., -0.5],
                    [0.5, 0., -0.5],
                    [0.5, 0., 0.5],
                    [-0.5, 0., 0.5],
                ],
                faces: vec![vec![0, 3, 2, 1]],
            },
        }],
    }
}
#[test]
fn structure_assets_compile_local_hierarchy_into_one_mesh_on_the_canonical_entity() {
    let f = Fixture::new();
    let mut structure = editable_structure();
    structure.parts[0].transform.translation = [2., 0., 0.];
    let mut child = structure.parts[0].clone();
    child.id = "roof".into();
    child.name = "Roof".into();
    child.parent = Some("base".into());
    child.transform.translation = [0., 3., 0.];
    structure.parts.push(child);
    let mesh = structure.mesh().unwrap();
    assert_eq!(mesh.vertices().len(), 8);
    assert_eq!(mesh.indices().len(), 12);
    assert_eq!(mesh.vertices()[4].position(), Vec3::new(1.5, 3., -0.5));
    fs::write(
        f.0.join("assets/building.structure.toml"),
        toml::to_string(&structure).unwrap(),
    )
    .unwrap();
    f.asset("schema_version=1\nid='building'\nkind='structure'\nstructure='assets/building.structure.toml'\n[transform]\ntranslation=[10.0,0.0,0.0]\n");
    f.scene("[[entities]]\nid='building'\nasset='assets/cube.entity.toml'\n");
    let mut world = World::new();
    let loaded = f.load().unwrap().instantiate(&mut world).unwrap();
    world.propagate_transforms();
    assert_eq!(loaded.entities.len(), 1);
    assert_eq!(loaded.renderables.len(), 1);
    assert_eq!(loaded.renderables[0].0, loaded.entities["building"]);
    assert_eq!(
        world
            .global_transform(loaded.entities["building"])
            .unwrap()
            .translation(),
        Vec3::new(10., 0., 0.)
    );
    assert_eq!(loaded.renderables[0].1, Geometry::from(mesh));
    f.asset("schema_version=1\nid='bad'\nkind='primitive'\nstructure='assets/building.structure.toml'\n");
    assert!(f.load().is_err());
}
#[test]
fn structure_topology_triangulation_and_hierarchy_failures_are_validated_before_instantiation() {
    let mut structure = editable_structure();
    let concave = emergent::PolygonMesh {
        vertices: vec![
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 2., 0.],
            [1., 1., 0.],
            [0., 2., 0.],
        ],
        faces: vec![vec![0, 1, 2, 3, 4]],
    };
    assert_eq!(
        concave.triangulate_face(0).unwrap(),
        vec![1, 2, 3, 0, 1, 3, 0, 3, 4]
    );
    structure.parts[0].mesh = concave;
    assert!(structure.mesh().is_ok());
    structure.parts[0].mesh.faces[0] = vec![0, 2, 1, 4];
    assert!(structure.mesh().is_err());
    structure = editable_structure();
    structure.parts[0].parent = Some("missing".into());
    assert!(structure.mesh().is_err());
    structure.parts[0].parent = Some("base".into());
    assert!(structure.mesh().is_err());
    structure = editable_structure();
    structure.parts.push(structure.parts[0].clone());
    assert!(structure.mesh().is_err());
    structure = editable_structure();
    structure.parts[0].mesh.faces[0] = vec![0, 99, 1];
    assert!(structure.mesh().is_err());
    structure = editable_structure();
    structure.parts[0].transform.scale[0] = 0.;
    assert!(structure.mesh().is_err());
    structure = editable_structure();
    structure.parts[0].mesh.vertices[0][0] = f32::NAN;
    assert!(structure.mesh().is_err());
    structure = editable_structure();
    structure.parts[0].transform.scale = [-2., 1., 1.];
    let mesh = structure.mesh().unwrap();
    let tri = &mesh.indices()[..3];
    let [a, b, c] = [0, 1, 2].map(|i| mesh.vertices()[tri[i] as usize].position());
    assert!(
        (b - a).cross(c - a).y > 0.,
        "mirrored parts retain outward winding"
    );
}

#[test]
fn particle_assets_resolve_components_and_enforce_scene_capacity() {
    let fixture = Fixture::new();
    fixture.asset("schema_version=1\nid='effect'\nname='Smoke'\nkind='particle-effect'\nparticles='assets/smoke.particle.toml'\n");
    let mut definition = emergent::ParticleDefinition {
        schema_version: 1,
        id: "smoke".into(),
        name: "Smoke".into(),
        emitters: vec![emergent::ParticleEmitter::default()],
    };
    fs::write(
        fixture.0.join("assets/smoke.particle.toml"),
        toml::to_string(&definition).unwrap(),
    )
    .unwrap();
    let mut world = World::new();
    let loaded = fixture.load().unwrap().instantiate(&mut world).unwrap();
    assert_eq!(loaded.renderables.len(), 0);
    assert!(world
        .component::<emergent::ParticleEffect>(loaded.entities["child"])
        .unwrap()
        .is_some());
    definition.emitters[0].max_particles = 60_000;
    fs::write(
        fixture.0.join("assets/smoke.particle.toml"),
        toml::to_string(&definition).unwrap(),
    )
    .unwrap();
    assert!(fixture
        .load()
        .err()
        .unwrap()
        .to_string()
        .contains("capacity"));
    definition.emitters[0].max_particles = 1000;
    definition.emitters[0].texture = Some("assets/missing.png".into());
    fs::write(
        fixture.0.join("assets/smoke.particle.toml"),
        toml::to_string(&definition).unwrap(),
    )
    .unwrap();
    assert!(fixture.load().is_err());
}
