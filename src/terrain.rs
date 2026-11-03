//! Bounded, versioned height-field preparation, independent of the game loop.
use crate::{Color, GeometryError, Mesh, MeshVertex, NoiseError, NoiseGenerator, NoiseSettings};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerrainPreset {
    Plain,
    Canyon,
    Underwater,
    Hilly,
    Mountainous,
    Rivers,
    Island,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerrainBase {
    Noise,
    DiamondSquare,
}
/// Supported dense terrain sample grids; sizes are samples per side.
pub const TERRAIN_RESOLUTIONS: [u32; 6] = [32, 64, 128, 256, 512, 1024];
pub(crate) const TERRAIN_DATA_MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Versioned recipe. Sizes are metres; resolution is samples per side.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct TerrainSettings {
    pub algorithm_version: u32,
    pub preset: TerrainPreset,
    pub generator: TerrainBase,
    pub seed: u32,
    pub resolution: u32,
    pub size: f64,
    pub height: f64,
    pub frequency: f64,
    pub octaves: u32,
    pub roughness: f64,
    pub mountain_count: u32,
    pub domain_warp: f64,
    pub ridge_strength: f64,
    pub erosion_iterations: u32,
}
impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            algorithm_version: 2,
            preset: TerrainPreset::Hilly,
            generator: TerrainBase::Noise,
            seed: 42,
            resolution: 128,
            size: 100.,
            height: 18.,
            frequency: 0.65,
            octaves: 4,
            roughness: 0.45,
            mountain_count: 3,
            domain_warp: 0.25,
            ridge_strength: 0.5,
            erosion_iterations: 16,
        }
    }
}
impl TerrainSettings {
    pub fn validate(&self) -> Result<(), NoiseError> {
        if !matches!(self.algorithm_version, 1 | 2)
            || !TERRAIN_RESOLUTIONS.contains(&self.resolution)
            || !(1..=8).contains(&self.octaves)
            || !(1..=12).contains(&self.mountain_count)
            || self.erosion_iterations > 64
        {
            return Err(NoiseError("Terrain requires version 1 or 2, resolution 32/64/128/256/512/1024, octaves 1..8, mountains 1..12 and erosion passes 0..64.".into()));
        }
        for (value, min, max) in [
            (self.size, 1., 10000.),
            (self.height, 0., 1000.),
            (self.frequency, 0.1, 8.),
            (self.roughness, 0., 1.),
            (self.domain_warp, 0., 1.),
            (self.ridge_strength, 0., 1.),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(NoiseError(
                    "Terrain numeric setting is outside its finite bounds.".into(),
                ));
            }
        }
        Ok(())
    }
    /// Matched comparison recipes; the enhanced recipe retains the requested settings.
    pub fn comparison(&self) -> [Self; 3] {
        let mut baseline = *self;
        baseline.algorithm_version = 1;
        baseline.generator = TerrainBase::Noise;
        baseline.domain_warp = 0.;
        baseline.ridge_strength = 0.;
        baseline.erosion_iterations = 0;
        let mut diamond = baseline;
        diamond.generator = TerrainBase::DiamondSquare;
        [baseline, diamond, *self]
    }
    pub fn generate(&self) -> Result<TerrainField, NoiseError> {
        self.validate()?;
        let n = self.resolution as usize;
        let noise = NoiseGenerator::for_sampling(NoiseSettings {
            seed: self.seed,
            size: self.resolution,
            frequency: self.frequency,
            octaves: self.octaves,
            persistence: self.roughness,
            extent: 6.,
            ..NoiseSettings::default()
        })?;
        let diamond = if self.generator == TerrainBase::DiamondSquare {
            Some(diamond_square(n, self.seed, self.roughness))
        } else {
            None
        };
        let warp = NoiseGenerator::new(NoiseSettings {
            seed: self.seed ^ 0x9e3779b9,
            octaves: 2,
            ..NoiseSettings::default()
        })?;
        let peaks = peaks(self.seed, self.mountain_count as usize);
        let summits = if self.algorithm_version == 2 && self.preset == TerrainPreset::Mountainous {
            mountain_summits(self.seed, peaks.len())
        } else {
            Vec::new()
        };
        let ridges: Vec<_> =
            if self.preset == TerrainPreset::Mountainous && self.ridge_strength > 0. {
                peaks
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| {
                        peaks
                            .iter()
                            .enumerate()
                            .filter(|(j, _)| *j != i)
                            .min_by(|(_, a), (_, b)| distance2(p, a).total_cmp(&distance2(p, b)))
                            .map(|(_, q)| (p, q))
                    })
                    .collect()
            } else {
                Vec::new()
            };
        let mut values = Vec::with_capacity(n * n);
        let phase = (self.seed % 360) as f64 * std::f64::consts::PI / 180.;
        for z in 0..n {
            for x in 0..n {
                let u = x as f64 / (n - 1) as f64;
                let v = z as f64 / (n - 1) as f64;
                let (mut a, mut b) = (u, v);
                if self.domain_warp > 0. {
                    a += (warp.sample_2d([u * 3. + 11., v * 3. + 7.])? as f64 - 0.5)
                        * self.domain_warp
                        * 0.2;
                    b += (warp.sample_2d([u * 3. + 37., v * 3. + 53.])? as f64 - 0.5)
                        * self.domain_warp
                        * 0.2;
                }
                let base = match &diamond {
                    Some(field) => bilinear(field, n + 1, a, b),
                    None => noise.sample_2d([a * 6., b * 6.])? as f64,
                };
                let mut value = base.powf(1.5);
                match self.preset {
                    TerrainPreset::Plain => value = 0.05 + 0.1 * base,
                    TerrainPreset::Underwater => value = 0.05 + 0.18 * base,
                    TerrainPreset::Canyon => {
                        let d = (u - 0.5 - 0.1 * (v * std::f64::consts::TAU + phase).sin()).abs();
                        let t = ((d - 0.04) / 0.11).clamp(0., 1.);
                        value = 0.1 + 0.1 * base + 0.75 * t * t * (3. - 2. * t);
                    }
                    TerrainPreset::Hilly => {}
                    TerrainPreset::Mountainous => {
                        let mut mountain: f64 = 0.;
                        for (index, peak) in peaks.iter().enumerate() {
                            if self.algorithm_version == 2 {
                                mountain = mountain.max(massif_height(
                                    peak,
                                    &summits[index],
                                    a,
                                    b,
                                    base,
                                    self.roughness,
                                ));
                                continue;
                            }
                            let dx = a - peak.x;
                            let dz = b - peak.z;
                            let d = ((dx * peak.cos + dz * peak.sin) / peak.rx)
                                .hypot((-dx * peak.sin + dz * peak.cos) / peak.rz)
                                * (0.8 + 0.4 * base);
                            mountain = mountain.max(peak.height * (1. - d).max(0.).powf(1.4));
                        }
                        for (p, q) in &ridges {
                            let dx = q.x - p.x;
                            let dz = q.z - p.z;
                            let t = (((a - p.x) * dx + (b - p.z) * dz) / (dx * dx + dz * dz))
                                .clamp(0., 1.);
                            let d = (a - p.x - t * dx).hypot(b - p.z - t * dz)
                                / (p.rz.min(q.rz) * 0.65);
                            let ridge = (1. - d).max(0.).powf(1.5)
                                * (p.height * (1. - t) + q.height * t)
                                * 0.65
                                * self.ridge_strength;
                            mountain = mountain.max(ridge);
                        }
                        value = (mountain * (1. + self.roughness * 0.08 * (2. * base - 1.))
                            + self.roughness * 0.025 * base)
                            .min(1.);
                    }
                    TerrainPreset::Rivers => {
                        let d = u - 0.5 - 0.17 * (v * std::f64::consts::TAU + phase).sin();
                        value = (0.15 + 0.7 * base - 0.8 * (-d * d / 0.0025).exp()).max(0.);
                    }
                    TerrainPreset::Island => {
                        let edge = (1. - 4. * ((u - 0.5).powi(2) + (v - 0.5).powi(2))).max(0.);
                        value = (0.25 + 0.75 * base) * edge * edge;
                    }
                }
                values.push((value * self.height) as f32);
            }
        }
        erode(
            &mut values,
            n,
            self.size / (n - 1) as f64,
            self.erosion_iterations,
        );
        Ok(TerrainField {
            settings: *self,
            values,
        })
    }
}
// Authored snapshots are immutable after scene construction. Reload replaces the owning EntityId.
#[cfg(feature = "physics")]
#[derive(bevy_ecs::component::Component)]
pub(crate) struct AuthoredTerrain(pub TerrainField);

