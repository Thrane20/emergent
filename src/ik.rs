//! Inverse kinematics on a sampled local pose (SKEL-5). Solvers read current model-space
//! matrices (`Skeleton::globals`) and write joint-local rotations back into the pose;
//! callers recompute the matrices before reading them again.
use glam::{Mat3, Mat4, Quat, Vec3};

use crate::{Skeleton, Transform};

pub(crate) fn rotation(m: &Mat4) -> Quat {
    m.to_scale_rotation_translation().1.normalize()
}

pub(crate) fn position(m: &Mat4) -> Vec3 {
    m.w_axis.truncate()
}

/// Rotation taking the frame (`u1`, `n1`) onto (`u2`, `n2`); each pair is orthonormal.
fn frame_rotation(u1: Vec3, n1: Vec3, u2: Vec3, n2: Vec3) -> Quat {
    let from = Mat3::from_cols(u1, n1, u1.cross(n1));
    let to = Mat3::from_cols(u2, n2, u2.cross(n2));
    Quat::from_mat3(&(to * from.transpose())).normalize()
}

/// Analytic two-bone IK. `chain` is upper, lower and end joint, each the direct child of
/// the one before. Moves the end towards `target` (model space), bending the middle joint
/// towards `pole`; out-of-reach targets straighten the chain towards them. The middle
/// joint keeps its hinge axis from the animated pose where it is bent. Returns false
/// (pose untouched) for a chain that is not parent-linked or has zero-length bones.
pub fn two_bone(
    skeleton: &Skeleton,
    pose: &mut [Transform],
    globals: &[Mat4],
    chain: [usize; 3],
    target: Vec3,
    pole: Vec3,
) -> bool {
    let [a, b, c] = chain;
    let parents = skeleton.parents();
    if parents[b] != Some(a as u16) || parents[c] != Some(b as u16) {
        return false;
    }
    let (pa, pb, pc) = (
        position(&globals[a]),
        position(&globals[b]),
        position(&globals[c]),
    );
    let (la, lb) = (pa.distance(pb), pb.distance(pc));
    if la < 1e-6 || lb < 1e-6 {
        return false;
    }
    let reach = target - pa;
    let Some(along) = reach.try_normalize() else {
        return false;
    };
    let eps = 1e-4 * (la + lb);
    let d = reach.length().clamp((la - lb).abs() + eps, la + lb - eps);
    // Knee direction: the pole, projected off the reach line.
    let side = |v: Vec3| (v - along * v.dot(along)).try_normalize();
    let bend = side(pole - pa)
        .or_else(|| side(pb - pa))
        .unwrap_or_else(|| along.any_orthonormal_vector());
    let cos = ((la * la + d * d - lb * lb) / (2.0 * la * d)).clamp(-1.0, 1.0);
    let knee = pa + la * (along * cos + bend * (1.0 - cos * cos).sqrt());
    let end = pa + along * d;

    let (u1, u2) = ((pb - pa) / la, (knee - pa) / la);
    let n2 = u2.cross(end - knee).normalize();
    // Current bend plane; a straight limb takes the target plane's normal.
    let n1 = u1
        .cross(pc - pb)
        .try_normalize()
        .map(|n| (n - u1 * n.dot(u1)).normalize())
        .unwrap_or_else(|| (n2 - u1 * n2.dot(u1)).try_normalize().unwrap_or(n2));
    let upper = frame_rotation(u1, n1, u2, n2);
    let lower_dir = (upper * (pc - pb)).normalize();
    let lower = Quat::from_rotation_arc(lower_dir, (end - knee).normalize());

    let (ga, gb) = (rotation(&globals[a]), rotation(&globals[b]));
    let (ga2, gb2) = (upper * ga, lower * upper * gb);
    let parent = parents[a].map_or(Quat::IDENTITY, |p| rotation(&globals[p as usize]));
    pose[a].rotation = (parent.inverse() * ga2).normalize();
    pose[b].rotation = (ga2.inverse() * gb2).normalize();
    true
}

