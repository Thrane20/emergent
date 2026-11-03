//! Authored surface settings and resolved noise inputs, independent of a GPU.
use crate::{NoiseDefinition, NoiseSettings, Project};
use bevy_ecs::component::Component;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialShading {
    #[default]
    Flat,
    Smooth,
    Unlit,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialNoiseMap {
    pub asset: String,
    #[serde(default = "one")]
    pub strength: f32,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialTextureMap {
    pub asset: String,
    #[serde(default = "one")]
    pub scale: f32,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialLayer {
    pub id: String,
    pub name: String,
    pub diffuse_map: Option<MaterialTextureMap>,
}
impl MaterialTextureMap {
    fn validate(&self) -> Result<(), MaterialError> {
        let ext = self
            .asset
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg")
            || self.asset.len() > 512
            || !self.scale.is_finite()
            || !(0.001..=100.).contains(&self.scale)
        {
            return Err(MaterialError(
                "Diffuse maps require PNG/JPEG assets and repeat scale 0.001..100.".into(),
            ));
        }
        Ok(())
    }
}
fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MaterialDefinition {
    pub asset: Option<String>,
    pub shader: Option<String>,
    pub shader_parameters: std::collections::BTreeMap<String, f32>,
    pub terrain_paint: Option<String>,
    pub shading: MaterialShading,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub albedo: Option<[f32; 3]>,
    pub specular: [f32; 3],
    pub shininess: f32,
    pub emission: [f32; 3],
    pub emission_intensity: f32,
    pub bump_strength: f32,
    pub noise_resolution: u32,
    pub texture_tiling: [f32; 2],
    pub diffuse_map: Option<MaterialTextureMap>,
    pub terrain_layers: Vec<MaterialLayer>,
    pub active_layer: Option<String>,
    pub albedo_map: Option<MaterialNoiseMap>,
    pub specular_map: Option<MaterialNoiseMap>,
    pub emission_map: Option<MaterialNoiseMap>,
    pub bump_map: Option<MaterialNoiseMap>,
}
impl Default for MaterialDefinition {
    fn default() -> Self {
        Self {
            asset: None,
            shader: None,
            shader_parameters: Default::default(),
            terrain_paint: None,
            shading: MaterialShading::Flat,
            cast_shadows: true,
            receive_shadows: true,
            albedo: None,
            specular: [0.0; 3],
            shininess: 32.0,
            emission: [0.0; 3],
            emission_intensity: 0.0,
            bump_strength: 0.1,
            noise_resolution: 32,
            texture_tiling: [1.0; 2],
            diffuse_map: None,
            terrain_layers: Vec::new(),
            active_layer: None,
            albedo_map: None,
            specular_map: None,
            emission_map: None,
            bump_map: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialError(pub String);
impl fmt::Display for MaterialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for MaterialError {}

/// Component attached to canonical entities; backend resources are renderer-owned.
#[derive(Clone, Debug, Default, Component)]
pub struct Material {
    definition: MaterialDefinition,
    pub(crate) shader: Option<crate::ShaderDefinition>,
    noise: [Option<NoiseSettings>; 4],
    diffuse: Option<crate::TextureImage>,
    /// The diffuse image is sampled with mesh texture coordinates instead of triplanar projection.
    uv_mapped: bool,
    pub(crate) terrain: Option<TerrainMaterial>,
}
/// Authored blend weights at the terrain heightfield resolution, in row-major XZ order.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainPaint {
    pub schema_version: u32,
    pub resolution: u32,
    pub materials: Vec<String>,
    pub transitions: [f32; 3],
    pub blend: f32,
    pub weights: Vec<u8>,
}
#[derive(Clone, Debug)]
pub(crate) struct TerrainMaterial {
    pub paint: TerrainPaint,
    pub layers: Vec<Material>,
}
fn read_asset<T: serde::de::DeserializeOwned>(
    project: &Project,
    asset: &str,
    limit: u64,
) -> Result<T, MaterialError> {
    use std::io::Read;
    let path = project
        .asset_path(asset)
        .map_err(|e| MaterialError(e.to_string()))?;
    let mut source = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_string(&mut source))
        .map_err(|e| MaterialError(format!("{asset}: {e}")))?;
    if source.len() as u64 > limit {
        return Err(MaterialError(format!("{asset}: asset exceeds size limit")));
    }
    toml::from_str(&source).map_err(|e| MaterialError(format!("{asset}: {e}")))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryMaterial {
    schema_version: u32,
    id: String,
    name: String,
    material: MaterialDefinition,
}
impl TerrainPaint {
    pub fn validate(&self) -> Result<(), MaterialError> {
        let valid = self.schema_version == 1
            && crate::terrain::TERRAIN_RESOLUTIONS.contains(&self.resolution)
            && (1..=4).contains(&self.materials.len())
            && self
                .materials
                .iter()
                .all(|p| p.ends_with(".material.toml") && p.len() <= 512)
            && self
                .transitions
                .iter()
                .all(|n| n.is_finite() && (0.0..=1000.0).contains(n))
            && self.transitions.windows(2).all(|p| p[0] < p[1])
            && self.blend.is_finite()
            && (0.0..=1000.0).contains(&self.blend)
            && self.weights.len() == (self.resolution as usize).pow(2) * 4
            && self.weights.as_chunks::<4>().0.iter().all(|w| {
                w.iter().map(|n| u32::from(*n)).sum::<u32>() == 255
                    && w[self.materials.len()..].iter().all(|n| *n == 0)
            });
        if !valid {
            return Err(MaterialError("Invalid terrain paint: use 1–4 material assets, ordered height transitions, and normalized RGBA weights at terrain resolution.".into()));
        }
        Ok(())
    }
}
impl MaterialDefinition {
    pub fn validate(&self) -> Result<(), MaterialError> {
        if self
            .shader
            .as_ref()
            .is_some_and(|p| !p.ends_with(".shader.toml") || p.len() > 512)
            || (self.shader.is_none() && !self.shader_parameters.is_empty())
            || self.shader_parameters.len() > 16
            || (self.shader.is_some() && (self.cast_shadows || self.terrain_paint.is_some()))
        {
            return Err(MaterialError("Shader materials need a .shader.toml asset, at most 16 parameters, shadows disabled and no terrain paint".into()));
        }
        if self
            .asset
            .as_ref()
            .is_some_and(|path| !path.ends_with(".material.toml") || path.len() > 512)
            || self
                .terrain_paint
                .as_ref()
                .is_some_and(|path| !path.ends_with(".paint.toml") || path.len() > 512)
        {
            return Err(MaterialError(
                "Invalid material or paint asset path.".into(),
            ));
        }
        if self.terrain_layers.len() > 8 {
            return Err(MaterialError(
                "At most eight terrain material layers are supported.".into(),
            ));
        }
        let mut ids = std::collections::HashSet::new();
        for layer in &self.terrain_layers {
            if layer.id.is_empty()
                || layer.id.len() > 64
                || !layer
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                || !ids.insert(&layer.id)
                || layer.name.trim().is_empty()
                || layer.name.chars().count() > 64
                || layer.name.chars().any(char::is_control)
            {
                return Err(MaterialError(
                    "Layer IDs must be unique and names contain 1–64 printable characters.".into(),
                ));
            }
        }
        if self
            .active_layer
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        {
            return Err(MaterialError(
                "Active terrain material layer does not exist.".into(),
            ));
        }
        for map in self.texture_maps() {
            map.validate()?;
        }
        if !matches!(self.noise_resolution, 32 | 64 | 128) {
            return Err(MaterialError(
                "Material noise resolution must be 32, 64 or 128.".into(),
            ));
        }
        let color = |rgb: [f32; 3]| rgb.iter().all(|n| n.is_finite() && (0.0..=1.0).contains(n));
        if !self.albedo.is_none_or(color)
            || !color(self.specular)
            || !color(self.emission)
            || !self.shininess.is_finite()
            || !(1.0..=256.0).contains(&self.shininess)
            || !self.emission_intensity.is_finite()
            || !(0.0..=100.0).contains(&self.emission_intensity)
            || !self.bump_strength.is_finite()
            || !(0.0..=2.0).contains(&self.bump_strength)
            || !self
                .texture_tiling
                .iter()
                .all(|n| n.is_finite() && (0.001..=100.0).contains(n))
        {
            return Err(MaterialError("Material colours must be linear RGB in 0..1; shininess 1..256, emission intensity 0..100, bump strength 0..2 and texture tiling 0.001..100 per axis.".into()));
        }
        for map in self.maps().into_iter().flatten() {
            if !map.asset.ends_with(".noise.toml")
                || map.asset.len() > 512
                || !map.strength.is_finite()
                || !(0.0..=1.0).contains(&map.strength)
            {
                return Err(MaterialError(
                    "Material maps require a .noise.toml asset and strength in 0..1.".into(),
                ));
            }
        }
        Ok(())
    }
    pub fn active_diffuse_map(&self) -> Option<&MaterialTextureMap> {
        match &self.active_layer {
            Some(id) => self
                .terrain_layers
                .iter()
                .find(|layer| &layer.id == id)
                .and_then(|layer| layer.diffuse_map.as_ref()),
            None => self.diffuse_map.as_ref(),
        }
    }
    pub fn texture_maps(&self) -> impl Iterator<Item = &MaterialTextureMap> {
        self.diffuse_map.iter().chain(
            self.terrain_layers
                .iter()
                .filter_map(|layer| layer.diffuse_map.as_ref()),
        )
    }
    pub(crate) fn maps(&self) -> [Option<&MaterialNoiseMap>; 4] {
        [
            self.albedo_map.as_ref(),
            self.specular_map.as_ref(),
            self.emission_map.as_ref(),
            self.bump_map.as_ref(),
        ]
    }
    pub fn resolve(self, project: &Project) -> Result<Material, MaterialError> {
        self.validate()?;
        if let Some(asset) = &self.asset {
            let reference = Self {
                asset: Some(asset.clone()),
                ..Self::default()
            };
            if self != reference {
                return Err(MaterialError(
                    "A material reference cannot contain inline overrides.".into(),
                ));
            }
            let file: LibraryMaterial = read_asset(project, asset, 256 * 1024)?;
            if file.schema_version != 1
                || file.id.is_empty()
                || file.id.len() > 128
                || file.name.trim().is_empty()
                || file.name.len() > 64
                || file.name.chars().any(char::is_control)
                || file.material.asset.is_some()
                || file.material.terrain_paint.is_some()
            {
                return Err(MaterialError("Library materials require schema 1, an ID/name and inline surface settings; nested references are not supported.".into()));
            }
            return file.material.resolve(project);
        }
        let shader = self
            .shader
            .as_ref()
            .map(|path| -> Result<_, MaterialError> {
                let graph: crate::ShaderDefinition = read_asset(project, path, 128 * 1024)?;
                graph.parameter_values(&self.shader_parameters)?;
                Ok(graph)
            })
            .transpose()?;
        let terrain = self
            .terrain_paint
            .as_ref()
            .map(|asset| -> Result<_, MaterialError> {
                let paint: TerrainPaint =
                    read_asset(project, asset, crate::terrain::TERRAIN_DATA_MAX_BYTES)?;
                paint.validate()?;
                let layers = paint
                    .materials
                    .iter()
                    .map(|asset| {
                        Self {
                            asset: Some(asset.clone()),
                            ..Self::default()
                        }
                        .resolve(project)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if layers.iter().any(|m| m.shader.is_some()) {
                    return Err(MaterialError(
                        "Shader materials cannot be terrain blend layers".into(),
                    ));
                }
                if layers.iter().any(|m| {
                    m.definition.noise_resolution > 64 && m.noise.iter().any(Option::is_some)
                }) {
                    return Err(MaterialError(
                        "Terrain material noise resolution is limited to 64³ per layer.".into(),
                    ));
                }
                Ok(TerrainMaterial { paint, layers })
            })
            .transpose()?;
        let mut noise: [Option<NoiseSettings>; 4] = Default::default();
        for (i, map) in self.maps().into_iter().enumerate() {
            if let Some(map) = map {
                noise[i] = Some(
                    NoiseDefinition::load(project, &map.asset)
                        .map_err(|e| MaterialError(format!("{}: {e}", map.asset)))?
                        .noise,
                );
            }
        }
        let evaluations = noise
            .iter()
            .flatten()
            .map(|n| n.octaves as usize)
            .sum::<usize>()
            * (self.noise_resolution as usize).pow(3);
        if evaluations > 8_388_608 {
            return Err(MaterialError("Material maps exceed 8,388,608 octave evaluations. Reduce map resolution, assigned maps or noise octaves.".into()));
        }
        for map in self.texture_maps() {
            let path = project
                .asset_path(&map.asset)
                .map_err(|e| MaterialError(e.to_string()))?;
            crate::TextureImage::dimensions_from_bytes(&crate::TextureImage::read(path)?)?;
        }
        let diffuse = self
            .active_diffuse_map()
            .map(|map| {
                let path = project
                    .asset_path(&map.asset)
                    .map_err(|e| MaterialError(e.to_string()))?;
                crate::TextureImage::load(path)
            })
            .transpose()?;
        Ok(Material {
            definition: self,
            shader,
            terrain,
            diffuse,
            uv_mapped: false,
            noise,
        })
    }
    /// Constructs a constant material without requiring a project or file system.
    pub fn constant(self) -> Result<Material, MaterialError> {
        self.validate()?;
        if self.shader.is_some()
            || self.asset.is_some()
            || self.terrain_paint.is_some()
            || self.maps().iter().any(|m| m.is_some())
            || self.texture_maps().next().is_some()
        {
            return Err(MaterialError(
                "Resolve shader, texture and material asset references against a project.".into(),
            ));
        }
        Ok(Material {
            definition: self,
            shader: None,
            noise: Default::default(),
            diffuse: None,
            uv_mapped: false,
            terrain: None,
        })
    }
}
impl Material {
    /// A constant material whose `image` is sampled with mesh texture coordinates, as
    /// imported models expect; the definition's albedo, if any, tints it.
    pub fn with_uv_image(
        definition: MaterialDefinition,
        image: crate::TextureImage,
    ) -> Result<Self, MaterialError> {
        let mut material = definition.constant()?;
        material.diffuse = Some(image);
        material.uv_mapped = true;
        Ok(material)
    }
    pub fn uv_mapped(&self) -> bool {
        self.uv_mapped
    }
    pub fn diffuse_image(&self) -> Option<&crate::TextureImage> {
        self.diffuse.as_ref()
    }
    pub fn shader(&self) -> Option<&crate::ShaderDefinition> {
        self.shader.as_ref()
    }
    pub fn definition(&self) -> &MaterialDefinition {
        &self.definition
    }
    pub fn noise_settings(&self) -> &[Option<NoiseSettings>; 4] {
        &self.noise
    }
    /// Bakes a bounded packed RGBA noise volume once during resource upload.
    #[cfg(feature = "graphics")]
    pub(crate) fn bake(&self) -> Result<(u32, Vec<u8>), MaterialError> {
        use crate::NoiseGenerator;
        if self.noise.iter().all(Option::is_none) {
            return Ok((
                1,
                [1.0, 1.0, 1.0, 0.5]
                    .into_iter()
                    .flat_map(|n| half::f16::from_f32(n).to_bits().to_le_bytes())
                    .collect(),
            ));
        }
        let side = self.definition.noise_resolution as usize;
        let mut generators = Vec::with_capacity(4);
        for settings in &self.noise {
            generators.push(
                settings
                    .clone()
                    .map(NoiseGenerator::new)
                    .transpose()
                    .map_err(|e| MaterialError(e.to_string()))?,
            );
        }
        let mut pixels = vec![0u8; side * side * side * 8];
        for z in 0..side {
            for y in 0..side {
                for x in 0..side {
                    let uv = [
                        x as f64 / (side - 1) as f64,
                        y as f64 / (side - 1) as f64,
                        z as f64 / (side - 1) as f64,
                    ];
                    for (channel, generator) in generators.iter().enumerate() {
                        let mut sample = 1.0;
                        if let (Some(generator), Some(settings)) = (generator, &self.noise[channel])
                        {
                            let p = std::array::from_fn::<_, 3, _>(|i| {
                                uv[i] * settings.extent + settings.offset[i]
                            });
                            let value = if settings.dimensions == 2 {
                                generator
                                    .sample_2d([p[0], uv[2] * settings.extent + settings.offset[1]])
                            } else {
                                generator.sample_3d(p)
                            }
                            .map_err(|e| MaterialError(e.to_string()))?;
                            sample = value;
                        }
                        let offset = (((z * side + y) * side + x) * 4 + channel) * 2;
                        pixels[offset..offset + 2]
                            .copy_from_slice(&half::f16::from_f32(sample).to_bits().to_le_bytes());
                    }
                }
            }
        }
        Ok((side as u32, pixels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_layers_are_bounded_and_selection_is_explicit() {
        let mut definition = MaterialDefinition {
            diffuse_map: Some(MaterialTextureMap {
                asset: "assets/grass.png".into(),
                scale: 1.,
            }),
            terrain_layers: vec![MaterialLayer {
                id: "rock".into(),
                name: "Rock".into(),
                diffuse_map: Some(MaterialTextureMap {
                    asset: "assets/rock.jpg".into(),
                    scale: 0.5,
                }),
            }],
            active_layer: Some("rock".into()),
            ..Default::default()
        };
        definition.validate().unwrap();
        assert_eq!(
            definition.active_diffuse_map().unwrap().asset,
            "assets/rock.jpg"
        );
        assert!(definition.clone().constant().is_err());
        assert_eq!(
            toml::from_str::<MaterialDefinition>(&toml::to_string(&definition).unwrap()).unwrap(),
            definition
        );
        definition.active_layer = Some("missing".into());
        assert!(definition.validate().is_err());
        definition.active_layer = None;
        definition
            .terrain_layers
            .push(definition.terrain_layers[0].clone());
        assert!(definition.validate().is_err());
        definition.terrain_layers.pop();
        definition.diffuse_map.as_mut().unwrap().scale = f32::NAN;
        assert!(definition.validate().is_err());
    }
    #[test]
    fn constant_materials_validate_parameters_and_round_trip() {
        let d = MaterialDefinition::default();
        assert!(d.cast_shadows && d.receive_shadows);
        assert_eq!(
            toml::from_str::<MaterialDefinition>(&toml::to_string(&d).unwrap()).unwrap(),
            d
        );
        assert!(d.constant().is_ok());
        for source in [
            "shading='unknown'",
            "cast_shadows='yes'",
            "receive_shadows=1",
            "albedo=[1.0,0.0]",
            "specular=[-1.0,0.0,0.0]",
            "shininess=0.0",
            "emission_intensity=nan",
            "bump_strength=3.0",
            "texture_tiling=[0.0,1.0]",
            "texture_tiling=[1.0,nan]",
            "unknown=true",
            "[bump_map]\nasset='bad.png'",
        ] {
            let result = toml::from_str::<MaterialDefinition>(source)
                .ok()
                .and_then(|d| d.constant().ok());
            assert!(result.is_none(), "accepted {source}");
        }
    }
    #[test]
    fn material_resolution_is_explicit_and_bounded() {
        for resolution in [32, 64, 128] {
            assert!(MaterialDefinition {
                noise_resolution: resolution,
                ..Default::default()
            }
            .validate()
            .is_ok());
        }
        assert!(MaterialDefinition {
            noise_resolution: 256,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
    #[cfg(feature = "graphics")]
    #[test]
    fn packed_noise_is_repeatable_seeded_and_respects_2d_projection() {
        let settings = NoiseSettings {
            size: 32,
            dimensions: 2,
            seed: 42,
            ..Default::default()
        };
        let mut material = Material {
            shader: None,
            terrain: None,
            diffuse: None,
            uv_mapped: false,
            definition: MaterialDefinition::default(),
            noise: [Some(settings), None, None, None],
        };
        let (side, pixels) = material.bake().unwrap();
        assert_eq!(side, 32);
        assert_eq!(pixels.len(), 32 * 32 * 32 * 8);
        assert_eq!(pixels, material.bake().unwrap().1);
        assert!(pixels
            .as_chunks::<8>()
            .0
            .iter()
            .any(|c| c[..2] != pixels[..2]));
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    assert_eq!(
                        pixels[((z * 32 + y) * 32 + x) * 8..((z * 32 + y) * 32 + x) * 8 + 2],
                        pixels[(z * 32 * 32 + x) * 8..(z * 32 * 32 + x) * 8 + 2]
                    );
                }
            }
        }
        material.noise[0].as_mut().unwrap().seed += 1;
        assert_ne!(pixels, material.bake().unwrap().1);
        let seeded = material.bake().unwrap().1;
        material.noise[0].as_mut().unwrap().offset[1] += 0.25;
        assert_ne!(seeded, material.bake().unwrap().1);
    }
}
