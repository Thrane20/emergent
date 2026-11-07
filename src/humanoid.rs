//! Humanoid bone profile (SKEL-3): the VRM 1.0 body bones, detected on any skeleton from
//! common naming conventions (VRM/VRoid, Mixamo, CMU/BVH, Unreal, Rigify, 3ds Max Biped).
//! A map is runtime-only: humanoid bone → joint index of one skeleton.
use crate::Skeleton;

/// VRM 1.0 humanoid body bones (fingers and eyes are not mapped yet).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HumanoidBone {
    Hips,
    Spine,
    Chest,
    UpperChest,
    Neck,
    Head,
    LeftShoulder,
    LeftUpperArm,
    LeftLowerArm,
    LeftHand,
    RightShoulder,
    RightUpperArm,
    RightLowerArm,
    RightHand,
    LeftUpperLeg,
    LeftLowerLeg,
    LeftFoot,
    LeftToes,
    RightUpperLeg,
    RightLowerLeg,
    RightFoot,
    RightToes,
}

use HumanoidBone::*;

impl HumanoidBone {
    pub const ALL: [Self; 22] = [
        Hips,
        Spine,
        Chest,
        UpperChest,
        Neck,
        Head,
        LeftShoulder,
        LeftUpperArm,
        LeftLowerArm,
        LeftHand,
        RightShoulder,
        RightUpperArm,
        RightLowerArm,
        RightHand,
        LeftUpperLeg,
        LeftLowerLeg,
        LeftFoot,
        LeftToes,
        RightUpperLeg,
        RightLowerLeg,
        RightFoot,
        RightToes,
    ];

    /// Bones a skeleton needs before clips can be retargeted onto or from it.
    pub const REQUIRED: [Self; 15] = [
        Hips,
        Spine,
        Head,
        LeftUpperArm,
        LeftLowerArm,
        LeftHand,
        RightUpperArm,
        RightLowerArm,
        RightHand,
        LeftUpperLeg,
        LeftLowerLeg,
        LeftFoot,
        RightUpperLeg,
        RightLowerLeg,
        RightFoot,
    ];

    /// VRM 1.0 name, e.g. `leftUpperArm`.
    pub fn name(self) -> &'static str {
        [
            "hips",
            "spine",
            "chest",
            "upperChest",
            "neck",
            "head",
            "leftShoulder",
            "leftUpperArm",
            "leftLowerArm",
            "leftHand",
            "rightShoulder",
            "rightUpperArm",
            "rightLowerArm",
            "rightHand",
            "leftUpperLeg",
            "leftLowerLeg",
            "leftFoot",
            "leftToes",
            "rightUpperLeg",
            "rightLowerLeg",
            "rightFoot",
            "rightToes",
        ][self as usize]
    }

    /// Humanoid bones further along the limb, nearest first; the first mapped one gives
    /// this bone's direction.
    pub(crate) fn children(self) -> &'static [Self] {
        match self {
            Hips => &[Spine, Chest, UpperChest, Neck, Head],
            Spine => &[Chest, UpperChest, Neck, Head],
            Chest => &[UpperChest, Neck, Head],
            UpperChest => &[Neck, Head],
            Neck => &[Head],
            LeftShoulder => &[LeftUpperArm],
            LeftUpperArm => &[LeftLowerArm],
            LeftLowerArm => &[LeftHand],
            RightShoulder => &[RightUpperArm],
            RightUpperArm => &[RightLowerArm],
            RightLowerArm => &[RightHand],
            LeftUpperLeg => &[LeftLowerLeg],
            LeftLowerLeg => &[LeftFoot],
            LeftFoot => &[LeftToes],
            RightUpperLeg => &[RightLowerLeg],
            RightLowerLeg => &[RightFoot],
            RightFoot => &[RightToes],
            Head | LeftHand | RightHand | LeftToes | RightToes => &[],
        }
    }

    /// The humanoid bone this one hangs from, for bones without a direction of their own.
    pub(crate) fn parent(self) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|b| b.children().first() == Some(&self))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Centre,
    Left,
    Right,
}

