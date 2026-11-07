//! CPU cost of skeletal animation: N animators each sampling a clip that rotates every
//! joint of a 65-joint chain (Mixamo-sized) and rebuilding its palette, per fixed tick.
//! `cargo run --release --example skeletal_bench [characters] [ticks] [blend]`; `blend`
//! (any non-zero value) speed-blends two clips half and half, the most expensive case.
//! `[blend] = 2` instead plays the standard template's mannequin walk with foot IK on
//! ground 5 cm above its floor (both legs solved every tick) and a look-at target.
use std::sync::Arc;
use std::time::Instant;

use emergent::{
    AnimationChannel, AnimationClip, Animator, ChannelProperty, Interpolation, Mat4, Quat,
    Skeleton, Transform, Vec3, Vec4,
};

const JOINTS: usize = 65;
const KEYS: usize = 30;

fn main() {
    let mut args = std::env::args()
        .skip(1)
        .map(|a| a.parse::<usize>().unwrap());
    let characters = args.next().unwrap_or(100);
    let ticks = args.next().unwrap_or(1000);
    let blend = args.next().unwrap_or(0) != 0;
    let rest = vec![Transform::new(Vec3::Y * 0.1, Quat::IDENTITY, Vec3::ONE); JOINTS];
    let skeleton = Arc::new(
        Skeleton::new(
            (0..JOINTS).map(|i| format!("joint{i}")).collect(),
            (0..JOINTS)
                .map(|i| i.checked_sub(1).map(|p| p as u16))
                .collect(),
            rest,
            vec![Mat4::IDENTITY; JOINTS],
        )
        .unwrap(),
    );
    let channels = |phase: f32| -> Vec<AnimationChannel> {
        (0..JOINTS as u16)
            .map(|joint| AnimationChannel {
                joint,
                property: ChannelProperty::Rotation,
                interpolation: Interpolation::Linear,
                times: (0..KEYS).map(|k| k as f32 / 30.0).collect(),
                values: (0..KEYS)
                    .map(|k| {
                        Vec4::from(Quat::from_rotation_z((k as f32 * 0.2 + phase).sin() * 0.3))
                    })
                    .collect(),
            })
            .collect()
    };
    let clips: Arc<[AnimationClip]> = vec![
        AnimationClip::new("sway".into(), channels(0.0), JOINTS).unwrap(),
        AnimationClip::new("fast".into(), channels(1.0), JOINTS).unwrap(),
    ]
    .into();
    let mut animators: Vec<Animator> = (0..characters)
        .map(|i| {
            let mut a = Animator::new(skeleton.clone(), clips.clone(), Some("sway")).unwrap();
            a.time = i as f32 * 0.01;
            if blend {
                a.set_blend(&[("sway", 0.0), ("fast", 2.0)]).unwrap();
            }
            a
        })
        .collect();
    let dt = 1.0 / 60.0;
    if std::env::args().nth(3).as_deref() == Some("2") {
        return ik(characters, ticks, dt);
    }
    let mut samples = Vec::with_capacity(ticks);
    for tick in 0..ticks {
        let start = Instant::now();
        for (i, animator) in animators.iter_mut().enumerate() {
            // Moving 1 m/s: halfway between the blend's 0 and 2 m/s clips.
            let at = Vec3::X * (tick as f32 * dt) + Vec3::Z * i as f32;
            animator.advance_at(dt, Mat4::from_translation(at));
        }
        samples.push(start.elapsed().as_secs_f64() * 1e3);
    }
    samples.sort_by(f64::total_cmp);
    let pick = |q: f64| samples[((samples.len() - 1) as f64 * q) as usize];
    let checksum: f32 = animators
        .iter()
        .map(|a| a.palette()[JOINTS - 1].w_axis.x)
        .sum();
    println!(
        "characters={characters} blend={blend} joints={JOINTS} keys/channel={KEYS} ticks={ticks} \
         ms/tick median={:.4} p99={:.4} max={:.4} (checksum {checksum:.3})",
        pick(0.5),
        pick(0.99),
        pick(1.0)
    );
}

fn ik(characters: usize, ticks: usize, dt: f32) {
    let project = emergent::Project::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/standard"),
    )
    .unwrap();
    let model = emergent::ImportedModel::load(&project, "assets/models/mannequin.glb").unwrap();
    let skeleton = Arc::new(model.skeleton.unwrap());
    let take = emergent::Motion::load(&project, "assets/mocap/walk.bvh").unwrap();
    let mut walk = emergent::retarget(
        &take.skeleton,
        &emergent::HumanoidMap::detect(&take.skeleton),
        &take.clip,
        &skeleton,
        &emergent::HumanoidMap::detect(&skeleton),
    )
    .unwrap();
    walk.make_in_place(&skeleton);
    let clips: Arc<[AnimationClip]> = vec![walk].into();
    let mut animators: Vec<Animator> = (0..characters)
        .map(|i| {
            let mut a = Animator::new(skeleton.clone(), clips.clone(), Some("walk")).unwrap();
            a.time = i as f32 * 0.01;
            a
        })
        .collect();
    let ground = |origin: Vec3, _: f32| Some((Vec3::new(origin.x, 0.05, origin.z), Vec3::Y));
    let mut samples = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        let start = Instant::now();
        for (i, animator) in animators.iter_mut().enumerate() {
            let inputs = emergent::AnimatorInputs {
                world: Mat4::from_translation(Vec3::Z * i as f32),
                ground: Some(&ground),
                look_target: Some(Vec3::new(2.0, 1.5, i as f32 + 2.0)),
            };
            animator.advance_with(dt, &inputs);
        }
        samples.push(start.elapsed().as_secs_f64() * 1e3);
    }
    samples.sort_by(f64::total_cmp);
    let pick = |q: f64| samples[((samples.len() - 1) as f64 * q) as usize];
    println!(
        "characters={characters} mannequin joints={} walk + foot IK + look-at ticks={ticks} \
         ms/tick median={:.4} p99={:.4} max={:.4}",
        skeleton.len(),
        pick(0.5),
        pick(0.99),
        pick(1.0)
    );
}
