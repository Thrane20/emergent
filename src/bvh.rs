//! Biovision Hierarchy (`.bvh`) motion capture import (SKEL-3): a joint hierarchy with
//! offsets, then per-frame channel values. Produces a [`Skeleton`] whose rest pose is the
//! zero-rotation offsets, and one [`AnimationClip`] sampled at the file's frame time.
use glam::{Mat4, Quat, Vec3, Vec4};

use crate::{
    AnimationChannel, AnimationClip, ChannelProperty, Interpolation, Project, Skeleton, Transform,
};

pub const BVH_MAX_BYTES: usize = 64 * 1024 * 1024;
pub const BVH_MAX_FRAMES: usize = 100_000;

/// A parsed motion capture take.
#[derive(Clone, Debug)]
pub struct Motion {
    pub skeleton: Skeleton,
    pub clip: AnimationClip,
}

#[derive(Clone, Copy, PartialEq)]
enum Channel {
    Position(usize),
    Rotation(usize),
}

fn err<T>(message: impl Into<String>) -> Result<T, String> {
    Err(message.into())
}

impl Motion {
    /// Reads a project-relative `.bvh`; the clip is named after the file stem.
    pub fn load(project: &Project, asset: &str) -> Result<Self, String> {
        if !asset.ends_with(".bvh") {
            return err("Motion capture files must be .bvh.");
        }
        let path = project.asset_path(asset).map_err(|e| e.to_string())?;
        let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        if size > BVH_MAX_BYTES as u64 {
            return err("BVH file exceeds 64 MiB.");
        }
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let stem = std::path::Path::new(asset)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("motion")
            .trim_end_matches(".bvh");
        Self::parse(&text, stem)
    }

