//! SKEL-3: BVH import, humanoid detection and retargeting between unlike skeletons.
use emergent::{
    retarget, HumanoidBone, HumanoidMap, Mat4, Motion, Quat, Skeleton, Transform, Vec3,
};

/// A CMU-style walk: centimetres, T-pose, facing +Z (left is +X), an unmapped
/// `LHipJoint`, ZXY rotation channels. One second at 30 fps; hips travel 60 cm.
fn walk_bvh() -> String {
    fn joint(out: &mut String, name: &str, offset: [f32; 3], children: &[&dyn Fn(&mut String)]) {
        out.push_str(&format!(
            "JOINT {name}\n{{\nOFFSET {} {} {}\nCHANNELS 3 Zrotation Xrotation Yrotation\n",
            offset[0], offset[1], offset[2]
        ));
        if children.is_empty() {
            out.push_str("End Site\n{\nOFFSET 0 -5 5\n}\n");
        }
        for child in children {
            child(out);
        }
        out.push_str("}\n");
    }
    // Left leg hangs from an unmapped LHipJoint, as in CMU files.
    let left_leg = |out: &mut String| {
        let foot = |o: &mut String| joint(o, "LeftFoot", [0., -45., 0.], &[]);
        let low = |o: &mut String| joint(o, "LeftLeg", [0., -45., 0.], &[&foot]);
        let up = |o: &mut String| joint(o, "LeftUpLeg", [0., 0., 0.], &[&low]);
        joint(out, "LHipJoint", [10., 0., 0.], &[&up]);
    };
    let arm = |side: &'static str, s: f32| {
        move |out: &mut String| {
            let (sh, up, low, hand) = (
                format!("{side}Shoulder"),
                format!("{side}Arm"),
                format!("{side}ForeArm"),
                format!("{side}Hand"),
            );
            let hand = move |o: &mut String| joint(o, &hand, [25. * s, 0., 0.], &[]);
            let low = move |o: &mut String| joint(o, &low, [28. * s, 0., 0.], &[&hand]);
            let up = move |o: &mut String| joint(o, &up, [12. * s, 0., 0.], &[&low]);
            joint(out, &sh, [5. * s, 18., 0.], &[&up]);
        }
    };
    let mut h = String::from("HIERARCHY\nROOT Hips\n{\nOFFSET 0 0 0\nCHANNELS 6 Xposition Yposition Zposition Zrotation Xrotation Yrotation\n");
    left_leg(&mut h);
    // RightUpLeg sits at -10 cm directly under the root.
    h.push_str("JOINT RightUpLeg\n{\nOFFSET -10 0 0\nCHANNELS 3 Zrotation Xrotation Yrotation\n");
    joint(
        &mut h,
        "RightLeg",
        [0., -45., 0.],
        &[&|o: &mut String| joint(o, "RightFoot", [0., -45., 0.], &[])],
    );
    h.push_str("}\n");
    let left_arm = arm("Left", 1.);
    let right_arm = arm("Right", -1.);
    let head = |o: &mut String| joint(o, "Head", [0., 10., 0.], &[]);
    let neck = |o: &mut String| joint(o, "Neck", [0., 20., 0.], &[&head]);
    let spine1 =
        |o: &mut String| joint(o, "Spine1", [0., 20., 0.], &[&neck, &left_arm, &right_arm]);
    joint(&mut h, "Spine", [0., 10., 0.], &[&spine1]);
    h.push_str("}\n");
    // Channel order matches the hierarchy's depth-first joint order.
    let names: Vec<String> = h
        .lines()
        .filter_map(|l| l.strip_prefix("JOINT ").or_else(|| l.strip_prefix("ROOT ")))
        .map(str::to_string)
        .collect();
    h.push_str("MOTION\nFrames: 31\nFrame Time: 0.0333333\n");
    for f in 0..31 {
        let phase = f as f32 / 30. * std::f32::consts::TAU;
        let swing = 30. * phase.sin();
        let mut row = vec![0., 95., f as f32 * 2., 0., 0., 5. * phase.sin()];
        for name in &names[1..] {
            let (z, x, y) = match name.as_str() {
                "LeftUpLeg" => (0., swing, 0.),
                "RightUpLeg" => (0., -swing, 0.),
                "LeftLeg" | "RightLeg" => (0., 20. + 20. * phase.cos(), 0.),
                "LeftArm" => (-60., -swing, 0.),
                "RightArm" => (60., swing, 0.),
                "LeftForeArm" => (0., 0., -30.),
                "Spine1" => (0., 0., 10. * phase.sin()),
                _ => (0., 0., 0.),
            };
            row.extend([z, x, y]);
        }
        let row: Vec<String> = row.iter().map(|v| format!("{v:.4}")).collect();
        h.push_str(&row.join(" "));
        h.push('\n');
    }
    h
}