/// Turns `joints` (parent first, each the direct child of the one before, the last one
/// facing `forward` in its own frame) so the last faces `target`, by at most `max_angle`
/// radians, scaled by `weight`. Each joint takes an equal share of the turn.
#[allow(clippy::too_many_arguments)]
pub fn look_at(
    skeleton: &Skeleton,
    pose: &mut [Transform],
    globals: &[Mat4],
    joints: &[usize],
    forward: Vec3,
    target: Vec3,
    max_angle: f32,
    weight: f32,
) {
    let Some(&last) = joints.last() else { return };
    let facing = rotation(&globals[last]) * forward;
    let Some(wanted) = (target - position(&globals[last])).try_normalize() else {
        return;
    };
    let angle = facing.angle_between(wanted);
    if angle < 1e-5 || weight <= 0.0 {
        return;
    }
    let full = Quat::from_rotation_arc(facing.normalize(), wanted);
    let turn = Quat::IDENTITY.slerp(full, weight.min(1.0) * (max_angle / angle).min(1.0));
    let parents = skeleton.parents();
    let mut parent = parents[joints[0]].map_or(Quat::IDENTITY, |p| rotation(&globals[p as usize]));
    for (i, &joint) in joints.iter().enumerate() {
        if i > 0 && parents[joint] != Some(joints[i - 1] as u16) {
            return;
        }
        let share = Quat::IDENTITY.slerp(turn, (i + 1) as f32 / joints.len() as f32);
        let global = share * rotation(&globals[joint]);
        pose[joint].rotation = (parent.inverse() * global).normalize();
        parent = global;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hip → knee → ankle hanging down 0.45 m each, slightly bent forward at the knee.
    fn leg() -> (Skeleton, Vec<Transform>) {
        let rest = vec![
            Transform::new(Vec3::Y, Quat::IDENTITY, Vec3::ONE),
            Transform::new(Vec3::NEG_Y * 0.45, Quat::IDENTITY, Vec3::ONE),
            Transform::new(Vec3::NEG_Y * 0.45, Quat::IDENTITY, Vec3::ONE),
        ];
        let skeleton = Skeleton::new(
            vec!["hip".into(), "knee".into(), "ankle".into()],
            vec![None, Some(0), Some(1)],
            rest.clone(),
            vec![Mat4::IDENTITY; 3],
        )
        .unwrap();
        (skeleton, rest)
    }

    fn solve(target: Vec3, bent: bool) -> (Vec<Mat4>, Vec<Mat4>) {
        let (skeleton, mut pose) = leg();
        if bent {
            pose[1].rotation = Quat::from_rotation_x(0.3);
        }
        let mut before = Vec::new();
        skeleton.globals(&pose, &mut before).unwrap();
        assert!(two_bone(
            &skeleton,
            &mut pose,
            &before,
            [0, 1, 2],
            target,
            Vec3::new(0., 0.5, 1.)
        ));
        let mut after = Vec::new();
        skeleton.globals(&pose, &mut after).unwrap();
        (before, after)
    }

    #[test]
    fn two_bone_reaches_targets_and_bends_towards_the_pole() {
        for (target, bent) in [
            (Vec3::new(0., 0.3, 0.2), false),
            (Vec3::new(0.1, 0.4, -0.1), true),
            (Vec3::new(0., 0.75, 0.), true),
        ] {
            let (_, after) = solve(target, bent);
            let end = position(&after[2]);
            assert!(end.distance(target) < 1e-4, "{target} reached {end}");
            assert!(position(&after[1]).z > 0.0, "knee bends forward");
            assert!((position(&after[1]).distance(position(&after[0])) - 0.45).abs() < 1e-4);
        }
    }

    #[test]
    fn two_bone_straightens_towards_unreachable_targets() {
        let target = Vec3::new(0., -2.0, 0.5);
        let (_, after) = solve(target, true);
        let (hip, end) = (position(&after[0]), position(&after[2]));
        assert!((hip.distance(end) - 0.9).abs() < 1e-3);
        let dir = (target - hip).normalize();
        assert!(((end - hip).normalize()).dot(dir) > 0.9999);
    }

    #[test]
    fn two_bone_rejects_chains_that_are_not_linked() {
        let (skeleton, mut pose) = leg();
        let mut g = Vec::new();
        skeleton.globals(&pose, &mut g).unwrap();
        assert!(!two_bone(
            &skeleton,
            &mut pose,
            &g,
            [0, 2, 1],
            Vec3::ZERO,
            Vec3::Z
        ));
    }

    #[test]
    fn look_at_turns_within_its_limit_and_shares_the_turn() {
        let (skeleton, mut pose) = leg();
        let mut g = Vec::new();
        skeleton.globals(&pose, &mut g).unwrap();
        // The "head" (joint 1) faces +Z; the target is 90° to its right (−X).
        let target = position(&g[1]) + Vec3::NEG_X * 3.0;
        look_at(&skeleton, &mut pose, &g, &[0, 1], Vec3::Z, target, 1.0, 1.0);
        skeleton.globals(&pose, &mut g).unwrap();
        let facing = rotation(&g[1]) * Vec3::Z;
        assert!(
            (facing.angle_between(Vec3::Z) - 1.0).abs() < 1e-4,
            "limited to 1 rad"
        );
        assert!(facing.x < 0.0, "towards the target");
        assert!(
            (rotation(&g[0]).angle_between(Quat::IDENTITY) - 0.5).abs() < 1e-4,
            "half each"
        );
    }
}