    pub fn parse(text: &str, name: &str) -> Result<Self, String> {
        if text.len() > BVH_MAX_BYTES {
            return err("BVH file exceeds 64 MiB.");
        }
        let mut tokens = text.split_ascii_whitespace();
        let mut next = || {
            tokens
                .next()
                .ok_or_else(|| "BVH file ends early.".to_string())
        };
        if next()? != "HIERARCHY" {
            return err("BVH files start with HIERARCHY.");
        }
        let mut names = Vec::new();
        let mut parents: Vec<Option<u16>> = Vec::new();
        let mut offsets = Vec::new();
        // Per joint: its channels in file order (index into the frame row).
        let mut channels: Vec<Vec<Channel>> = Vec::new();
        let mut channel_count = 0usize;
        let mut stack: Vec<Option<usize>> = Vec::new(); // None marks an End Site block.
        let mut token = next()?;
        loop {
            match token {
                "ROOT" | "JOINT" => {
                    if (token == "ROOT") != stack.is_empty() {
                        return err("BVH needs exactly one ROOT, with JOINTs inside it.");
                    }
                    if names.len() >= crate::SKELETON_MAX_JOINTS {
                        return err("BVH skeleton exceeds 256 joints.");
                    }
                    let parent = match stack.last() {
                        Some(Some(p)) => Some(*p as u16),
                        Some(None) => return err("End Site blocks cannot contain joints."),
                        None => None,
                    };
                    names.push(next()?.to_string());
                    parents.push(parent);
                    offsets.push(Vec3::ZERO);
                    channels.push(Vec::new());
                    if next()? != "{" {
                        return err("Expected { after a joint name.");
                    }
                    stack.push(Some(names.len() - 1));
                }
                "End" => {
                    if next()? != "Site" || next()? != "{" {
                        return err("Expected End Site {.");
                    }
                    stack.push(None);
                }
                "OFFSET" => {
                    let mut v = [0f32; 3];
                    for x in &mut v {
                        *x = number(next()?)?;
                    }
                    if let Some(Some(j)) = stack.last() {
                        offsets[*j] = Vec3::from_array(v);
                    }
                }
                "CHANNELS" => {
                    let Some(Some(j)) = stack.last().copied() else {
                        return err("CHANNELS must belong to a joint.");
                    };
                    let count: usize = next()?.parse().map_err(|_| "Bad CHANNELS count.")?;
                    if count > 6 || !channels[j].is_empty() {
                        return err("A joint has at most one CHANNELS line of up to 6 channels.");
                    }
                    for _ in 0..count {
                        let channel = match next()? {
                            "Xposition" => Channel::Position(0),
                            "Yposition" => Channel::Position(1),
                            "Zposition" => Channel::Position(2),
                            "Xrotation" => Channel::Rotation(0),
                            "Yrotation" => Channel::Rotation(1),
                            "Zrotation" => Channel::Rotation(2),
                            other => return err(format!("Unknown BVH channel {other}.")),
                        };
                        channels[j].push(channel);
                    }
                    channel_count += count;
                }
                "}" => {
                    if stack.pop().is_none() {
                        return err("Unbalanced } in BVH hierarchy.");
                    }
                    if stack.is_empty() {
                        break;
                    }
                }
                other => return err(format!("Unexpected {other:?} in BVH hierarchy.")),
            }
            token = next()?;
        }
        if next()? != "MOTION" || next()? != "Frames:" {
            return err("Expected MOTION and Frames: after the hierarchy.");
        }
        let frames: usize = next()?.parse().map_err(|_| "Bad frame count.")?;
        if frames == 0 || frames > BVH_MAX_FRAMES {
            return err(format!("BVH needs 1 to {BVH_MAX_FRAMES} frames."));
        }
        if next()? != "Frame" || next()? != "Time:" {
            return err("Expected Frame Time:.");
        }
        let frame_time = number(next()?)?;
        if frame_time <= 0.0 || frame_time > 10.0 {
            return err("Frame Time must be between 0 and 10 seconds.");
        }
        let mut values = Vec::with_capacity(frames * channel_count);
        for _ in 0..frames * channel_count {
            values.push(number(next()?)?);
        }
        if next().is_ok() {
            return err("BVH has more motion values than its frames and channels.");
        }

        let joints = names.len();
        let rest: Vec<Transform> = offsets
            .iter()
            .map(|&o| Transform::new(o, Quat::IDENTITY, Vec3::ONE))
            .collect();
        let mut global = Vec::with_capacity(joints);
        for (i, t) in rest.iter().enumerate() {
            let local = Mat4::from_translation(t.translation);
            global.push(match parents[i] {
                Some(p) => global[p as usize] * local,
                None => local,
            });
        }
        let inverse_bind = global.iter().map(|m: &Mat4| m.inverse()).collect();
        let skeleton = Skeleton::new(names, parents, rest, inverse_bind).map_err(|e| e.0)?;

        let times: Vec<f32> = (0..frames).map(|f| f as f32 * frame_time).collect();
        let mut clip_channels = Vec::new();
        let mut column = 0;
        for (joint, list) in channels.iter().enumerate() {
            let at = |f: usize, k: usize| values[f * channel_count + column + k];
            if list.iter().any(|c| matches!(c, Channel::Position(_))) {
                let base = offsets[joint];
                let keys = (0..frames)
                    .map(|f| {
                        let mut p = base;
                        for (k, c) in list.iter().enumerate() {
                            if let Channel::Position(axis) = c {
                                p[*axis] = at(f, k);
                            }
                        }
                        p.extend(0.0)
                    })
                    .collect();
                clip_channels.push(channel(joint, ChannelProperty::Translation, &times, keys));
            }
            if list.iter().any(|c| matches!(c, Channel::Rotation(_))) {
                // Rotations compose in channel order: R = R1 · R2 · R3 (degrees).
                let keys = (0..frames)
                    .map(|f| {
                        let mut q = Quat::IDENTITY;
                        for (k, c) in list.iter().enumerate() {
                            if let Channel::Rotation(axis) = c {
                                let angle = at(f, k).to_radians();
                                q *= match axis {
                                    0 => Quat::from_rotation_x(angle),
                                    1 => Quat::from_rotation_y(angle),
                                    _ => Quat::from_rotation_z(angle),
                                };
                            }
                        }
                        Vec4::from(q.normalize())
                    })
                    .collect();
                clip_channels.push(channel(joint, ChannelProperty::Rotation, &times, keys));
            }
            column += list.len();
        }
        let clip = AnimationClip::new(name.to_string(), clip_channels, joints)?;
        Ok(Self { skeleton, clip })
    }
}

fn channel(
    joint: usize,
    property: ChannelProperty,
    times: &[f32],
    values: Vec<Vec4>,
) -> AnimationChannel {
    AnimationChannel {
        joint: joint as u16,
        property,
        interpolation: Interpolation::Linear,
        times: times.to_vec(),
        values,
    }
}

fn number(token: &str) -> Result<f32, String> {
    token
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{token:?} is not a finite number."))
}
