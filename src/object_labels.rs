//! Project label bit positions and per-entity membership, independent of hierarchy ownership.
use bevy_ecs::component::Component;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Component)]
#[serde(default, deny_unknown_fields)]
pub struct ObjectClassification {
    pub layer: u8,
    pub tags: u32,
}
impl ObjectClassification {
    pub fn layer_mask(self) -> u32 {
        1u32.checked_shl(self.layer as u32).unwrap_or(0)
    }
    pub fn matches(self, layers: u32, tags: u32) -> bool {
        self.layer_mask() & layers != 0 && (tags == 0 || self.tags & tags != 0)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectLabels {
    pub schema_version: u32,
    pub layers: Vec<String>,
    pub tags: Vec<String>,
}
impl Default for ObjectLabels {
    fn default() -> Self {
        Self {
            schema_version: 1,
            layers: vec!["Default".into()],
            tags: vec![],
        }
    }
}
impl ObjectLabels {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.layers.is_empty()
            || self.layers.len() > 32
            || self.tags.len() > 32
            || self.layers[0].trim().is_empty()
        {
            return Err("Labels require schema 1, layer 0 and at most 32 layers/tags".into());
        }
        for list in [&self.layers, &self.tags] {
            let mut names = std::collections::HashSet::new();
            for name in list {
                if name.len() > 64
                    || name.chars().any(char::is_control)
                    || name.trim() != name
                    || (!name.is_empty() && !names.insert(name.to_lowercase()))
                {
                    return Err("Label names must be unique, trimmed and at most 64 bytes".into());
                }
            }
        }
        Ok(())
    }
    pub fn validate_object(&self, value: ObjectClassification) -> Result<(), String> {
        self.validate()?;
        let mask = self.tags.iter().enumerate().fold(0u32, |mask, (i, name)| {
            if name.is_empty() {
                mask
            } else {
                mask | (1u32 << i)
            }
        });
        if value.layer >= 32
            || self
                .layers
                .get(value.layer as usize)
                .is_none_or(|s| s.is_empty())
            || value.tags & !mask != 0
        {
            return Err("Object references an undefined layer or tag bit".into());
        }
        Ok(())
    }
    pub fn load(project: &crate::Project) -> Result<Self, Box<dyn std::error::Error>> {
        use std::io::Read;
        let root = project
            .manifest()
            .asset_roots
            .first()
            .ok_or("Project has no asset root")?;
        let path = root.join("object-labels.toml");
        let full = project.root().join(&path);
        if !full.try_exists()? {
            return Ok(Self::default());
        }
        let full = project.asset_path(path)?;
        let mut source = String::new();
        std::fs::File::open(full)?
            .take(16385)
            .read_to_string(&mut source)?;
        if source.len() > 16384 {
            return Err("Object labels exceed 16 KiB".into());
        }
        let labels: Self = toml::from_str(&source)?;
        labels.validate()?;
        Ok(labels)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn masks_include_high_bit_and_reject_missing_slots() {
        let mut labels = ObjectLabels {
            tags: vec![String::new(); 32],
            ..Default::default()
        };
        labels.tags[31] = "Enemy".into();
        labels.layers.resize(32, String::new());
        labels.layers[31] = "Actors".into();
        let object = ObjectClassification {
            layer: 31,
            tags: 1 << 31,
        };
        assert!(labels.validate_object(object).is_ok());
        assert!(object.matches(1 << 31, 1 << 31));
        assert!(!object.matches(1, 1 << 31));
        assert!(labels
            .validate_object(ObjectClassification { tags: 1, ..object })
            .is_err());
        labels.tags.push("Overflow".into());
        assert!(labels.validate().is_err());
    }
}

#[cfg(test)]
mod scene_tests {
    #[test]
    fn authored_membership_loads_and_undefined_bits_fail() {
        let project = crate::Project::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/standard"),
        )
        .unwrap();
        let source="schema_version=1\nid='labels-test'\n[[entities]]\nid='cube'\nasset='assets/entities/cube.entity.toml'\n[entities.classification]\nlayer=0\ntags=0\n";
        let scene =
            crate::SceneDefinition::from_source(&project, "assets/scenes/main.scene.toml", source)
                .unwrap();
        let mut world = crate::World::new();
        let loaded = scene.instantiate(&mut world).unwrap();
        assert_eq!(
            world
                .component::<super::ObjectClassification>(loaded.entities["cube"])
                .unwrap()
                .copied(),
            Some(super::ObjectClassification::default())
        );
        assert!(crate::SceneDefinition::from_source(
            &project,
            "assets/scenes/main.scene.toml",
            &source.replace("tags=0", "tags=2147483648")
        )
        .is_err());
    }
}
