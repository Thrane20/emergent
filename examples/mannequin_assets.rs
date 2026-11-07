//! Writes the standard template's try-out character: a segmented wooden-mannequin GLB
//! (VRM bone names, metres, T-pose, faces +Z, feet at y = 0, each part rigidly skinned to
//! one joint) and looping idle/walk/run BVH takes (CMU-style names, centimetres) that the
//! animator retargets onto it.
//! `cargo run --example mannequin_assets -- templates/standard/assets`
use std::fmt::Write as _;
use std::path::Path;

use emergent::{Mat4, Vec3};

/// (VRM name, parent, model-space T-pose position).
const JOINTS: &[(&str, Option<&str>, [f32; 3])] = &[
    ("hips", None, [0.0, 0.95, 0.0]),
    ("spine", Some("hips"), [0.0, 1.05, 0.0]),
    ("chest", Some("spine"), [0.0, 1.22, 0.0]),
    ("neck", Some("chest"), [0.0, 1.45, 0.0]),
    ("head", Some("neck"), [0.0, 1.55, 0.0]),
    ("leftUpperLeg", Some("hips"), [0.1, 0.92, 0.0]),
    ("leftLowerLeg", Some("leftUpperLeg"), [0.1, 0.5, 0.0]),
    ("leftFoot", Some("leftLowerLeg"), [0.1, 0.08, 0.0]),
    ("leftToes", Some("leftFoot"), [0.1, 0.02, 0.13]),
    ("rightUpperLeg", Some("hips"), [-0.1, 0.92, 0.0]),
    ("rightLowerLeg", Some("rightUpperLeg"), [-0.1, 0.5, 0.0]),
    ("rightFoot", Some("rightLowerLeg"), [-0.1, 0.08, 0.0]),
    ("rightToes", Some("rightFoot"), [-0.1, 0.02, 0.13]),
    ("leftShoulder", Some("chest"), [0.04, 1.4, 0.0]),
    ("leftUpperArm", Some("leftShoulder"), [0.18, 1.4, 0.0]),
    ("leftLowerArm", Some("leftUpperArm"), [0.46, 1.4, 0.0]),
    ("leftHand", Some("leftLowerArm"), [0.71, 1.4, 0.0]),
    ("rightShoulder", Some("chest"), [-0.04, 1.4, 0.0]),
    ("rightUpperArm", Some("rightShoulder"), [-0.18, 1.4, 0.0]),
    ("rightLowerArm", Some("rightUpperArm"), [-0.46, 1.4, 0.0]),
    ("rightHand", Some("rightLowerArm"), [-0.71, 1.4, 0.0]),
];

const WOOD: [f32; 3] = [0.62, 0.42, 0.24];
const SHIRT: [f32; 3] = [0.12, 0.22, 0.45];
const TROUSERS: [f32; 3] = [0.08, 0.09, 0.12];
const SHOES: [f32; 3] = [0.35, 0.08, 0.05];

fn joint(name: &str) -> usize {
    JOINTS.iter().position(|j| j.0 == name).unwrap()
}

/// (joint, model-space min, max, colour). Axis-aligned in the T-pose.
fn parts() -> Vec<(usize, Vec3, Vec3, [f32; 3])> {
    let mut parts = vec![
        (
            joint("hips"),
            [-0.15, 0.87, -0.09],
            [0.15, 1.05, 0.09],
            TROUSERS,
        ),
        (
            joint("spine"),
            [-0.13, 1.04, -0.08],
            [0.13, 1.23, 0.08],
            SHIRT,
        ),
        (
            joint("chest"),
            [-0.18, 1.22, -0.1],
            [0.18, 1.46, 0.1],
            SHIRT,
        ),
        (
            joint("neck"),
            [-0.04, 1.44, -0.04],
            [0.04, 1.56, 0.04],
            WOOD,
        ),
        (joint("head"), [-0.09, 1.55, -0.1], [0.09, 1.77, 0.11], WOOD),
        // Nose, so facing reads at a glance.
        (
            joint("head"),
            [-0.015, 1.63, 0.11],
            [0.015, 1.67, 0.14],
            WOOD,
        ),
    ];
    for (side, x) in [("left", 1.0f32), ("right", -1.0)] {
        let j = |part: &str| joint(&format!("{side}{part}"));
        let span = |a: f32, b: f32| (a.min(b), a.max(b));
        let mut push = |joint, (x0, x1): (f32, f32), y: [f32; 2], z: [f32; 2], c| {
            parts.push((joint, [x0, y[0], z[0]], [x1, y[1], z[1]], c));
        };
        push(
            j("UpperLeg"),
            span(x * 0.035, x * 0.165),
            [0.49, 0.93],
            [-0.07, 0.07],
            TROUSERS,
        );
        push(
            j("LowerLeg"),
            span(x * 0.05, x * 0.15),
            [0.07, 0.51],
            [-0.055, 0.055],
            TROUSERS,
        );
        push(
            j("Foot"),
            span(x * 0.05, x * 0.15),
            [0.0, 0.1],
            [-0.06, 0.14],
            SHOES,
        );
        push(
            j("Toes"),
            span(x * 0.055, x * 0.145),
            [0.0, 0.05],
            [0.13, 0.21],
            SHOES,
        );
        push(
            j("Shoulder"),
            span(x * 0.03, x * 0.19),
            [1.36, 1.44],
            [-0.05, 0.05],
            SHIRT,
        );
        push(
            j("UpperArm"),
            span(x * 0.17, x * 0.47),
            [1.355, 1.445],
            [-0.045, 0.045],
            SHIRT,
        );
        push(
            j("LowerArm"),
            span(x * 0.45, x * 0.72),
            [1.36, 1.44],
            [-0.04, 0.04],
            WOOD,
        );
        push(
            j("Hand"),
            span(x * 0.7, x * 0.86),
            [1.375, 1.425],
            [-0.045, 0.045],
            WOOD,
        );
    }
    parts
        .into_iter()
        .map(|(j, lo, hi, c)| (j, Vec3::from_array(lo), Vec3::from_array(hi), c))
        .collect()
}