/// VRM-named rig: metres, A-pose (arms 45° down), facing −Z (left is −X), and a knee
/// whose local axes are twisted 90° about Y.
fn vrm_rig() -> Skeleton {
    let joints: &[(&str, Option<&str>, [f32; 3], Quat)] = &[
        ("hips", None, [0., 1.0, 0.], Quat::IDENTITY),
        ("leftUpperLeg", Some("hips"), [-0.1, 0., 0.], Quat::IDENTITY),
        (
            "leftLowerLeg",
            Some("leftUpperLeg"),
            [0., -0.45, 0.],
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        ),
        (
            "leftFoot",
            Some("leftLowerLeg"),
            [0., -0.45, 0.],
            Quat::IDENTITY,
        ),
        ("rightUpperLeg", Some("hips"), [0.1, 0., 0.], Quat::IDENTITY),
        (
            "rightLowerLeg",
            Some("rightUpperLeg"),
            [0., -0.45, 0.],
            Quat::IDENTITY,
        ),
        (
            "rightFoot",
            Some("rightLowerLeg"),
            [0., -0.45, 0.],
            Quat::IDENTITY,
        ),
        ("spine", Some("hips"), [0., 0.1, 0.], Quat::IDENTITY),
        ("chest", Some("spine"), [0., 0.2, 0.], Quat::IDENTITY),
        ("neck", Some("chest"), [0., 0.2, 0.], Quat::IDENTITY),
        ("head", Some("neck"), [0., 0.1, 0.], Quat::IDENTITY),
        (
            "leftShoulder",
            Some("chest"),
            [-0.05, 0.18, 0.],
            Quat::IDENTITY,
        ),
        (
            "leftUpperArm",
            Some("leftShoulder"),
            [-0.12, 0., 0.],
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
        ),
        (
            "leftLowerArm",
            Some("leftUpperArm"),
            [-0.28, 0., 0.],
            Quat::IDENTITY,
        ),
        (
            "leftHand",
            Some("leftLowerArm"),
            [-0.25, 0., 0.],
            Quat::IDENTITY,
        ),
        (
            "rightShoulder",
            Some("chest"),
            [0.05, 0.18, 0.],
            Quat::IDENTITY,
        ),
        (
            "rightUpperArm",
            Some("rightShoulder"),
            [0.12, 0., 0.],
            Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4),
        ),
        (
            "rightLowerArm",
            Some("rightUpperArm"),
            [0.28, 0., 0.],
            Quat::IDENTITY,
        ),
        (
            "rightHand",
            Some("rightLowerArm"),
            [0.25, 0., 0.],
            Quat::IDENTITY,
        ),
    ];
    let index = |name: &str| joints.iter().position(|j| j.0 == name).unwrap() as u16;
    let rest: Vec<Transform> = joints
        .iter()
        .map(|j| Transform::new(Vec3::from_array(j.2), j.3, Vec3::ONE))
        .collect();
    let parents: Vec<Option<u16>> = joints.iter().map(|j| j.1.map(index)).collect();
    let mut global: Vec<Mat4> = Vec::new();
    for (i, t) in rest.iter().enumerate() {
        let m = Mat4::from_rotation_translation(t.rotation, t.translation);
        global.push(parents[i].map_or(m, |p| global[p as usize] * m));
    }
    Skeleton::new(
        joints.iter().map(|j| j.0.to_string()).collect(),
        parents,
        rest,
        global.iter().map(Mat4::inverse).collect(),
    )
    .unwrap()
}

