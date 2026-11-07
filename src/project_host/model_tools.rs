//! `--inspect-model`: loads a project `.glb` (or `.bvh` motion capture) through the engine
//! importers and reports its mesh size, skeleton joints, humanoid mapping and animation
//! clips, so the IDE and MCP show what the game will play.
use std::error::Error;

use crate::{AnimationClip, HumanoidMap, ImportedModel, Motion, Project, Skeleton};

pub(super) fn inspect(project: &Project, path: &str) -> Result<(), Box<dyn Error>> {
    let mut summary = toml::Table::new();
    let (skeleton, clips) = if path.ends_with(".bvh") {
        let motion = Motion::load(project, path)?;
        (Some(motion.skeleton), vec![motion.clip])
    } else {
        let model = ImportedModel::load(project, path)?;
        summary.insert(
            "vertices".into(),
            (model.mesh.vertices().len() as i64).into(),
        );
        summary.insert(
            "triangles".into(),
            ((model.mesh.indices().len() / 3) as i64).into(),
        );
        summary.insert("skinned".into(), model.skeleton.is_some().into());
        (model.skeleton, model.clips)
    };
    if let Some(skeleton) = &skeleton {
        summary.insert("joints".into(), joints(skeleton).into());
        let map = HumanoidMap::detect(skeleton);
        let mut humanoid = toml::Table::new();
        for (bone, joint) in map.bones() {
            humanoid.insert(bone.name().into(), skeleton.names()[joint].clone().into());
        }
        summary.insert("humanoid".into(), humanoid.into());
        summary.insert(
            "humanoid_missing".into(),
            map.missing()
                .into_iter()
                .map(toml::Value::from)
                .collect::<Vec<_>>()
                .into(),
        );
    }
    summary.insert("clips".into(), clip_list(&clips).into());
    out!("{}", toml::to_string(&summary)?);
    Ok(())
}

fn joints(skeleton: &Skeleton) -> Vec<toml::Value> {
    skeleton
        .names()
        .iter()
        .zip(skeleton.parents())
        .map(|(name, parent)| {
            let mut joint = toml::Table::new();
            joint.insert("name".into(), name.clone().into());
            if let Some(p) = parent {
                joint.insert(
                    "parent".into(),
                    skeleton.names()[*p as usize].clone().into(),
                );
            }
            toml::Value::Table(joint)
        })
        .collect()
}

fn clip_list(clips: &[AnimationClip]) -> Vec<toml::Value> {
    clips
        .iter()
        .map(|clip| {
            let mut entry = toml::Table::new();
            entry.insert("name".into(), clip.name().into());
            entry.insert(
                "duration_seconds".into(),
                ((f64::from(clip.duration()) * 1000.0).round() / 1000.0).into(),
            );
            entry.insert("channels".into(), (clip.channels().len() as i64).into());
            toml::Value::Table(entry)
        })
        .collect()
}