fn mannequin_glb() -> Vec<u8> {
    let (mut positions, mut normals, mut colors, mut joints) = (
        Vec::<Vec3>::new(),
        Vec::<Vec3>::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut indices = Vec::<u32>::new();
    for (j, lo, hi, color) in parts() {
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                let mut n = Vec3::ZERO;
                n[axis] = sign;
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let base = positions.len() as u32;
                for (a, b) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                    let mut p = Vec3::ZERO;
                    p[axis] = if sign > 0.0 { hi[axis] } else { lo[axis] };
                    p[u] = if a == 1 { hi[u] } else { lo[u] };
                    p[v] = if b == 1 { hi[v] } else { lo[v] };
                    positions.push(p);
                    normals.push(n);
                    colors.push([color[0], color[1], color[2], 1.0]);
                    joints.push([j as u16, 0, 0, 0]);
                }
                // Counter-clockwise seen from outside.
                if sign > 0.0 {
                    indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                } else {
                    indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
                }
            }
        }
    }
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut view = |bin: &mut Vec<u8>, bytes: Vec<u8>| {
        let offset = bin.len();
        bin.extend(bytes);
        views.push(format!(
            r#"{{"buffer":0,"byteOffset":{offset},"byteLength":{}}}"#,
            bin.len() - offset
        ));
    };
    let floats = |v: &[f32]| v.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
    view(
        &mut bin,
        floats(
            &positions
                .iter()
                .flat_map(|p| p.to_array())
                .collect::<Vec<_>>(),
        ),
    );
    view(
        &mut bin,
        floats(
            &normals
                .iter()
                .flat_map(|p| p.to_array())
                .collect::<Vec<_>>(),
        ),
    );
    view(
        &mut bin,
        floats(&colors.iter().flatten().copied().collect::<Vec<_>>()),
    );
    view(
        &mut bin,
        joints
            .iter()
            .flatten()
            .flat_map(|j| j.to_le_bytes())
            .collect(),
    );
    view(
        &mut bin,
        floats(&[1.0, 0.0, 0.0, 0.0].repeat(positions.len())),
    );
    view(
        &mut bin,
        indices.iter().flat_map(|i| i.to_le_bytes()).collect(),
    );
    let binds: Vec<f32> = JOINTS
        .iter()
        .flat_map(|j| Mat4::from_translation(-Vec3::from_array(j.2)).to_cols_array())
        .collect();
    view(&mut bin, floats(&binds));
    let (lo, hi) = positions.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    let n = JOINTS.len();
    let nodes: Vec<String> = JOINTS
        .iter()
        .enumerate()
        .map(|(i, (name, parent, at))| {
            let origin = parent.map_or(Vec3::ZERO, |p| Vec3::from_array(JOINTS[joint(p)].2));
            let local = Vec3::from_array(*at) - origin;
            let children: Vec<String> = (0..n)
                .filter(|&c| JOINTS[c].1 == Some(JOINTS[i].0))
                .map(|c| c.to_string())
                .collect();
            format!(
                r#"{{"name":"{name}","translation":{:?},"children":[{}]}}"#,
                local.to_array(),
                children.join(",")
            )
        })
        .chain([r#"{"name":"mannequin","mesh":0,"skin":0}"#.to_string()])
        .collect();
    let count = positions.len();
    let json = format!(
        r#"{{"asset":{{"version":"2.0","generator":"emergent mannequin_assets"}},"scene":0,"scenes":[{{"nodes":[0,{n}]}}],"nodes":[{}],
"skins":[{{"joints":[{}],"inverseBindMatrices":6}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"NORMAL":1,"COLOR_0":2,"JOINTS_0":3,"WEIGHTS_0":4}},"indices":5}}]}}],
"buffers":[{{"byteLength":{}}}],"bufferViews":[{}],
"accessors":[{{"bufferView":0,"componentType":5126,"count":{count},"type":"VEC3","min":{:?},"max":{:?}}},{{"bufferView":1,"componentType":5126,"count":{count},"type":"VEC3"}},{{"bufferView":2,"componentType":5126,"count":{count},"type":"VEC4"}},{{"bufferView":3,"componentType":5123,"count":{count},"type":"VEC4"}},{{"bufferView":4,"componentType":5126,"count":{count},"type":"VEC4"}},{{"bufferView":5,"componentType":5125,"count":{},"type":"SCALAR"}},{{"bufferView":6,"componentType":5126,"count":{n},"type":"MAT4"}}]}}"#,
        nodes.join(","),
        (0..n).map(|i| i.to_string()).collect::<Vec<_>>().join(","),
        bin.len(),
        views.join(","),
        lo.to_array(),
        hi.to_array(),
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