fn world_positions(skeleton: &Skeleton, clip: &emergent::AnimationClip, time: f32) -> Vec<Vec3> {
    let mut pose = skeleton.rest().to_vec();
    clip.sample(time, &mut pose);
    let mut global: Vec<Mat4> = Vec::new();
    for (i, t) in pose.iter().enumerate() {
        let m = Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation);
        global.push(skeleton.parents()[i].map_or(m, |p| global[p as usize] * m));
    }
    global.iter().map(|m| m.w_axis.truncate()).collect()
}

#[test]
fn bvh_parses_hierarchy_offsets_and_channels() {
    let motion = Motion::parse(&walk_bvh(), "walk").unwrap();
    assert_eq!(motion.skeleton.len(), 20, "LHipJoint is a joint too");
    assert_eq!(motion.skeleton.names()[0], "Hips");
    assert_eq!(motion.clip.name(), "walk");
    assert!((motion.clip.duration() - 1.0).abs() < 1e-4);
    let positions = world_positions(&motion.skeleton, &motion.clip, 0.0);
    let hips = motion.skeleton.joint("Hips").unwrap();
    assert!(positions[hips].abs_diff_eq(Vec3::new(0., 95., 0.), 1e-4));
    let map = HumanoidMap::detect(&motion.skeleton);
    assert!(map.missing().is_empty(), "{:?}", map.missing());
    assert_eq!(
        map.joint(HumanoidBone::Chest),
        motion.skeleton.joint("Spine1")
    );
}

#[test]
fn malformed_bvh_is_rejected_with_a_reason() {
    let good = walk_bvh();
    let cases = [
        ("", "ends early"),
        ("MOTION", "start with HIERARCHY"),
        (
            &good.replace("Xposition", "Wposition"),
            "Unknown BVH channel",
        ),
        (&good.replace("Frames: 31", "Frames: 32"), "ends early"),
        (
            &good.replace("Frames: 31", "Frames: 30"),
            "more motion values",
        ),
        (
            &good.replace("Frame Time: 0.0333333", "Frame Time: 0"),
            "Frame Time",
        ),
        (&good.replacen("95.0000", "nan", 1), "finite"),
        (&good.replacen("}\n", "", 1), "Unexpected"),
    ];
    for (text, reason) in cases {
        let error = Motion::parse(text, "bad").unwrap_err();
        assert!(error.contains(reason), "expected {reason:?}, got {error:?}");
    }
}

#[test]
fn a_bvh_walk_retargets_onto_an_a_posed_rig_facing_the_other_way() {
    let motion = Motion::parse(&walk_bvh(), "walk").unwrap();
    let source_map = HumanoidMap::detect(&motion.skeleton);
    let target = vrm_rig();
    let target_map = HumanoidMap::detect(&target);
    let clip = retarget(
        &motion.skeleton,
        &source_map,
        &motion.clip,
        &target,
        &target_map,
    )
    .unwrap();
    assert_eq!(clip.name(), "walk");
    // Source faces +Z, target −Z: world directions agree after a half turn about Y.
    let face = Quat::from_rotation_y(std::f32::consts::PI);
    let pairs = [
        (HumanoidBone::LeftUpperLeg, HumanoidBone::LeftLowerLeg),
        (HumanoidBone::LeftLowerLeg, HumanoidBone::LeftFoot),
        (HumanoidBone::RightUpperLeg, HumanoidBone::RightLowerLeg),
        (HumanoidBone::RightLowerLeg, HumanoidBone::RightFoot),
        (HumanoidBone::LeftUpperArm, HumanoidBone::LeftLowerArm),
        (HumanoidBone::LeftLowerArm, HumanoidBone::LeftHand),
        (HumanoidBone::RightUpperArm, HumanoidBone::RightLowerArm),
        (HumanoidBone::Chest, HumanoidBone::Neck),
        (HumanoidBone::Neck, HumanoidBone::Head),
    ];
    for time in [0.0, 0.25, 0.4, 0.75, 1.0] {
        let source = world_positions(&motion.skeleton, &motion.clip, time);
        let result = world_positions(&target, &clip, time);
        for (a, b) in pairs {
            let dir = |p: &[Vec3], map: &HumanoidMap| {
                (p[map.joint(b).unwrap()] - p[map.joint(a).unwrap()]).normalize()
            };
            let expected = face * dir(&source, &source_map);
            let actual = dir(&result, &target_map);
            assert!(
                actual.abs_diff_eq(expected, 2e-3),
                "{a:?} at {time}s: {actual} vs {expected}"
            );
        }
    }
    // Hips at (0, 95, 60) cm become (0, 0.95, −0.6) m: leg-length scale and a half turn.
    let hips = target_map.joint(HumanoidBone::Hips).unwrap();
    let end = world_positions(&target, &clip, 1.0)[hips];
    assert!(end.abs_diff_eq(Vec3::new(0., 0.95, -0.6), 1e-3), "{end}");
}