/// Dense row-major world-space heights. No entity identity is created during preparation.
#[derive(Clone)]
pub struct TerrainField {
    settings: TerrainSettings,
    values: Vec<f32>,
}
impl TerrainField {
    /// Validate a frozen authored field using the same extent and height bounds as generation.
    pub fn from_heights(settings: TerrainSettings, values: Vec<f32>) -> Result<Self, NoiseError> {
        settings.validate()?;
        if values.len() != settings.resolution as usize * settings.resolution as usize
            || values
                .iter()
                .any(|v| !v.is_finite() || *v < 0. || *v > settings.height as f32 + 0.0001)
        {
            return Err(NoiseError("Invalid authored terrain height field.".into()));
        }
        Ok(Self { settings, values })
    }

    pub fn settings(&self) -> &TerrainSettings {
        &self.settings
    }
    pub fn values(&self) -> &[f32] {
        &self.values
    }
    /// Prepare a smooth, untextured mesh once, before uploading to the renderer.
    pub fn mesh(&self, color: Color) -> Result<Mesh, GeometryError> {
        let n = self.settings.resolution as usize;
        let step = self.settings.size as f32 / (n - 1) as f32;
        let mut vertices = Vec::with_capacity(n * n);
        let mut indices = Vec::with_capacity((n - 1) * (n - 1) * 6);
        for z in 0..n {
            for x in 0..n {
                let l = x.saturating_sub(1);
                let r = (x + 1).min(n - 1);
                let u = z.saturating_sub(1);
                let d = (z + 1).min(n - 1);
                let dx =
                    (self.values[z * n + r] - self.values[z * n + l]) / ((r - l) as f32 * step);
                let dz =
                    (self.values[d * n + x] - self.values[u * n + x]) / ((d - u) as f32 * step);
                vertices.push(MeshVertex::with_normal(
                    Vec3::new(
                        x as f32 * step - self.settings.size as f32 / 2.,
                        self.values[z * n + x],
                        z as f32 * step - self.settings.size as f32 / 2.,
                    ),
                    Vec3::new(-dx, 1., -dz),
                    color,
                )?);
                if x < n - 1 && z < n - 1 {
                    let i = (z * n + x) as u32;
                    let n = n as u32;
                    indices.extend([i, i + n, i + 1, i + 1, i + n, i + n + 1]);
                }
            }
        }
        Mesh::new(vertices, indices)
    }
}
struct Random(u32);
impl Random {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0 as f64 / 4294967296.
    }
}
struct Peak {
    x: f64,
    z: f64,
    rx: f64,
    rz: f64,
    cos: f64,
    sin: f64,
    height: f64,
}
fn distance2(p: &Peak, q: &Peak) -> f64 {
    (p.x - q.x).powi(2) + (p.z - q.z).powi(2)
}
fn peaks(seed: u32, count: usize) -> Vec<Peak> {
    let mut rng = Random(seed);
    let columns = (count as f64).sqrt().ceil() as usize;
    let rows = count.div_ceil(columns);
    let mut cells: Vec<_> = (0..columns * rows).collect();
    for i in (1..cells.len()).rev() {
        let j = (rng.next() * (i + 1) as f64) as usize;
        cells.swap(i, j);
    }
    let radius = (0.3 / (count as f64).sqrt()).min(0.35 / columns.max(rows) as f64);
    cells
        .into_iter()
        .take(count)
        .map(|cell| {
            let x = ((cell % columns) as f64 + 0.4 + rng.next() * 0.2) / columns as f64;
            let z = ((cell / columns) as f64 + 0.4 + rng.next() * 0.2) / rows as f64;
            let rx = radius * (0.85 + rng.next() * 0.15);
            let rz = rx * (0.6 + rng.next() * 0.3);
            let angle = rng.next() * std::f64::consts::TAU;
            Peak {
                x,
                z,
                rx,
                rz,
                cos: angle.cos(),
                sin: angle.sin(),
                height: 0.7 + rng.next() * 0.2,
            }
        })
        .collect()
}
struct Summit {
    x: f64,
    z: f64,
    height: f64,
    width: f64,
}
fn mountain_summits(seed: u32, count: usize) -> Vec<[Summit; 4]> {
    let mut rng = Random(seed ^ 0x6a09e667);
    (0..count)
        .map(|_| {
            std::array::from_fn(|i| Summit {
                x: -0.65 + i as f64 * 0.43 + (rng.next() - 0.5) * 0.12,
                z: (rng.next() - 0.5) * 0.5,
                height: 0.8 + 0.2 * rng.next(),
                width: 0.4 + 0.12 * rng.next(),
            })
        })
        .collect()
}
fn massif_height(
    peak: &Peak,
    summits: &[Summit; 4],
    u: f64,
    v: f64,
    base: f64,
    roughness: f64,
) -> f64 {
    let dx = u - peak.x;
    let dz = v - peak.z;
    let x = (dx * peak.cos + dz * peak.sin) / (peak.rx * 1.18);
    let z = (-dx * peak.sin + dz * peak.cos) / (peak.rz * 1.3);
    if x.abs() > 1.6 || z.abs() > 1.6 {
        return 0.;
    }
    // Local contour bends give shoulders an irregular outline even at zero surface roughness.
    let x = x + 0.12 * (z * 4. + peak.sin * 3.).sin();
    let z = z + 0.1 * (x * 5. + peak.cos * 4.).sin();
    let shoulder = (1. - (x / 1.25).powi(2) - (z / 1.3).powi(2))
        .max(0.)
        .powf(1.6);
    let mut crest: f64 = 0.;
    for pair in summits.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        let dx = b.x - a.x;
        let dz = b.z - a.z;
        let t = (((x - a.x) * dx + (z - a.z) * dz) / (dx * dx + dz * dz)).clamp(0., 1.);
        let width = a.width * (1. - t) + b.width * t;
        let distance2 = ((x - a.x - t * dx).powi(2) + (z - a.z - t * dz).powi(2)) / (width * width);
        let saddle = 1. - 0.3 * (std::f64::consts::PI * t).sin().powi(2);
        let height = (a.height * (1. - t) + b.height * t) * saddle;
        crest = crest.max(height * (1. - distance2).max(0.).powf(1.4));
    }
    peak.height * (0.28 * shoulder + 0.72 * crest) * (1. + roughness * 0.2 * (2. * base - 1.))
}
// shortcut: independent diamond-square patches have no shared edge contract; add one before chunk streaming.
fn diamond_square(cells: usize, seed: u32, roughness: f64) -> Vec<f32> {
    let n = cells + 1;
    let mut field = vec![0.; n * n];
    let mut rng = Random(seed ^ 0xa511e9b3);
    for (x, z) in [(0, 0), (cells, 0), (0, cells), (cells, cells)] {
        field[z * n + x] = rng.next() as f32;
    }
    let mut step = cells;
    let mut amplitude = 0.5;
    while step > 1 {
        let half = step / 2;
        for z in (half..cells).step_by(step) {
            for x in (half..cells).step_by(step) {
                let mean = (field[(z - half) * n + x - half]
                    + field[(z - half) * n + x + half]
                    + field[(z + half) * n + x - half]
                    + field[(z + half) * n + x + half])
                    * 0.25;
                field[z * n + x] = (mean + (rng.next() as f32 * 2. - 1.) * amplitude).clamp(0., 1.);
            }
        }
        for z in (0..=cells).step_by(half) {
            for x in ((if (z / half).is_multiple_of(2) {
                half
            } else {
                0
            })..=cells)
                .step_by(step)
            {
                let mut sum = 0.;
                let mut count = 0.;
                for (dx, dz) in [
                    (-(half as isize), 0),
                    (half as isize, 0),
                    (0, -(half as isize)),
                    (0, half as isize),
                ] {
                    let xx = x as isize + dx;
                    let zz = z as isize + dz;
                    if xx >= 0 && zz >= 0 && xx <= cells as isize && zz <= cells as isize {
                        sum += field[zz as usize * n + xx as usize];
                        count += 1.;
                    }
                }
                field[z * n + x] =
                    (sum / count + (rng.next() as f32 * 2. - 1.) * amplitude).clamp(0., 1.);
            }
        }
        amplitude *= roughness as f32;
        step = half;
    }
    field
}
fn bilinear(field: &[f32], n: usize, u: f64, v: f64) -> f64 {
    let x = u.clamp(0., 1.) * (n - 1) as f64;
    let z = v.clamp(0., 1.) * (n - 1) as f64;
    let a = x.floor() as usize;
    let b = z.floor() as usize;
    let r = (a + 1).min(n - 1);
    let d = (b + 1).min(n - 1);
    let tx = x - a as f64;
    let tz = z - b as f64;
    (field[b * n + a] as f64 * (1. - tx) + field[b * n + r] as f64 * tx) * (1. - tz)
        + (field[d * n + a] as f64 * (1. - tx) + field[d * n + r] as f64 * tx) * tz
}
fn erode(values: &mut [f32], n: usize, step: f64, iterations: u32) {
    if iterations == 0 {
        return;
    }
    let mut delta = vec![0.; values.len()];
    let talus = (step * 0.6) as f32;
    for _ in 0..iterations {
        delta.fill(0.);
        for z in 0..n {
            for x in 0..n {
                let i = z * n + x;
                for j in [
                    if x > 0 { Some(i - 1) } else { None },
                    if x + 1 < n { Some(i + 1) } else { None },
                    if z > 0 { Some(i - n) } else { None },
                    if z + 1 < n { Some(i + n) } else { None },
                ]
                .into_iter()
                .flatten()
                {
                    // Four outgoing transfers together cannot overshoot a donor's available height.
                    let amount = ((values[i] - values[j] - talus).max(0.)) * 0.125;
                    delta[i] -= amount;
                    delta[j] += amount;
                }
            }
        }
        for (value, change) in values.iter_mut().zip(&delta) {
            *value += change;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn high_resolution_fields_keep_full_mesh_indices() {
        for resolution in [512, 1024] {
            let settings = TerrainSettings {
                resolution,
                ..Default::default()
            };
            settings.validate().unwrap();
            let count = resolution as usize * resolution as usize;
            let field = TerrainField::from_heights(settings, vec![0.; count]).unwrap();
            let mesh = field.mesh(Color::WHITE).unwrap();
            assert_eq!(mesh.vertices().len(), count);
            assert_eq!(mesh.indices().len(), (resolution as usize - 1).pow(2) * 6);
            assert_eq!(mesh.indices().iter().copied().max(), Some(count as u32 - 1));
        }
    }
    #[test]
    fn repeatable_bounded_distinct_and_versioned() {
        let mut s = TerrainSettings {
            resolution: 32,
            ..Default::default()
        };
        for preset in [
            TerrainPreset::Hilly,
            TerrainPreset::Mountainous,
            TerrainPreset::Rivers,
            TerrainPreset::Island,
        ] {
            s.preset = preset;
            let variants = s.comparison();
            let mut prior = Vec::new();
            for recipe in variants {
                let field = recipe.generate().unwrap();
                assert_eq!(field.values, recipe.generate().unwrap().values);
                assert!(field
                    .values
                    .iter()
                    .all(|h| h.is_finite() && *h >= 0. && *h <= s.height as f32));
                for other in &prior {
                    assert_ne!(&field.values, other);
                }
                prior.push(field.values);
            }
        }
        assert_eq!(
            toml::from_str::<TerrainSettings>(&toml::to_string(&s).unwrap()).unwrap(),
            s
        );
        s.algorithm_version = 3;
        assert!(s.generate().is_err());
        s.algorithm_version = 1;
        for res in [0, 31, 513, 2048, u32::MAX] {
            s.resolution = res;
            assert!(s.generate().is_err());
        }
        s.resolution = 32;
        s.erosion_iterations = 65;
        assert!(s.generate().is_err());
    }
    #[test]
    fn seed_changes_output_flat_stays_flat_and_island_edges_are_zero_without_erosion() {
        let mut s = TerrainSettings {
            resolution: 32,
            ..Default::default()
        };
        let a = s.generate().unwrap();
        s.seed += 1;
        assert_ne!(a.values, s.generate().unwrap().values);
        s.height = 0.;
        let flat = s.generate().unwrap();
        assert!(flat.values.iter().all(|h| *h == 0.));
        let mesh = flat.mesh(Color::WHITE).unwrap();
        assert_eq!(mesh.indices().len(), 31 * 31 * 6);
        assert!(mesh
            .vertices()
            .iter()
            .all(|v| v.normal().unwrap().dot(Vec3::Y) > 0.999));
        s.height = 18.;
        s.preset = TerrainPreset::Island;
        s.erosion_iterations = 0;
        let island = s.generate().unwrap();
        for i in 0..32 {
            for j in [i, 31 * 32 + i, i * 32, i * 32 + 31] {
                assert_eq!(island.values[j], 0.);
            }
        }
    }
    #[test]
    fn erosion_conserves_mass_and_reduces_a_peak() {
        let mut values = vec![0.; 32 * 32];
        values[16 * 32 + 16] = 100.;
        erode(&mut values, 32, 1., 32);
        assert!((values.iter().sum::<f32>() - 100.).abs() < 0.001);
        assert!(values.iter().all(|h| *h >= 0. && *h < 100.));
        assert!(values[16 * 32 + 16] < 10.);
    }
    #[test]
    fn isolated_major_mountains_keep_the_requested_count() {
        for count in [1, 3, 8, 12] {
            let settings = TerrainSettings {
                preset: TerrainPreset::Mountainous,
                resolution: 128,
                mountain_count: count,
                roughness: 0.,
                domain_warp: 0.,
                ridge_strength: 0.,
                erosion_iterations: 0,
                ..Default::default()
            };
            let field = settings.generate().unwrap();
            let mut seen = vec![false; 128 * 128];
            let mut regions = 0;
            for i in 0..field.values.len() {
                if seen[i] || field.values[i] <= settings.height as f32 * 0.05 {
                    continue;
                }
                regions += 1;
                seen[i] = true;
                let mut pending = vec![i];
                while let Some(i) = pending.pop() {
                    let x = i % 128;
                    let z = i / 128;
                    for j in [
                        if x > 0 { Some(i - 1) } else { None },
                        if x < 127 { Some(i + 1) } else { None },
                        if z > 0 { Some(i - 128) } else { None },
                        if z < 127 { Some(i + 128) } else { None },
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if !seen[j] && field.values[j] > settings.height as f32 * 0.05 {
                            seen[j] = true;
                            pending.push(j);
                        }
                    }
                }
            }
            assert_eq!(regions, count);
        }
    }
    #[test]
    fn slopes_have_unit_upward_normals_and_extreme_valid_recipes_are_bounded() {
        let settings = TerrainSettings {
            resolution: 32,
            ..Default::default()
        };
        let mesh = settings.generate().unwrap().mesh(Color::WHITE).unwrap();
        for v in mesh.vertices() {
            let normal = v.normal().unwrap();
            assert!((normal.length() - 1.).abs() < 1e-5);
            assert!(normal.y > 0.);
        }
        for triangle in mesh.indices().as_chunks::<3>().0 {
            let p: Vec<_> = triangle
                .iter()
                .map(|i| mesh.vertices()[*i as usize].position())
                .collect();
            assert!((p[1] - p[0]).cross(p[2] - p[0]).y > 0.);
        }
        for generator in [TerrainBase::Noise, TerrainBase::DiamondSquare] {
            let settings = TerrainSettings {
                generator,
                resolution: 256,
                seed: u32::MAX,
                size: 1.,
                height: 1000.,
                frequency: 8.,
                octaves: 8,
                roughness: 1.,
                mountain_count: 12,
                domain_warp: 1.,
                ridge_strength: 1.,
                erosion_iterations: 64,
                ..Default::default()
            };
            let field = settings.generate().unwrap();
            assert!(field
                .values
                .iter()
                .all(|h| h.is_finite() && *h >= 0. && *h <= 1000.));
        }
    }
    #[test]
    fn massif_has_multiple_substantial_summits_and_keeps_ridge_links() {
        for seed in [42, 7, 99] {
            let settings = TerrainSettings {
                preset: TerrainPreset::Mountainous,
                seed,
                resolution: 128,
                height: 38.,
                mountain_count: 1,
                ..Default::default()
            };
            let field = settings.generate().unwrap();
            let mut summits = 0;
            for z in 3..125 {
                for x in 3..125 {
                    let h = field.values[z * 128 + x];
                    if h <= settings.height as f32 * 0.35 {
                        continue;
                    }
                    let is_summit = (-3isize..=3).all(|dz| {
                        (-3isize..=3).all(|dx| {
                            (dx == 0 && dz == 0)
                                || h > field.values
                                    [(z as isize + dz) as usize * 128 + (x as isize + dx) as usize]
                        })
                    });
                    if is_summit {
                        summits += 1;
                    }
                }
            }
            assert!(
                summits >= 2,
                "seed {seed} has only {summits} substantial summits"
            );
            assert_eq!(field.values, settings.generate().unwrap().values);
        }
        let mut settings = TerrainSettings {
            preset: TerrainPreset::Mountainous,
            resolution: 128,
            mountain_count: 2,
            domain_warp: 0.,
            ridge_strength: 0.,
            erosion_iterations: 0,
            ..Default::default()
        };
        let separate = settings.generate().unwrap();
        settings.ridge_strength = 1.;
        let connected = settings.generate().unwrap();
        assert!(
            separate
                .values
                .iter()
                .zip(&connected.values)
                .filter(|(a, b)| **b - **a > settings.height as f32 * 0.05)
                .count()
                > 20
        );
    }
    #[test]
    fn explicit_v1_recipes_retain_their_previous_heights() {
        let settings = TerrainSettings {
            algorithm_version: 1,
            preset: TerrainPreset::Mountainous,
            height: 38.,
            roughness: 0.35,
            ..Default::default()
        };
        let field = settings.generate().unwrap();
        for (index, expected) in [
            (11804, 27.656818),
            (2325, 9.800434),
            (10517, 6.1802855),
            (12916, 3.1585717),
            (14193, 0.996292),
        ] {
            assert!((field.values[index] - expected).abs() < 0.00001);
        }
        let mut modern = settings;
        modern.algorithm_version = 2;
        assert_ne!(field.values, modern.generate().unwrap().values);
        let recipes = modern.comparison();
        assert_eq!(recipes[0].algorithm_version, 1);
        assert_eq!(recipes[1].algorithm_version, 1);
        assert_eq!(recipes[2].algorithm_version, 2);
    }
    #[test]
    fn diamond_grid_is_seeded_and_resampling_preserves_corners() {
        let a = diamond_square(32, 42, 0.5);
        assert_eq!(a, diamond_square(32, 42, 0.5));
        assert_ne!(a, diamond_square(32, 43, 0.5));
        assert_eq!(bilinear(&a, 33, 0., 0.), a[0] as f64);
        assert_eq!(bilinear(&a, 33, 1., 1.), a[33 * 33 - 1] as f64);
    }
    #[test]
    fn plain_canyon_and_seabed_are_bounded_distinct_heightfields() {
        let mut fields = Vec::new();
        for preset in [
            TerrainPreset::Plain,
            TerrainPreset::Canyon,
            TerrainPreset::Underwater,
        ] {
            let recipe = TerrainSettings {
                preset,
                resolution: 32,
                ..Default::default()
            };
            let field = recipe.generate().unwrap();
            assert!(field
                .values
                .iter()
                .all(|v| v.is_finite() && *v >= 0. && *v <= recipe.height as f32));
            assert!(!fields.contains(&field.values));
            fields.push(field.values);
        }
    }
}
