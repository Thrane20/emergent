//! Continuous-world terrain: a versioned world descriptor and seamless, headless chunk
//! preparation on one global sample lattice. See `docs/design/terrain-world.md`.
use crate::{
    Color, GeometryError, MaterialError, Mesh, MeshVertex, NoiseError, NoiseGenerator,
    NoiseSettings,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Cells per chunk side; a chunk holds one more sample than this per side.
pub const TERRAIN_CHUNK_CELLS: u32 = 128;
const SAMPLES: usize = TERRAIN_CHUNK_CELLS as usize + 1;
const HALO: usize = SAMPLES + 2;
const MAX_CHUNKS: u32 = 256;

/// Integer chunk address within one world. A lookup key, not runtime identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

/// Versioned, bounded world recipe. Distances are metres; the world is centred on the origin.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct TerrainWorld {
    pub world_version: u32,
    pub seed: u32,
    pub chunks_x: u32,
    pub chunks_z: u32,
    pub cell_size: f64,
    pub height: f64,
    /// Noise features per kilometre.
    pub frequency: f64,
    pub octaves: u32,
    pub roughness: f64,
    pub domain_warp: f64,
}
impl Default for TerrainWorld {
    fn default() -> Self {
        Self {
            world_version: 1,
            seed: 42,
            chunks_x: 16,
            chunks_z: 16,
            cell_size: 1.,
            height: 120.,
            frequency: 1.5,
            octaves: 5,
            roughness: 0.45,
            domain_warp: 0.25,
        }
    }
}

impl TerrainWorld {
    pub fn validate(&self) -> Result<(), NoiseError> {
        if self.world_version != 1
            || !(1..=MAX_CHUNKS).contains(&self.chunks_x)
            || !(1..=MAX_CHUNKS).contains(&self.chunks_z)
            || !(1..=8).contains(&self.octaves)
        {
            return Err(NoiseError(
                "Terrain world requires version 1, 1..256 chunks per axis and octaves 1..8.".into(),
            ));
        }
        for (value, min, max) in [
            (self.cell_size, 0.25, 16.),
            (self.height, 0., 4000.),
            (self.frequency, 0.01, 100.),
            (self.roughness, 0., 1.),
            (self.domain_warp, 0., 1.),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(NoiseError(
                    "Terrain world numeric setting is outside its finite bounds.".into(),
                ));
            }
        }
        Ok(())
    }

    /// Chunk x/z ranges are `-(count / 2) .. count - count / 2`.
    pub fn contains(&self, coord: ChunkCoord) -> bool {
        let inside = |c: i32, count: u32| {
            let low = -((count / 2) as i32);
            (low..low + count as i32).contains(&c)
        };
        inside(coord.x, self.chunks_x) && inside(coord.z, self.chunks_z)
    }

    /// Prepare one chunk. A pure function of the descriptor and coordinate: no live
    /// world, renderer or physics state is touched, so it is safe off the frame thread.
    pub fn prepare_chunk(&self, coord: ChunkCoord) -> Result<TerrainChunk, NoiseError> {
        self.validate()?;
        if !self.contains(coord) {
            return Err(NoiseError(
                "Terrain chunk coordinate is outside the world.".into(),
            ));
        }
        let noise = NoiseGenerator::for_sampling(NoiseSettings {
            seed: self.seed,
            frequency: 1.,
            octaves: self.octaves,
            persistence: self.roughness,
            ..NoiseSettings::default()
        })?;
        let warp = NoiseGenerator::for_sampling(NoiseSettings {
            seed: self.seed ^ 0x9e3779b9,
            octaves: 2,
            ..NoiseSettings::default()
        })?;
        let cells = TERRAIN_CHUNK_CELLS as i64;
        let origin = [coord.x as i64 * cells - 1, coord.z as i64 * cells - 1];
        let mut halo = Vec::with_capacity(HALO * HALO);
        for z in 0..HALO as i64 {
            for x in 0..HALO as i64 {
                halo.push(self.sample(&noise, &warp, [origin[0] + x, origin[1] + z])?);
            }
        }
        let mut heights = Vec::with_capacity(SAMPLES * SAMPLES);
        let mut normals = Vec::with_capacity(SAMPLES * SAMPLES);
        let span = 2. * self.cell_size as f32;
        for z in 1..=SAMPLES {
            for x in 1..=SAMPLES {
                let at = |x: usize, z: usize| halo[z * HALO + x];
                heights.push(at(x, z));
                let dx = (at(x + 1, z) - at(x - 1, z)) / span;
                let dz = (at(x, z + 1) - at(x, z - 1)) / span;
                normals.push(Vec3::new(-dx, 1., -dz).normalize());
            }
        }
        let (min_height, max_height) = heights
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &h| {
                (lo.min(h), hi.max(h))
            });
        Ok(TerrainChunk {
            coord,
            cell_size: self.cell_size,
            heights,
            normals,
            min_height,
            max_height,
        })
    }

    fn sample(
        &self,
        noise: &NoiseGenerator,
        warp: &NoiseGenerator,
        lattice: [i64; 2],
    ) -> Result<f32, NoiseError> {
        let scale = self.cell_size * self.frequency / 1000.;
        let (u, v) = (lattice[0] as f64 * scale, lattice[1] as f64 * scale);
        let (mut a, mut b) = (u, v);
        if self.domain_warp > 0. {
            a += (warp.sample_2d([u * 0.5 + 11., v * 0.5 + 7.])? as f64 - 0.5) * self.domain_warp;
            b += (warp.sample_2d([u * 0.5 + 37., v * 0.5 + 53.])? as f64 - 0.5) * self.domain_warp;
        }
        let base = noise.sample_2d([a, b])? as f64;
        Ok((base.powf(1.5) * self.height) as f32)
    }
}