#[test]
fn non_humanoid_skeletons_cannot_be_retargeted() {
    let motion = Motion::parse(&walk_bvh(), "walk").unwrap();
    let bar = Skeleton::new(
        vec!["root".into()],
        vec![None],
        vec![Transform::IDENTITY],
        vec![Mat4::IDENTITY],
    )
    .unwrap();
    let error = retarget(
        &motion.skeleton,
        &HumanoidMap::detect(&motion.skeleton),
        &motion.clip,
        &bar,
        &HumanoidMap::detect(&bar),
    )
    .unwrap_err();
    assert!(
        error.contains("target skeleton is not a recognised humanoid"),
        "{error}"
    );
    assert!(error.contains("hips"));
}

/// A skinned GLB of `skeleton`: one thin box per bone, bound rigidly to the parent joint.
fn rig_glb(skeleton: &Skeleton) -> Vec<u8> {
    let mut global: Vec<Mat4> = Vec::new();
    for (i, t) in skeleton.rest().iter().enumerate() {
        let m = Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation);
        global.push(skeleton.parents()[i].map_or(m, |p| global[p as usize] * m));
    }
    let (mut positions, mut joints, mut indices) = (Vec::new(), Vec::new(), Vec::<u32>::new());
    for (i, parent) in skeleton.parents().iter().enumerate() {
        let Some(p) = parent else { continue };
        let (a, b) = (
            global[*p as usize].w_axis.truncate(),
            global[i].w_axis.truncate(),
        );
        let axis = (b - a).normalize();
        let side = axis.any_orthonormal_vector() * 0.03;
        let up = axis.cross(side);
        let base = positions.len() as u32;
        for end in [a, b] {
            for (s, u) in [(1., 1.), (-1., 1.), (-1., -1.), (1., -1.)] {
                positions.push(end + side * s + up * u);
                joints.push([*p, 0, 0, 0]);
            }
        }
        for k in 0..4 {
            let (k0, k1) = (base + k, base + (k + 1) % 4);
            indices.extend([k0, k1, k1 + 4, k0, k1 + 4, k0 + 4]);
        }
    }
    let mut bin = Vec::new();
    for p in &positions {
        bin.extend(p.to_array().iter().flat_map(|v| v.to_le_bytes()));
    }
    let o_joints = bin.len();
    for j in &joints {
        bin.extend(j.iter().flat_map(|v| v.to_le_bytes()));
    }
    let o_weights = bin.len();
    for _ in &positions {
        bin.extend([1f32, 0., 0., 0.].iter().flat_map(|v| v.to_le_bytes()));
    }
    let o_indices = bin.len();
    bin.extend(indices.iter().flat_map(|v| v.to_le_bytes()));
    let o_bind = bin.len();
    for m in &global {
        bin.extend(
            m.inverse()
                .to_cols_array()
                .iter()
                .flat_map(|v| v.to_le_bytes()),
        );
    }
    let (lo, hi) = positions.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    let n = skeleton.len();
    let nodes: Vec<String> = (0..n)
        .map(|i| {
            let t = skeleton.rest()[i];
            let children: Vec<String> = (0..n)
                .filter(|&c| skeleton.parents()[c] == Some(i as u16))
                .map(|c| c.to_string())
                .collect();
            format!(
                r#"{{"name":"{}","translation":{:?},"rotation":{:?},"children":[{}]}}"#,
                skeleton.names()[i],
                t.translation.to_array(),
                t.rotation.to_array(),
                children.join(",")
            )
        })
        .chain([r#"{"name":"body","mesh":0,"skin":0}"#.to_string()])
        .collect();
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0,{n}]}}],"nodes":[{}],
"skins":[{{"joints":[{}],"inverseBindMatrices":4}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"JOINTS_0":1,"WEIGHTS_0":2}},"indices":3}}]}}],
"buffers":[{{"byteLength":{}}}],
"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{o_joints}}},{{"buffer":0,"byteOffset":{o_joints},"byteLength":{}}},{{"buffer":0,"byteOffset":{o_weights},"byteLength":{}}},{{"buffer":0,"byteOffset":{o_indices},"byteLength":{}}},{{"buffer":0,"byteOffset":{o_bind},"byteLength":{}}}],
"accessors":[{{"bufferView":0,"componentType":5126,"count":{},"type":"VEC3","min":{:?},"max":{:?}}},{{"bufferView":1,"componentType":5123,"count":{},"type":"VEC4"}},{{"bufferView":2,"componentType":5126,"count":{},"type":"VEC4"}},{{"bufferView":3,"componentType":5125,"count":{},"type":"SCALAR"}},{{"bufferView":4,"componentType":5126,"count":{n},"type":"MAT4"}}]}}"#,
        nodes.join(","),
        (0..n).map(|i| i.to_string()).collect::<Vec<_>>().join(","),
        bin.len(),
        o_weights - o_joints,
        o_indices - o_weights,
        o_bind - o_indices,
        bin.len() - o_bind,
        positions.len(),
        lo.to_array(),
        hi.to_array(),
        positions.len(),
        positions.len(),
        indices.len(),
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

