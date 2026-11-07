//! Skeletons for skinned meshes (SKEL-1). Joints are dense, topologically sorted
//! arrays inside one asset rather than world entities: `parents[i] < i`, so cycles
//! cannot be represented and pose evaluation is a single linear pass. Joint indices
//! are runtime-only; authored data refers to joints by name.
use std::fmt;

use glam::Mat4;

use crate::Transform;

/// Upper bound per skeleton (VRM humanoids use ~55 joints, Mixamo rigs 65).
pub const SKELETON_MAX_JOINTS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkeletonError(pub String);

impl fmt::Display for SkeletonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SkeletonError {}

fn invalid(message: impl Into<String>) -> Result<(), SkeletonError> {
    Err(SkeletonError(message.into()))
}

/// Immutable joint hierarchy shared by every instance of a skinned model.
#[derive(Clone, Debug, PartialEq)]
pub struct Skeleton {
    names: Vec<String>,
    parents: Vec<Option<u16>>,
    rest: Vec<Transform>,
    inverse_bind: Vec<Mat4>,
}

impl Skeleton {
    /// `rest` holds each joint's local transform (roots: model space); `inverse_bind`
    /// maps model-space bind positions into each joint's space.
    pub fn new(
        names: Vec<String>,
        parents: Vec<Option<u16>>,
        rest: Vec<Transform>,
        inverse_bind: Vec<Mat4>,
    ) -> Result<Self, SkeletonError> {
        let count = names.len();
        if count == 0 || count > SKELETON_MAX_JOINTS {
            invalid(format!(
                "A skeleton needs 1 to {SKELETON_MAX_JOINTS} joints, found {count}."
            ))?;
        }
        if parents.len() != count || rest.len() != count || inverse_bind.len() != count {
            invalid("Skeleton joint arrays differ in length.")?;
        }
        for (i, parent) in parents.iter().enumerate() {
            if parent.is_some_and(|p| p as usize >= i) {
                invalid(format!("Joint {i} must come after its parent."))?;
            }
        }
        let mut seen = std::collections::HashSet::with_capacity(count);
        for name in &names {
            if name.is_empty() || !seen.insert(name.as_str()) {
                invalid(format!(
                    "Joint names must be non-empty and unique: {name:?}."
                ))?;
            }
        }
        if !rest.iter().all(|t| {
            Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation).is_finite()
        }) || !inverse_bind.iter().all(|m| m.is_finite())
        {
            invalid("Skeleton transforms must be finite.")?;
        }
        Ok(Self {
            names,
            parents,
            rest,
            inverse_bind,
        })
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn parents(&self) -> &[Option<u16>] {
        &self.parents
    }

    pub fn rest(&self) -> &[Transform] {
        &self.rest
    }

    pub fn inverse_bind(&self) -> &[Mat4] {
        &self.inverse_bind
    }

    /// Runtime index of a joint by its durable name.
    pub fn joint(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }

    /// Writes the skinning palette (model-space joint matrix × inverse bind) for a
    /// local pose. Reuses `out`'s allocation; allocation-free once it has capacity.
    pub fn palette(&self, local: &[Transform], out: &mut Vec<Mat4>) -> Result<(), SkeletonError> {
        self.globals(local, out)?;
        // Second pass: every parent's model-space matrix is complete before any is replaced.
        for (matrix, inverse_bind) in out.iter_mut().zip(&self.inverse_bind) {
            *matrix *= *inverse_bind;
        }
        Ok(())
    }

    /// Model-space joint matrices of a local pose.
    pub fn globals(&self, local: &[Transform], out: &mut Vec<Mat4>) -> Result<(), SkeletonError> {
        if local.len() != self.len() {
            invalid("Pose joint count does not match the skeleton.")?;
        }
        out.clear();
        for (i, transform) in local.iter().enumerate() {
            let matrix = Mat4::from_scale_rotation_translation(
                transform.scale,
                transform.rotation,
                transform.translation,
            );
            let global = match self.parents[i] {
                Some(p) => out[p as usize] * matrix,
                None => matrix,
            };
            out.push(global);
        }
        Ok(())
    }

    pub fn rest_palette(&self) -> Vec<Mat4> {
        let mut out = Vec::with_capacity(self.len());
        self.palette(&self.rest, &mut out)
            .expect("rest pose matches its skeleton");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    fn chain() -> Skeleton {
        let rest = vec![
            Transform::IDENTITY,
            Transform::new(Vec3::Y, Quat::IDENTITY, Vec3::ONE),
        ];
        let inverse_bind = vec![Mat4::IDENTITY, Mat4::from_translation(-Vec3::Y)];
        Skeleton::new(
            vec!["hips".into(), "spine".into()],
            vec![None, Some(0)],
            rest,
            inverse_bind,
        )
        .unwrap()
    }

    #[test]
    fn rest_pose_palette_is_identity_when_bind_matches_rest() {
        for m in chain().rest_palette() {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-6));
        }
    }

    #[test]
    fn rotating_a_parent_moves_its_child_joint_and_vertices() {
        let skeleton = chain();
        let mut pose = skeleton.rest().to_vec();
        pose[0].rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let mut palette = Vec::new();
        skeleton.palette(&pose, &mut palette).unwrap();
        // A vertex bound at the spine joint (0, 1, 0) swings to (-1, 0, 0).
        let moved = palette[1].transform_point3(Vec3::Y);
        assert!(moved.abs_diff_eq(Vec3::NEG_X, 1e-5), "{moved}");
        let capacity = palette.capacity();
        skeleton.palette(&pose, &mut palette).unwrap();
        assert_eq!(palette.capacity(), capacity, "palette buffer is reused");
    }

    #[test]
    fn invalid_hierarchies_are_rejected() {
        let one = |parent| {
            Skeleton::new(
                vec!["a".into(), "b".into()],
                vec![None, parent],
                vec![Transform::IDENTITY; 2],
                vec![Mat4::IDENTITY; 2],
            )
        };
        assert!(one(Some(1)).is_err(), "self-parent");
        assert!(one(Some(5)).is_err(), "parent after child");
        assert!(Skeleton::new(
            vec!["a".into(), "a".into()],
            vec![None, None],
            vec![Transform::IDENTITY; 2],
            vec![Mat4::IDENTITY; 2],
        )
        .is_err());
        let many = SKELETON_MAX_JOINTS + 1;
        assert!(Skeleton::new(
            (0..many).map(|i| i.to_string()).collect(),
            vec![None; many],
            vec![Transform::IDENTITY; many],
            vec![Mat4::IDENTITY; many],
        )
        .is_err());
        assert!(chain()
            .palette(&[Transform::IDENTITY], &mut Vec::new())
            .is_err());
    }
}