/// Limb or trunk part named by a side-free, separator-free, lower-case alias.
fn alias(base: &str, side: Side) -> Option<HumanoidBone> {
    let (left, right) = match base {
        "shoulder" | "clavicle" | "collar" => (LeftShoulder, RightShoulder),
        "upperarm" | "arm" | "humerus" => (LeftUpperArm, RightUpperArm),
        "lowerarm" | "forearm" | "elbow" => (LeftLowerArm, RightLowerArm),
        "hand" | "wrist" => (LeftHand, RightHand),
        "upperleg" | "upleg" | "thigh" | "femur" => (LeftUpperLeg, RightUpperLeg),
        "lowerleg" | "leg" | "shin" | "calf" | "knee" | "tibia" => (LeftLowerLeg, RightLowerLeg),
        "foot" | "ankle" => (LeftFoot, RightFoot),
        "toes" | "toebase" | "toe" | "ball" => (LeftToes, RightToes),
        _ if side == Side::Centre => {
            // Mixamo Spine/Spine1/Spine2 and Unreal spine_01/02/03 both mean spine, chest, upper chest.
            return match base {
                "hips" | "hip" | "pelvis" => Some(Hips),
                "spine" | "spine01" => Some(Spine),
                "chest" | "spine1" | "spine02" => Some(Chest),
                "upperchest" | "spine2" | "spine03" => Some(UpperChest),
                "neck" | "neck01" => Some(Neck),
                "head" => Some(Head),
                _ => None,
            };
        }
        _ => return None,
    };
    match side {
        Side::Left => Some(left),
        Side::Right => Some(right),
        Side::Centre => None,
    }
}

/// Humanoid bone for one joint name, if it follows a known convention.
pub fn detect_bone(name: &str) -> Option<HumanoidBone> {
    let name = name.rsplit(':').next().unwrap_or(name);
    let mut key: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    for prefix in ["mixamorig", "jbip", "bip001", "bip01", "def"] {
        if let Some(rest) = key.strip_prefix(prefix) {
            key = rest.to_string();
        }
    }
    // VRoid: J_Bip_C_Hips (centre marker).
    if let Some(rest) = key.strip_prefix('c') {
        if let Some(bone) = alias(rest, Side::Centre) {
            return Some(bone);
        }
    }
    let k = key.as_str();
    let sided = [
        ("left", Side::Left),
        ("right", Side::Right),
        ("l", Side::Left),
        ("r", Side::Right),
    ];
    for (token, side) in sided {
        if let Some(bone) = k.strip_prefix(token).and_then(|b| alias(b, side)) {
            return Some(bone);
        }
        if let Some(bone) = k.strip_suffix(token).and_then(|b| alias(b, side)) {
            return Some(bone);
        }
    }
    alias(k, Side::Centre)
}

/// Whether `joint` is below `ancestor` (parents precede children, so the walk is bounded).
fn descends(skeleton: &Skeleton, mut joint: usize, ancestor: usize) -> bool {
    while let Some(parent) = skeleton.parents()[joint] {
        joint = parent as usize;
        if joint == ancestor {
            return true;
        }
    }
    false
}

/// Humanoid bone → joint index for one skeleton.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HumanoidMap {
    joints: [Option<u16>; HumanoidBone::ALL.len()],
}

impl HumanoidMap {
    /// Maps joints by name; the first joint (parent-first order) claiming a bone wins.
    /// Spine, chest and upper chest are assigned by hierarchy instead, lowest first:
    /// rigs number their spine both ways (Mixamo Spine→Spine2 upward, Meshy Spine02→Spine).
    pub fn detect(skeleton: &Skeleton) -> Self {
        let mut joints = [None; HumanoidBone::ALL.len()];
        let mut spine: Vec<usize> = Vec::with_capacity(3);
        for (i, name) in skeleton.names().iter().enumerate() {
            match detect_bone(name) {
                Some(Spine | Chest | UpperChest) => {
                    if spine.len() < 3 && spine.last().is_none_or(|&p| descends(skeleton, i, p)) {
                        spine.push(i);
                    }
                }
                Some(bone) => {
                    joints[bone as usize].get_or_insert(i as u16);
                }
                None => {}
            }
        }
        for (bone, joint) in [Spine, Chest, UpperChest].into_iter().zip(spine) {
            joints[bone as usize] = Some(joint as u16);
        }
        Self { joints }
    }