/// One gait: period (s), travel (cm/s) and joint amplitudes (degrees, cm).
struct Gait {
    period: f32,
    travel: f32,
    thigh: f32,
    knee_swing: f32,
    knee_stance: f32,
    bob: f32,
    lean: f32,
    arm_swing: f32,
    elbow: f32,
    arm_down: f32,
    height: f32,
}

/// BVH joints: (name, offset cm, children), depth-first.
struct Node(&'static str, [f32; 3], Vec<Node>);

fn bvh_skeleton() -> Node {
    let leg = |side: &'static str, x: f32| -> Node {
        let (up, low, foot, toe): (&'static str, &'static str, &'static str, &'static str) =
            match side {
                "Left" => ("LeftUpLeg", "LeftLeg", "LeftFoot", "LeftToeBase"),
                _ => ("RightUpLeg", "RightLeg", "RightFoot", "RightToeBase"),
            };
        Node(
            up,
            [x, -3.0, 0.0],
            vec![Node(
                low,
                [0.0, -42.0, 0.0],
                vec![Node(
                    foot,
                    [0.0, -42.0, 0.0],
                    vec![Node(toe, [0.0, -6.0, 13.0], vec![])],
                )],
            )],
        )
    };
    let arm = |side: &'static str, x: f32| -> Node {
        let (sh, up, low, hand): (&'static str, &'static str, &'static str, &'static str) =
            match side {
                "Left" => ("LeftShoulder", "LeftArm", "LeftForeArm", "LeftHand"),
                _ => ("RightShoulder", "RightArm", "RightForeArm", "RightHand"),
            };
        Node(
            sh,
            [4.0 * x, 18.0, 0.0],
            vec![Node(
                up,
                [14.0 * x, 0.0, 0.0],
                vec![Node(
                    low,
                    [28.0 * x, 0.0, 0.0],
                    vec![Node(hand, [25.0 * x, 0.0, 0.0], vec![])],
                )],
            )],
        )
    };
    Node(
        "Hips",
        [0.0, 0.0, 0.0],
        vec![
            leg("Left", 10.0),
            leg("Right", -10.0),
            Node(
                "Spine",
                [0.0, 10.0, 0.0],
                vec![Node(
                    "Spine1",
                    [0.0, 17.0, 0.0],
                    vec![
                        Node(
                            "Neck",
                            [0.0, 23.0, 0.0],
                            vec![Node("Head", [0.0, 10.0, 0.0], vec![])],
                        ),
                        arm("Left", 1.0),
                        arm("Right", -1.0),
                    ],
                )],
            ),
        ],
    )
}

fn write_hierarchy(out: &mut String, node: &Node, depth: usize, names: &mut Vec<&'static str>) {
    let pad = "  ".repeat(depth);
    let Node(name, offset, children) = node;
    names.push(name);
    let root = depth == 0;
    let _ = writeln!(out, "{pad}{} {name}", if root { "ROOT" } else { "JOINT" });
    let _ = writeln!(out, "{pad}{{");
    let _ = writeln!(
        out,
        "{pad}  OFFSET {} {} {}",
        offset[0], offset[1], offset[2]
    );
    if root {
        let _ = writeln!(
            out,
            "{pad}  CHANNELS 6 Xposition Yposition Zposition Zrotation Xrotation Yrotation"
        );
    } else {
        let _ = writeln!(out, "{pad}  CHANNELS 3 Zrotation Xrotation Yrotation");
    }
    if children.is_empty() {
        let _ = writeln!(
            out,
            "{pad}  End Site\n{pad}  {{\n{pad}    OFFSET 0 0 5\n{pad}  }}"
        );
    }
    for child in children {
        write_hierarchy(out, child, depth + 1, names);
    }
    let _ = writeln!(out, "{pad}}}");
}