/// Height-band material weights, the same rule as the Terrain Builder's height paint:
/// layer `c` gives way to `c + 1` around `transitions[c]` over a smoothstep `blend` metres
/// wide. Per sample, so chunks sharing border heights share border weights. Returns
/// RGBA bytes per height, each summing to 255 (rounding remainder on the largest).
pub fn height_band_weights(
    heights: &[f32],
    transitions: [f32; 3],
    blend: f32,
    count: usize,
) -> Result<Vec<u8>, MaterialError> {
    if !(1..=4).contains(&count)
        || !transitions
            .iter()
            .all(|n| n.is_finite() && (0.0..=1000.0).contains(n))
        || !transitions.windows(2).all(|p| p[0] < p[1])
        || !blend.is_finite()
        || !(0.0..=1000.0).contains(&blend)
    {
        return Err(MaterialError(
            "Use 1–4 layers, three increasing height transitions and a blend width 0..1000.".into(),
        ));
    }
    let mut result = Vec::with_capacity(heights.len() * 4);
    for &height in heights {
        let (mut values, mut remaining) = ([0f32; 4], 1f32);
        for (c, value) in values.iter_mut().enumerate().take(count) {
            let mut upper = 0.;
            if c < count - 1 {
                let distance = height - transitions[c];
                upper = if blend == 0. {
                    f32::from(distance >= 0.)
                } else {
                    (distance / blend + 0.5).clamp(0., 1.)
                };
                upper = upper * upper * (3. - 2. * upper);
            }
            *value = remaining * (1. - upper);
            remaining *= upper;
        }
        let mut bytes = values.map(|v| (v * 255.).round() as i32);
        let largest = (0..4).fold(0, |l, c| if values[c] > values[l] { c } else { l });
        bytes[largest] += 255 - bytes.iter().sum::<i32>();
        result.extend(bytes.map(|b| b as u8));
    }
    Ok(result)
}