    pub fn joint(&self, bone: HumanoidBone) -> Option<usize> {
        self.joints[bone as usize].map(usize::from)
    }

    /// Required bones this map lacks, by VRM name.
    pub fn missing(&self) -> Vec<&'static str> {
        HumanoidBone::REQUIRED
            .into_iter()
            .filter(|b| self.joint(*b).is_none())
            .map(HumanoidBone::name)
            .collect()
    }

    /// Mapped bones with their joint indices.
    pub fn bones(&self) -> impl Iterator<Item = (HumanoidBone, usize)> + '_ {
        HumanoidBone::ALL
            .into_iter()
            .filter_map(|b| self.joint(b).map(|j| (b, j)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_rig_conventions_map_to_the_same_bones() {
        let cases = [
            ("hips", Hips),
            ("leftUpperArm", LeftUpperArm),
            ("mixamorig:LeftArm", LeftUpperArm),
            ("mixamorig:LeftForeArm", LeftLowerArm),
            ("mixamorig:RightUpLeg", RightUpperLeg),
            ("mixamorig:RightLeg", RightLowerLeg),
            ("mixamorig:LeftToeBase", LeftToes),
            ("mixamorig:Spine1", Chest),
            ("mixamorig:Spine2", UpperChest),
            ("pelvis", Hips),
            ("spine_01", Spine),
            ("spine_03", UpperChest),
            ("upperarm_l", LeftUpperArm),
            ("calf_r", RightLowerLeg),
            ("clavicle_l", LeftShoulder),
            ("ball_r", RightToes),
            ("DEF-upper_arm.L", LeftUpperArm),
            ("DEF-thigh.R", RightUpperLeg),
            ("J_Bip_C_Hips", Hips),
            ("J_Bip_C_UpperChest", UpperChest),
            ("J_Bip_L_LowerArm", LeftLowerArm),
            ("J_Bip_R_Foot", RightFoot),
            ("Bip01 L Thigh", LeftUpperLeg),
            ("Bip01 R Forearm", RightLowerArm),
            ("LeftUpLeg", LeftUpperLeg),
            ("RightForeArm", RightLowerArm),
        ];
        for (name, bone) in cases {
            assert_eq!(detect_bone(name), Some(bone), "{name}");
        }
        for name in [
            "LHipJoint",
            "LowerBack",
            "Neck1",
            "root",
            "LeftHandIndex1",
            "lowerback",
        ] {
            assert_eq!(detect_bone(name), None, "{name}");
        }
    }

    #[test]
    fn spine_bones_follow_the_hierarchy_whichever_way_they_are_numbered() {
        use crate::Transform;
        use glam::Mat4;
        let rig = |names: &[&str]| {
            let n = names.len();
            let parents = (0..n).map(|i| i.checked_sub(1).map(|p| p as u16)).collect();
            Skeleton::new(
                names.iter().map(|s| s.to_string()).collect(),
                parents,
                vec![Transform::IDENTITY; n],
                vec![Mat4::IDENTITY; n],
            )
            .unwrap()
        };
        // Meshy rigs count down from the hips; Mixamo counts up. Both map lowest-first.
        for names in [
            ["Hips", "Spine02", "Spine01", "Spine", "neck"],
            ["Hips", "Spine", "Spine1", "Spine2", "Neck"],
        ] {
            let map = HumanoidMap::detect(&rig(&names));
            assert_eq!(map.joint(Spine), Some(1), "{names:?}");
            assert_eq!(map.joint(Chest), Some(2), "{names:?}");
            assert_eq!(map.joint(UpperChest), Some(3), "{names:?}");
            assert_eq!(map.joint(Neck), Some(4), "{names:?}");
        }
        // A two-joint spine leaves the upper chest unmapped.
        let map = HumanoidMap::detect(&rig(&["Hips", "Spine", "Chest", "Neck"]));
        assert_eq!((map.joint(Chest), map.joint(UpperChest)), (Some(2), None));
    }

    #[test]
    fn bone_parents_follow_the_limb_chains() {
        assert_eq!(LeftHand.parent(), Some(LeftLowerArm));
        assert_eq!(RightToes.parent(), Some(RightFoot));
        assert_eq!(Hips.parent(), None);
    }
}