/// Rotations (degrees, Z X Y) for one joint at gait phase `p` (radians).
fn pose(g: &Gait, name: &str, p: f32) -> [f32; 3] {
    let leg = |phase: f32| {
        let thigh = g.thigh * phase.sin();
        let knee = g.knee_stance + (g.knee_swing - g.knee_stance) * phase.cos().max(0.0).powi(2);
        (thigh, knee)
    };
    let (thigh_l, knee_l) = leg(p);
    let (thigh_r, knee_r) = leg(p + std::f32::consts::PI);
    // +X rotation swings a hanging limb backwards (towards −Z); forward is negative.
    match name {
        "LeftUpLeg" => [0.0, -thigh_l, 0.0],
        "RightUpLeg" => [0.0, -thigh_r, 0.0],
        "LeftLeg" => [0.0, knee_l, 0.0],
        "RightLeg" => [0.0, knee_r, 0.0],
        "LeftFoot" => [0.0, thigh_l - knee_l, 0.0],
        "RightFoot" => [0.0, thigh_r - knee_r, 0.0],
        "Spine" => [0.0, g.lean, 0.0],
        "Spine1" => [0.0, 1.5 * (p * 2.0).sin(), -4.0 * p.sin()],
        "Neck" => [0.0, -0.6 * g.lean, 0.0],
        // Arms hang from the T-pose; each swings against its own side's leg.
        "LeftArm" => [-g.arm_down, 0.0, g.arm_swing * p.sin()],
        "RightArm" => [g.arm_down, 0.0, g.arm_swing * p.sin()],
        "LeftForeArm" => [0.0, 0.0, -g.elbow],
        "RightForeArm" => [0.0, 0.0, g.elbow],
        _ => [0.0, 0.0, 0.0],
    }
}

fn bvh(g: &Gait) -> String {
    let mut out = String::from("HIERARCHY\n");
    let mut names = Vec::new();
    write_hierarchy(&mut out, &bvh_skeleton(), 0, &mut names);
    let fps = 30.0;
    let frames = (g.period * fps).round() as usize;
    // One extra frame repeats the first pose, so the clip loops exactly.
    let _ = writeln!(
        out,
        "MOTION\nFrames: {}\nFrame Time: {:.7}",
        frames + 1,
        1.0 / fps
    );
    for f in 0..=frames {
        let t = f as f32 / frames as f32;
        let p = t * std::f32::consts::TAU;
        let height = g.height - g.bob * (1.0 - (2.0 * p).cos()) / 2.0;
        let sway = if g.travel == 0.0 { 1.2 * p.sin() } else { 0.0 };
        let mut row = vec![
            sway,
            height,
            g.travel * g.period * t,
            0.0,
            0.0,
            4.0 * p.sin(),
        ];
        for name in &names[1..] {
            row.extend(pose(g, name, p));
        }
        let row: Vec<String> = row.iter().map(|v| format!("{v:.3}")).collect();
        out.push_str(&row.join(" "));
        out.push('\n');
    }
    out
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: mannequin_assets <assets dir>");
    let out = Path::new(&out);
    std::fs::create_dir_all(out.join("models")).unwrap();
    std::fs::create_dir_all(out.join("mocap")).unwrap();
    std::fs::write(out.join("models/mannequin.glb"), mannequin_glb()).unwrap();
    let gaits = [
        (
            "idle",
            Gait {
                period: 3.0,
                travel: 0.0,
                thigh: 0.0,
                knee_swing: 4.0,
                knee_stance: 4.0,
                bob: 0.6,
                lean: 0.0,
                arm_swing: 0.0,
                elbow: 12.0,
                arm_down: 78.0,
                height: 94.8,
            },
        ),
        (
            "walk",
            Gait {
                period: 1.1,
                travel: 140.0,
                thigh: 24.0,
                knee_swing: 60.0,
                knee_stance: 6.0,
                bob: 2.5,
                lean: 3.0,
                arm_swing: 18.0,
                elbow: 15.0,
                arm_down: 75.0,
                height: 94.5,
            },
        ),
        (
            "run",
            Gait {
                period: 0.7,
                travel: 380.0,
                thigh: 40.0,
                knee_swing: 105.0,
                knee_stance: 20.0,
                bob: 5.0,
                lean: 12.0,
                arm_swing: 35.0,
                elbow: 85.0,
                arm_down: 70.0,
                height: 91.0,
            },
        ),
    ];
    for (name, gait) in &gaits {
        std::fs::write(out.join(format!("mocap/{name}.bvh")), bvh(gait)).unwrap();
    }
    println!("wrote {}", out.display());
}