/// A disposable template-shaped project with the rig as a model and the walk as a BVH.
struct RigProject(std::path::PathBuf);
impl RigProject {
    fn new(component: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "emergent-mocap-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        for dir in ["assets/models", "assets/mocap", "src/generated"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
        std::fs::write(root.join("assets/models/rig.glb"), rig_glb(&vrm_rig())).unwrap();
        std::fs::write(root.join("assets/mocap/walk.bvh"), walk_bvh()).unwrap();
        std::fs::write(
            root.join("assets/rig.entity.toml"),
            "schema_version = 1\nid = 'rig'\nkind = 'model'\nmodel = 'assets/models/rig.glb'\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/main.scene.toml"),
            format!("schema_version = 1\nid = 'main'\n[[entities]]\nid = 'rig'\nasset = 'assets/rig.entity.toml'\n{component}"),
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
impl Drop for RigProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn an_animator_plays_a_bvh_walk_retargeted_onto_a_rigged_model() {
    struct Idle;
    impl emergent::Game for Idle {
        fn fixed_update(&mut self, _: &mut emergent::EngineContext<'_>) {}
    }
    let project = RigProject::new(
        "[[entities.components]]\ntype = 'emergent.animator'\nmotion = 'assets/mocap/walk.bvh'\nclip = 'walk'\nloop = false\n",
    );
    let mut engine = emergent::Engine::new(Default::default()).unwrap();
    let loaded = project
        .load()
        .unwrap()
        .instantiate(engine.world_mut())
        .unwrap();
    engine.run_ticks(&mut Idle, 61).unwrap();
    let animator = engine
        .world()
        .component::<emergent::Animator>(loaded.entities["rig"])
        .unwrap()
        .unwrap();
    assert_eq!(animator.clip().unwrap().name(), "walk");
    let skeleton = animator.skeleton();
    let hips = skeleton.joint("hips").unwrap();
    // palette = global × inverse bind, so the bind-space hips position maps to its world position.
    let bind = skeleton.inverse_bind()[hips].inverse().w_axis.truncate();
    let at = animator.palette()[hips].transform_point3(bind);
    assert!(at.abs_diff_eq(Vec3::new(0., 0.95, -0.6), 1e-3), "{at}");
}

#[test]
fn a_motion_onto_a_non_humanoid_model_fails_at_load() {
    let project = RigProject::new("");
    std::fs::write(
        project.0.join("assets/models/rig.glb"),
        rig_glb(
            &Skeleton::new(
                vec!["a".into(), "b".into()],
                vec![None, Some(0)],
                vec![
                    Transform::IDENTITY,
                    Transform::new(Vec3::Y, Quat::IDENTITY, Vec3::ONE),
                ],
                vec![Mat4::IDENTITY, Mat4::from_translation(-Vec3::Y)],
            )
            .unwrap(),
        ),
    )
    .unwrap();
    std::fs::write(
        project.0.join("assets/main.scene.toml"),
        "schema_version = 1\nid = 'main'\n[[entities]]\nid = 'rig'\nasset = 'assets/rig.entity.toml'\n[[entities.components]]\ntype = 'emergent.animator'\nmotion = 'assets/mocap/walk.bvh'\n",
    )
    .unwrap();
    let error = project.load().err().unwrap().to_string();
    assert!(error.contains("not a recognised humanoid"), "{error}");
}

#[test]
fn a_speed_blend_follows_the_objects_movement_with_clips_in_place() {
    struct Walker(Option<emergent::EntityId>);
    impl emergent::Game for Walker {
        fn fixed_update(&mut self, context: &mut emergent::EngineContext<'_>) {
            let id = self.0.unwrap();
            let world = &mut *context.world;
            let mut t = *world.local_transform(id).unwrap();
            t.translation.z -= 0.9 / 60.0;
            world.set_local_transform(id, t).unwrap();
        }
    }
    let project = RigProject::new(
        "[[entities.components]]\ntype = 'emergent.animator'\nmotion = 'assets/mocap/walk.bvh, assets/mocap/run.bvh'\nblend = 'walk, run'\n",
    );
    // Same take at double rate: travels 1.2 m/s against the walk's 0.6 m/s.
    std::fs::write(
        project.0.join("assets/mocap/run.bvh"),
        walk_bvh().replace("Frame Time: 0.0333333", "Frame Time: 0.0166667"),
    )
    .unwrap();
    let mut engine = emergent::Engine::new(Default::default()).unwrap();
    let loaded = project
        .load()
        .unwrap()
        .instantiate(engine.world_mut())
        .unwrap();
    let id = loaded.entities["rig"];
    engine.run_ticks(&mut Walker(Some(id)), 120).unwrap();
    let animator = engine
        .world()
        .component::<emergent::Animator>(id)
        .unwrap()
        .unwrap();
    assert!(
        (animator.ground_speed() - 0.9).abs() < 0.01,
        "{}",
        animator.ground_speed()
    );
    let skeleton = animator.skeleton();
    let hips = skeleton.joint("hips").unwrap();
    let bind = skeleton.inverse_bind()[hips].inverse().w_axis.truncate();
    let at = animator.palette()[hips].transform_point3(bind);
    assert!(
        at.x.abs() < 0.01 && at.z.abs() < 0.01,
        "hips stay over the origin: {at}"
    );

    let project = RigProject::new(
        "[[entities.components]]\ntype = 'emergent.animator'\nmotion = 'assets/mocap/walk.bvh'\nblend = 'walk, jog'\n",
    );
    let error = project.load().err().unwrap().to_string();
    assert!(error.contains("no animation clip named \"jog\""), "{error}");
}

#[test]
fn feet_land_on_uneven_static_ground_and_the_hips_drop_to_reach() {
    struct Idle;
    impl emergent::Game for Idle {
        fn fixed_update(&mut self, _: &mut emergent::EngineContext<'_>) {}
    }
    // The rig's left foot (x −0.1) stands over ground at y 0, the right (x +0.1) at −0.2.
    let project = RigProject::new(
        "[[entities.components]]\ntype = 'emergent.animator'\n\
         [[entities]]\nid = 'left'\nasset = 'assets/block.entity.toml'\n\
         [entities.transform]\ntranslation = [-0.15, -0.5, 0]\nscale = [0.2, 1, 1]\n\
         [[entities]]\nid = 'right'\nasset = 'assets/block.entity.toml'\n\
         [entities.transform]\ntranslation = [0.15, -0.7, 0]\nscale = [0.2, 1, 1]\n",
    );
    std::fs::write(
        project.0.join("assets/block.entity.toml"),
        "schema_version = 1\nid = 'block'\nkind = 'primitive'\n[geometry]\nprimitive = 'cube'\nhalf_extent = 0.5\ncolor = [1, 1, 1, 1]\n[physics]\nbody = 'static'\n",
    )
    .unwrap();
    let mut engine = emergent::Engine::new(Default::default()).unwrap();
    let loaded = project
        .load()
        .unwrap()
        .instantiate(engine.world_mut())
        .unwrap();
    engine.run_ticks(&mut Idle, 60).unwrap();
    let animator = engine
        .world()
        .component::<emergent::Animator>(loaded.entities["rig"])
        .unwrap()
        .unwrap();
    let skeleton = animator.skeleton();
    let at = |name: &str| {
        let j = skeleton.joint(name).unwrap();
        let bind = skeleton.inverse_bind()[j].inverse().w_axis.truncate();
        animator.palette()[j].transform_point3(bind)
    };
    let (hips, left, right) = (at("hips"), at("leftFoot"), at("rightFoot"));
    assert!((hips.y - 0.8).abs() < 2e-3, "hips drop 0.2: {hips}");
    assert!(
        (left.y - 0.1).abs() < 2e-3,
        "left ankle stays 0.1 above its ground: {left}"
    );
    assert!(
        (right.y + 0.1).abs() < 2e-3,
        "right ankle 0.1 above its lower ground: {right}"
    );
    assert!(
        left.x.abs() > 0.09 && left.z.abs() < 0.02,
        "feet stay under the body: {left}"
    );
    let knee = at("leftLowerLeg");
    assert!(
        knee.z < -0.02,
        "the bent knee points forward (−Z for this rig): {knee}"
    );
}

#[test]
fn the_head_turns_towards_its_look_at_target() {
    struct Idle;
    impl emergent::Game for Idle {
        fn fixed_update(&mut self, _: &mut emergent::EngineContext<'_>) {}
    }
    // The rig faces −Z; the target is 45° off to its right (+X) and above its head.
    let project = RigProject::new(
        "[[entities.components]]\ntype = 'emergent.animator'\nlook_at = 'target'\n\
         [[entities]]\nid = 'target'\nasset = 'assets/marker.entity.toml'\n\
         [entities.transform]\ntranslation = [2, 1.5, -2]\n",
    );
    std::fs::write(
        project.0.join("assets/marker.entity.toml"),
        "schema_version = 1\nid = 'marker'\nkind = 'primitive'\n[geometry]\nprimitive = 'cube'\nhalf_extent = 0.1\ncolor = [1, 1, 1, 1]\n",
    )
    .unwrap();
    let mut engine = emergent::Engine::new(Default::default()).unwrap();
    let loaded = project
        .load()
        .unwrap()
        .instantiate(engine.world_mut())
        .unwrap();
    engine.run_ticks(&mut Idle, 60).unwrap();
    let animator = engine
        .world()
        .component::<emergent::Animator>(loaded.entities["rig"])
        .unwrap()
        .unwrap();
    let skeleton = animator.skeleton();
    let head = skeleton.joint("head").unwrap();
    let palette = animator.palette()[head];
    let (_, rotation, _) = palette.to_scale_rotation_translation();
    let facing = rotation * Vec3::NEG_Z;
    let at = palette.transform_point3(skeleton.inverse_bind()[head].inverse().w_axis.truncate());
    let wanted = (Vec3::new(2., 1.5, -2.) - at).normalize();
    assert!(facing.angle_between(wanted) < 0.02, "{facing} vs {wanted}");

    let project =
        RigProject::new("[[entities.components]]\ntype = 'emergent.animator'\nlook_at = 'rig'\n");
    let error = project.load().err().unwrap().to_string();
    assert!(error.contains("cannot look at its own object"), "{error}");
}

/// The standard template's try-out scene: WASD drives an invisible physics box, and the
/// mannequin under it blends idle/walk/run by speed, turns to its movement and plants
/// its feet.
#[cfg(feature = "physics")]
#[test]
fn template_third_person_mannequin_walks_runs_and_turns() {
    use emergent::{Key, KeySet};
    struct Idle;
    impl emergent::Game for Idle {
        fn fixed_update(&mut self, _: &mut emergent::EngineContext<'_>) {}
    }
    let project = emergent::Project::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/standard"),
    )
    .unwrap();
    let scene =
        emergent::SceneDefinition::load(&project, "assets/scenes/third-person.scene.toml").unwrap();
    let mut engine = emergent::Engine::new(Default::default()).unwrap();
    let loaded = scene.instantiate(engine.world_mut()).unwrap();
    let hero = loaded.entities["hero"];
    let speed = |e: &emergent::Engine| {
        e.world()
            .component::<emergent::Animator>(hero)
            .unwrap()
            .unwrap()
            .ground_speed()
    };
    // Facing: the mannequin's +Z, turned by its object's rotation.
    let facing = |e: &emergent::Engine| {
        e.world()
            .global_transform(hero)
            .unwrap()
            .matrix()
            .transform_vector3(Vec3::Z)
    };
    engine.run_ticks(&mut Idle, 60).unwrap();
    assert!(speed(&engine) < 0.05, "standing: {}", speed(&engine));

    engine.set_keys(KeySet::EMPTY.with(Key::W));
    engine.run_ticks(&mut Idle, 120).unwrap();
    assert!(
        (speed(&engine) - 1.4).abs() < 0.1,
        "walk: {}",
        speed(&engine)
    );
    let f = facing(&engine);
    assert!(f.z < -0.99, "turned to walk away from the camera (−Z): {f}");

    engine.set_keys(KeySet::EMPTY.with(Key::W).with(Key::Shift));
    engine.run_ticks(&mut Idle, 90).unwrap();
    assert!(
        (speed(&engine) - 3.8).abs() < 0.2,
        "run: {}",
        speed(&engine)
    );

    engine.set_keys(KeySet::EMPTY.with(Key::D));
    engine.run_ticks(&mut Idle, 60).unwrap();
    let f = facing(&engine);
    assert!(f.x > 0.99, "turned right (+X): {f}");

    engine.clear_input();
    engine.run_ticks(&mut Idle, 90).unwrap();
    assert!(speed(&engine) < 0.1, "stopped: {}", speed(&engine));
    // Standing on the flat floor, the feet are on it (ankles 8 cm up, foot boxes touch).
    let animator = engine
        .world()
        .component::<emergent::Animator>(hero)
        .unwrap()
        .unwrap();
    let skeleton = animator.skeleton();
    let world = engine.world().global_transform(hero).unwrap().matrix();
    for foot in ["leftFoot", "rightFoot"] {
        let j = skeleton.joint(foot).unwrap();
        let bind = skeleton.inverse_bind()[j].inverse().w_axis.truncate();
        let at = world.transform_point3(animator.palette()[j].transform_point3(bind));
        assert!((at.y - 0.08).abs() < 0.03, "{foot} ankle at {at}");
    }
}
