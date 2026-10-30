//! Versioned, bounded procedural noise assets and dense fields, independent of rendering.
use crate::Project;
use noise::{NoiseFn, OpenSimplex, Perlin, Worley};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

pub const NOISE_MAX_EVALUATIONS: usize = 2_097_152;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoiseAlgorithm {
    Perlin,
    OpenSimplex,
    Worley,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NoiseSettings {
    pub algorithm_version: u32,
    pub algorithm: NoiseAlgorithm,
    pub seed: u32,
    pub dimensions: u32,
    pub size: u32,
    pub frequency: f64,
    pub octaves: u32,
    pub lacunarity: f64,
    pub persistence: f64,
    pub extent: f64,
    pub offset: [f64; 3],
}
impl Default for NoiseSettings {
    fn default() -> Self {
        Self {
            algorithm_version: 1,
            algorithm: NoiseAlgorithm::Perlin,
            seed: 1,
            dimensions: 2,
            size: 128,
            frequency: 1.0,
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            extent: 8.0,
            offset: [0.0; 3],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseDefinition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub kind: String,
    pub template: String,
    pub runtime_status: String,
    pub noise: NoiseSettings,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoiseError(pub String);
impl fmt::Display for NoiseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for NoiseError {}
fn invalid(message: &str) -> NoiseError {
    NoiseError(message.into())
}
impl NoiseSettings {
    pub fn validate(&self) -> Result<usize, NoiseError> {
        self.validate_sampling()?;
        if self.size < 2 || self.size > if self.dimensions == 2 { 512 } else { 128 } {
            return Err(invalid(
                "Use dimensions 2 or 3, with resolution 2..512 in 2D or 2..128 in 3D.",
            ));
        }
        let count = (self.size as usize).pow(self.dimensions);
        if count * self.octaves as usize > NOISE_MAX_EVALUATIONS {
            return Err(invalid(
                "Field exceeds 2,097,152 octave evaluations. Reduce resolution or octaves.",
            ));
        }
        Ok(count)
    }
    fn validate_sampling(&self) -> Result<(), NoiseError> {
        if self.algorithm_version != 1 {
            return Err(invalid("Unsupported noise algorithm version; expected 1 (noise-rs 0.9.0 / Emergent octave normalization v1)."));
        }
        if !matches!(self.dimensions, 2 | 3) {
            return Err(invalid("Use dimensions 2 or 3."));
        }
        if !(1..=8).contains(&self.octaves) {
            return Err(invalid("Octaves must be 1..8."));
        }
        for (value, min, max, name) in [
            (self.frequency, 0.0001, 64.0, "Frequency"),
            (self.lacunarity, 1.0, 4.0, "Lacunarity"),
            (self.persistence, 0.0, 1.0, "Persistence"),
            (self.extent, 0.001, 1024.0, "Domain extent"),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(NoiseError(format!(
                    "{name} must be finite and in {min}..{max}."
                )));
            }
        }
        if !self
            .offset
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 10000.0)
        {
            return Err(invalid(
                "Offset axes must be finite and within -10000..10000.",
            ));
        }
        if self
            .offset
            .iter()
            .any(|v| (v.abs() + self.extent) * self.maximum_frequency() > 1e8)
        {
            return Err(invalid("Noise coordinates exceed the supported range; reduce frequency, lacunarity, octaves or offset."));
        }
        Ok(())
    }
    fn maximum_frequency(&self) -> f64 {
        self.frequency * self.lacunarity.powi(self.octaves as i32 - 1)
    }
}
impl NoiseDefinition {
    pub fn from_source(source: &str) -> Result<Self, NoiseError> {
        if source.len() > 256 * 1024 {
            return Err(invalid("Noise definition exceeds 256 KiB."));
        }
        let definition: Self = toml::from_str(source).map_err(|e| NoiseError(e.to_string()))?;
        if definition.schema_version != 1
            || definition.kind != "noise-function"
            || definition.id.is_empty()
            || definition.id.len() > 128
            || !definition
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        {
            return Err(invalid(
                "Noise asset requires schema 1, kind noise-function and a valid durable ID.",
            ));
        }
        definition.noise.validate()?;
        Ok(definition)
    }
    pub fn load(project: &Project, path: &str) -> Result<Self, NoiseError> {
        if !path.ends_with(".noise.toml") {
            return Err(invalid("Choose a .noise.toml filter asset."));
        }
        use std::io::Read;
        let path = project
            .asset_path(path)
            .map_err(|e| NoiseError(e.to_string()))?;
        let mut source = String::new();
        std::fs::File::open(path)
            .and_then(|file| file.take(256 * 1024 + 1).read_to_string(&mut source))
            .map_err(|e| NoiseError(e.to_string()))?;
        Self::from_source(&source)
    }
}

enum Source {
    Perlin(Perlin),
    OpenSimplex(OpenSimplex),
    Worley(Worley),
}
impl Source {
    fn sample(&self, p: [f64; 3], dimensions: u32) -> f64 {
        macro_rules! get {
            ($source:expr) => {
                if dimensions == 2 {
                    $source.get([p[0], p[1]])
                } else {
                    $source.get(p)
                }
            };
        }
        match self {
            Self::Perlin(s) => get!(s),
            Self::OpenSimplex(s) => get!(s),
            Self::Worley(s) => get!(s),
        }
    }
}
/// Prepared once; each sample reuses seeded generators without allocating.
/// Worley uses the dependency's local distance callback; prepare a generator per worker thread.
pub struct NoiseGenerator {
    settings: NoiseSettings,
    sources: Vec<Source>,
}
impl NoiseGenerator {
    pub fn new(settings: NoiseSettings) -> Result<Self, NoiseError> {
        settings.validate()?;
        Ok(Self::prepare(settings))
    }
    /// Point samplers allocate only octave sources; the caller bounds its own sample workload.
    pub(crate) fn for_sampling(settings: NoiseSettings) -> Result<Self, NoiseError> {
        settings.validate_sampling()?;
        Ok(Self::prepare(settings))
    }
    fn prepare(settings: NoiseSettings) -> Self {
        let sources = (0..settings.octaves)
            .map(|octave| {
                let seed = settings.seed.wrapping_add(octave.wrapping_mul(0x9E3779B9));
                match settings.algorithm {
                    NoiseAlgorithm::Perlin => Source::Perlin(Perlin::new(seed)),
                    NoiseAlgorithm::OpenSimplex => Source::OpenSimplex(OpenSimplex::new(seed)),
                    NoiseAlgorithm::Worley => Source::Worley(
                        Worley::new(seed)
                            .set_return_type(noise::core::worley::ReturnType::Distance),
                    ),
                }
            })
            .collect();
        Self { settings, sources }
    }
    fn sample(&self, point: [f64; 3], dimensions: u32) -> Result<f32, NoiseError> {
        if !point
            .iter()
            .all(|v| v.is_finite() && v.abs() * self.settings.maximum_frequency() <= 1e8)
        {
            return Err(invalid(
                "Sample coordinates are non-finite or outside the supported range.",
            ));
        }
        let (mut frequency, mut amplitude, mut total, mut weight) =
            (self.settings.frequency, 1.0, 0.0, 0.0);
        for source in &self.sources {
            total += source.sample(point.map(|v| v * frequency), dimensions) * amplitude;
            weight += amplitude;
            frequency *= self.settings.lacunarity;
            amplitude *= self.settings.persistence;
        }
        let value = (total / weight * 0.5 + 0.5).clamp(0.0, 1.0);
        if !value.is_finite() {
            return Err(invalid("Noise generator produced a non-finite sample."));
        }
        Ok(value as f32)
    }
    pub fn sample_2d(&self, point: [f64; 2]) -> Result<f32, NoiseError> {
        self.sample([point[0], point[1], 0.0], 2)
    }
    pub fn sample_3d(&self, point: [f64; 3]) -> Result<f32, NoiseError> {
        self.sample(point, 3)
    }
    pub fn generate(&self) -> Result<NoiseField, NoiseError> {
        let count = self.settings.validate()?;
        let size = self.settings.size;
        let depth = if self.settings.dimensions == 3 {
            size
        } else {
            1
        };
        let mut values = Vec::with_capacity(count);
        for z in 0..depth {
            for y in 0..size {
                for x in 0..size {
                    let point =
                        [x, y, z].map(|n| self.settings.extent * n as f64 / (size - 1) as f64);
                    let point =
                        std::array::from_fn(|axis| point[axis] + self.settings.offset[axis]);
                    values.push(self.sample(point, self.settings.dimensions)?);
                }
            }
        }
        Ok(NoiseField {
            width: size,
            height: size,
            depth,
            values,
        })
    }
}
/// Dense normalized data. Index = x + width * (y + height * z).
pub struct NoiseField {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub values: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeded_algorithms_are_repeatable_bounded_and_distinct() {
        let mut fields = Vec::new();
        for algorithm in [
            NoiseAlgorithm::Perlin,
            NoiseAlgorithm::OpenSimplex,
            NoiseAlgorithm::Worley,
        ] {
            let settings = NoiseSettings {
                algorithm,
                size: 16,
                ..Default::default()
            };
            let generator = NoiseGenerator::new(settings.clone()).unwrap();
            let a = generator.generate().unwrap();
            let expected = match algorithm {
                NoiseAlgorithm::Perlin => 0.62752867,
                NoiseAlgorithm::OpenSimplex => 0.49876,
                NoiseAlgorithm::Worley => 0.6209161,
            };
            assert!(
                (generator.sample_3d([0.31, 0.73, -0.28]).unwrap() - expected).abs() < 1e-6,
                "version-1 golden sample changed"
            );
            assert_eq!(a.values, generator.generate().unwrap().values);
            assert!(a.values.iter().all(|v| (0.0..=1.0).contains(v)));
            assert_ne!(
                a.values,
                NoiseGenerator::new(NoiseSettings {
                    seed: 2,
                    ..settings
                })
                .unwrap()
                .generate()
                .unwrap()
                .values
            );
            fields.push(a.values);
        }
        assert_ne!(fields[0], fields[1]);
        assert_ne!(fields[1], fields[2]);
    }
    #[test]
    fn volumes_are_full_3d_fields_with_x_fastest_indexing() {
        let generator = NoiseGenerator::new(NoiseSettings {
            dimensions: 3,
            size: 8,
            extent: 2.5,
            ..Default::default()
        })
        .unwrap();
        let field = generator.generate().unwrap();
        assert_eq!(field.values.len(), 512);
        assert_eq!(field.depth, 8);
        assert_ne!(&field.values[..64], &field.values[64..128]);
        assert_eq!(
            field.values[3 + 8 * (2 + 8 * 4)],
            generator
                .sample_3d([2.5 * 3.0 / 7.0, 2.5 * 2.0 / 7.0, 2.5 * 4.0 / 7.0])
                .unwrap()
        );
        assert_eq!(
            NoiseGenerator::new(NoiseSettings {
                size: 8,
                ..Default::default()
            })
            .unwrap()
            .sample_2d([0.0, 0.0])
            .unwrap(),
            0.5
        );
    }
    #[test]
    fn prepared_point_sampling_preserves_field_work_limits() {
        let settings = NoiseSettings {
            size: 1024,
            octaves: 8,
            ..Default::default()
        };
        assert!(NoiseGenerator::new(settings.clone()).is_err());
        let sampler = NoiseGenerator::for_sampling(settings).unwrap();
        assert!(sampler.sample_2d([0.25, 0.75]).unwrap().is_finite());
        assert!(sampler.generate().is_err());
        assert!(NoiseGenerator::for_sampling(NoiseSettings {
            frequency: f64::NAN,
            ..Default::default()
        })
        .is_err());
        let settings = NoiseSettings::default();
        assert_eq!(
            NoiseGenerator::new(settings.clone())
                .unwrap()
                .sample_2d([0.25, 0.75])
                .unwrap(),
            NoiseGenerator::for_sampling(settings)
                .unwrap()
                .sample_2d([0.25, 0.75])
                .unwrap()
        );
    }
    #[test]
    fn invalid_inputs_and_work_budgets_fail_before_generation() {
        for settings in [
            NoiseSettings {
                frequency: f64::NAN,
                ..Default::default()
            },
            NoiseSettings {
                dimensions: 4,
                ..Default::default()
            },
            NoiseSettings {
                size: 0,
                ..Default::default()
            },
            NoiseSettings {
                dimensions: 3,
                size: 128,
                octaves: 8,
                ..Default::default()
            },
            NoiseSettings {
                persistence: -0.1,
                ..Default::default()
            },
            NoiseSettings {
                algorithm_version: 2,
                ..Default::default()
            },
        ] {
            assert!(NoiseGenerator::new(settings).is_err());
        }
        let generator = NoiseGenerator::new(Default::default()).unwrap();
        assert!(generator.sample_3d([f64::INFINITY, 0.0, 0.0]).is_err());
        assert!(NoiseDefinition::from_source("schema_version=99").is_err());
    }
}
