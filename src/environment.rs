//! Scene-owned global lighting and a bounded panoramic background.
use crate::{Color, LightSource, MaterialError, Project, TextureImage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnvironmentDefinition {
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub background_color: [f32; 3],
    pub skybox: Option<String>,
    pub skybox_rotation: f32,
    pub skybox_intensity: f32,
}
impl Default for EnvironmentDefinition {
    fn default() -> Self {
        Self {
            ambient_color: [1.; 3],
            ambient_intensity: 0.18,
            background_color: [0.025, 0.035, 0.060],
            skybox: None,
            skybox_rotation: 0.,
            skybox_intensity: 1.,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct SceneEnvironment {
    pub settings: EnvironmentDefinition,
    pub(crate) skybox: Option<TextureImage>,
}
impl EnvironmentDefinition {
    pub fn validate(&self) -> Result<(), MaterialError> {
        if !self
            .ambient_color
            .iter()
            .chain(&self.background_color)
            .all(|v| v.is_finite() && (0. ..=1.).contains(v))
            || !self.ambient_intensity.is_finite()
            || !(0. ..=10.).contains(&self.ambient_intensity)
            || !self.skybox_rotation.is_finite()
            || !(0. ..=360.).contains(&self.skybox_rotation)
            || !self.skybox_intensity.is_finite()
            || !(0. ..=10.).contains(&self.skybox_intensity)
        {
            return Err(MaterialError("Environment requires RGB colours in 0..1, intensities in 0..10 and skybox rotation in 0..360 degrees.".into()));
        }
        if let Some(path) = &self.skybox {
            if path.len() > 512
                || !matches!(
                    path.rsplit('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .as_str(),
                    "png" | "jpg" | "jpeg"
                )
            {
                return Err(MaterialError(
                    "Skyboxes require a project PNG/JPEG panorama.".into(),
                ));
            }
        }
        Ok(())
    }
    pub fn resolve(&self, project: &Project) -> Result<SceneEnvironment, MaterialError> {
        self.validate()?;
        let skybox = self
            .skybox
            .as_ref()
            .map(|path| {
                let image = TextureImage::load(
                    project
                        .asset_path(path)
                        .map_err(|e| MaterialError(e.to_string()))?,
                )?;
                if image.width() != image.height() * 2 {
                    return Err(MaterialError(
                        "Skyboxes require a 2:1 equirectangular panorama.".into(),
                    ));
                }
                Ok(image)
            })
            .transpose()?;
        Ok(SceneEnvironment {
            settings: self.clone(),
            skybox,
        })
    }
    pub fn ambient_light(&self) -> Result<LightSource, MaterialError> {
        self.validate()?;
        let color = Color::new(
            self.ambient_color[0],
            self.ambient_color[1],
            self.ambient_color[2],
            1.,
        )
        .map_err(|e| MaterialError(e.to_string()))?;
        LightSource::ambient(color, self.ambient_intensity)
            .map_err(|e| MaterialError(e.to_string()))
    }
}
impl SceneEnvironment {
    /// Decoded panorama pixels, prepared outside rendering and simulation.
    pub fn skybox(&self) -> Option<&TextureImage> {
        self.skybox.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_roundtrip_and_untrusted_settings_are_bounded() {
        let settings: EnvironmentDefinition = toml::from_str("").unwrap();
        assert_eq!(settings, EnvironmentDefinition::default());
        assert!(settings.ambient_light().is_ok());
        assert_eq!(
            toml::from_str::<EnvironmentDefinition>(&toml::to_string(&settings).unwrap()).unwrap(),
            settings
        );
        for bad in [
            EnvironmentDefinition {
                ambient_intensity: f32::NAN,
                ..Default::default()
            },
            EnvironmentDefinition {
                ambient_color: [-1., 0., 0.],
                ..Default::default()
            },
            EnvironmentDefinition {
                background_color: [0., f32::INFINITY, 0.],
                ..Default::default()
            },
            EnvironmentDefinition {
                skybox_intensity: 11.,
                ..Default::default()
            },
            EnvironmentDefinition {
                skybox_rotation: 361.,
                ..Default::default()
            },
            EnvironmentDefinition {
                skybox: Some("sky.hdr".into()),
                ..Default::default()
            },
        ] {
            assert!(bad.validate().is_err());
        }
        assert!(toml::from_str::<EnvironmentDefinition>("unknown=1").is_err());
    }
}