/// Owned result of [`TerrainWorld::prepare_chunk`]: row-major heights and unit normals.
#[derive(Clone, Debug)]
pub struct TerrainChunk {
    coord: ChunkCoord,
    cell_size: f64,
    heights: Vec<f32>,
    normals: Vec<Vec3>,
    min_height: f32,
    max_height: f32,
}
impl TerrainChunk {
    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }
    /// Samples per side (`TERRAIN_CHUNK_CELLS + 1`).
    pub fn samples(&self) -> u32 {
        SAMPLES as u32
    }
    pub fn heights(&self) -> &[f32] {
        &self.heights
    }
    pub fn normals(&self) -> &[Vec3] {
        &self.normals
    }
    /// Lowest and highest sample; a body above `max` cannot touch this chunk.
    pub fn height_bounds(&self) -> (f32, f32) {
        (self.min_height, self.max_height)
    }
    /// World-space x/z of the chunk's first sample, in f64 so far chunks stay exact.
    pub fn origin(&self) -> [f64; 2] {
        let width = TERRAIN_CHUNK_CELLS as f64 * self.cell_size;
        [self.coord.x as f64 * width, self.coord.z as f64 * width]
    }
    /// Chunk-local smooth mesh; place it at [`Self::origin`].
    pub fn mesh(&self, color: Color) -> Result<Mesh, GeometryError> {
        let step = self.cell_size as f32;
        let mut vertices = Vec::with_capacity(SAMPLES * SAMPLES);
        let mut indices = Vec::with_capacity((SAMPLES - 1) * (SAMPLES - 1) * 6);
        for z in 0..SAMPLES {
            for x in 0..SAMPLES {
                let i = z * SAMPLES + x;
                vertices.push(MeshVertex::with_normal(
                    Vec3::new(x as f32 * step, self.heights[i], z as f32 * step),
                    self.normals[i],
                    color,
                )?);
                if x < SAMPLES - 1 && z < SAMPLES - 1 {
                    let (i, n) = (i as u32, SAMPLES as u32);
                    indices.extend([i, i + n, i + 1, i + 1, i + n, i + n + 1]);
                }
            }
        }
        Mesh::new(vertices, indices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAST: usize = SAMPLES - 1;
    fn chunk(world: &TerrainWorld, x: i32, z: i32) -> TerrainChunk {
        world.prepare_chunk(ChunkCoord { x, z }).unwrap()
    }
    fn at(c: &TerrainChunk, x: usize, z: usize) -> (f32, Vec3) {
        (c.heights[z * SAMPLES + x], c.normals[z * SAMPLES + x])
    }

    #[test]
    fn neighbouring_chunks_share_border_heights_and_normals_including_negative_coordinates() {
        let world = TerrainWorld::default();
        // The four chunks meeting at the world origin: (-1,-1), (0,-1), (-1,0), (0,0).
        let [a, b, c, d] = [(-1, -1), (0, -1), (-1, 0), (0, 0)].map(|(x, z)| chunk(&world, x, z));
        for i in 0..SAMPLES {
            assert_eq!(at(&a, LAST, i), at(&b, 0, i), "x edge, row {i}");
            assert_eq!(at(&c, LAST, i), at(&d, 0, i), "x edge, row {i}");
            assert_eq!(at(&a, i, LAST), at(&c, i, 0), "z edge, column {i}");
            assert_eq!(at(&b, i, LAST), at(&d, i, 0), "z edge, column {i}");
        }
        let corner = at(&d, 0, 0);
        assert_eq!(at(&a, LAST, LAST), corner);
        assert_eq!(at(&b, 0, LAST), corner);
        assert_eq!(at(&c, LAST, 0), corner);
        assert_ne!(a.heights, d.heights, "chunks are distinct terrain");
        assert!(
            d.heights.iter().any(|&h| h != d.heights[0]),
            "terrain is not flat"
        );
    }

    #[test]
    fn preparation_is_order_independent_finite_and_bounded() {
        let world = TerrainWorld::default();
        let first = chunk(&world, 3, -2);
        let _ = chunk(&world, -8, 7);
        let again = chunk(&world, 3, -2);
        assert_eq!(first.heights, again.heights);
        assert_eq!(first.normals, again.normals);
        let (low, high) = first.height_bounds();
        assert!(low <= high && low >= 0. && high <= world.height as f32);
        assert!(first.heights.iter().all(|h| h.is_finite()));
        assert!(first
            .normals
            .iter()
            .all(|n| n.is_finite() && n.y > 0. && (n.length() - 1.).abs() < 1e-5));
        let other_seed = TerrainWorld { seed: 7, ..world };
        assert_ne!(chunk(&other_seed, 3, -2).heights, first.heights);
    }

    #[test]
    fn descriptor_and_coordinates_are_validated() {
        let world = TerrainWorld::default();
        for bad in [
            TerrainWorld {
                world_version: 2,
                ..world
            },
            TerrainWorld {
                chunks_x: 0,
                ..world
            },
            TerrainWorld {
                chunks_z: 257,
                ..world
            },
            TerrainWorld {
                octaves: 9,
                ..world
            },
            TerrainWorld {
                cell_size: 0.1,
                ..world
            },
            TerrainWorld {
                height: f64::NAN,
                ..world
            },
            TerrainWorld {
                frequency: f64::INFINITY,
                ..world
            },
            TerrainWorld {
                domain_warp: 1.5,
                ..world
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
        // 16 chunks span -8..8; odd counts put the extra chunk on the positive side.
        assert!(world.contains(ChunkCoord { x: -8, z: 7 }));
        for outside in [(8, 0), (-9, 0), (0, 8), (0, -9)] {
            let coord = ChunkCoord {
                x: outside.0,
                z: outside.1,
            };
            assert!(!world.contains(coord));
            assert!(world.prepare_chunk(coord).is_err());
        }
        let odd = TerrainWorld {
            chunks_x: 3,
            chunks_z: 1,
            ..world
        };
        assert!(
            odd.contains(ChunkCoord { x: -1, z: 0 }) && odd.contains(ChunkCoord { x: 1, z: 0 })
        );
        assert!(
            !odd.contains(ChunkCoord { x: 2, z: 0 }) && !odd.contains(ChunkCoord { x: 0, z: 1 })
        );
    }

    #[test]
    fn descriptor_round_trips_and_rejects_unknown_fields() {
        let world = TerrainWorld {
            seed: 9,
            cell_size: 2.,
            ..TerrainWorld::default()
        };
        let source = toml::to_string(&world).unwrap();
        assert!(source.contains("cellSize = 2.0"));
        assert_eq!(toml::from_str::<TerrainWorld>(&source).unwrap(), world);
        assert!(toml::from_str::<TerrainWorld>("bogus = 1").is_err());
    }

    #[test]
    fn height_band_weights_match_the_builder_rule_and_chunk_borders() {
        // Below, at and above a 10m transition with a 4m blend, two layers.
        let w = height_band_weights(&[0., 10., 20.], [10., 20., 30.], 4., 2).unwrap();
        // A 50/50 tie rounds both to 128; the remainder comes off the first largest, as in the IDE.
        assert_eq!(w, [255, 0, 0, 0, 127, 128, 0, 0, 0, 255, 0, 0]);
        let hard = height_band_weights(&[9.9, 10.], [10., 20., 30.], 0., 3).unwrap();
        assert_eq!(hard, [255, 0, 0, 0, 0, 255, 0, 0]);
        for bad in [
            ([10., 10., 30.], 4., 2),
            ([10., 20., 30.], -1., 2),
            ([10., 20., 30.], 4., 5),
        ] {
            assert!(height_band_weights(&[1.], bad.0, bad.1, bad.2).is_err());
        }
        let world = TerrainWorld::default();
        let (a, b) = (chunk(&world, -1, 0), chunk(&world, 0, 0));
        let bands = [30., 50., 70.];
        let (wa, wb) = (
            height_band_weights(a.heights(), bands, 8., 4).unwrap(),
            height_band_weights(b.heights(), bands, 8., 4).unwrap(),
        );
        assert!(wa
            .as_chunks::<4>()
            .0
            .iter()
            .all(|t| t.iter().map(|&n| n as u32).sum::<u32>() == 255));
        for z in 0..SAMPLES {
            let (l, r) = ((z * SAMPLES + LAST) * 4, z * SAMPLES * 4);
            assert_eq!(wa[l..l + 4], wb[r..r + 4]);
        }
    }

    #[test]
    fn terrain_blend_validates_layers_weights_and_offset() {
        use crate::{Material, MaterialDefinition};
        let layer = || MaterialDefinition::default().constant().unwrap();
        let ok = |layers: usize, weights: Vec<u8>, offset: [f64; 2]| {
            Material::terrain_blend(vec![layer(); layers], 2, weights, offset).is_ok()
        };
        let even = [255u8, 0, 0, 0].repeat(4);
        assert!(ok(1, even.clone(), [-256., 512.]));
        assert!(!ok(0, even.clone(), [0.; 2]), "no layers");
        assert!(!ok(5, even.clone(), [0.; 2]), "too many layers");
        assert!(!ok(1, [255u8, 0, 0, 0].repeat(3), [0.; 2]), "wrong length");
        assert!(
            !ok(1, [254u8, 0, 0, 0].repeat(4), [0.; 2]),
            "not normalized"
        );
        assert!(
            !ok(1, [0u8, 255, 0, 0].repeat(4), [0.; 2]),
            "weight on a missing layer"
        );
        assert!(!ok(1, even, [f64::NAN, 0.]), "non-finite offset");
    }

    #[test]
    fn chunk_meshes_meet_in_world_space() {
        let world = TerrainWorld {
            cell_size: 2.,
            ..TerrainWorld::default()
        };
        let (left, right) = (chunk(&world, -1, 0), chunk(&world, 0, 0));
        let color = Color::new(1., 1., 1., 1.).unwrap();
        let (ml, mr) = (left.mesh(color).unwrap(), right.mesh(color).unwrap());
        assert_eq!(ml.vertices().len(), SAMPLES * SAMPLES);
        assert_eq!(ml.indices().len(), LAST * LAST * 6);
        assert_eq!(left.origin(), [-256., 0.]);
        assert_eq!(right.origin(), [0., 0.]);
        let world_pos = |c: &TerrainChunk, v: &MeshVertex| {
            let o = c.origin();
            [
                o[0] + v.position().x as f64,
                v.position().y as f64,
                o[1] + v.position().z as f64,
            ]
        };
        for z in 0..SAMPLES {
            let l = &ml.vertices()[z * SAMPLES + LAST];
            let r = &mr.vertices()[z * SAMPLES];
            assert_eq!(world_pos(&left, l), world_pos(&right, r));
            assert_eq!(l.normal(), r.normal());
        }
    }
}
