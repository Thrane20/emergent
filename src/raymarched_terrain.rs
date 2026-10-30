//! Experimental bounded implicit terrain; negative distance is solid.
use crate::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u32)]
pub enum RaymarchedBiome {
    #[default]
    Blend,
    Plain,
    Hilly,
    Mountainous,
    Island,
    Rivers,
    Canyon,
    Underwater,
    CaveCavern,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, bevy_ecs::component::Component)]
#[serde(default, deny_unknown_fields)]
pub struct RaymarchedTerrain {
    #[serde(default = "legacy_version")]
    pub version: u32,
    pub seed: u32,
    /// Region width in metres. V2 features retain their world-space wavelength.
    pub size: f32,
    pub biome_scale: f32,
    pub relief: f32,
    pub overhang: f32,
    pub cave_radius: f32,
    #[serde(default)]
    pub biome: RaymarchedBiome,
}
fn legacy_version() -> u32 {
    1
}
impl Default for RaymarchedTerrain {
    fn default() -> Self {
        Self {
            version: 2,
            seed: 42,
            size: 1024.,
            biome_scale: 180.,
            relief: 120.,
            overhang: 6.,
            cave_radius: 8.,
            biome: RaymarchedBiome::Hilly,
        }
    }
}
impl RaymarchedTerrain {
    pub fn validate(&self) -> Result<(), &'static str> {
        let extent = if self.version == 1 {
            8. ..=64.
        } else {
            64. ..=4096.
        };
        if !matches!(self.version, 1 | 2) || !self.size.is_finite() || !extent.contains(&self.size)
        {
            return Err("Ray-marched terrain requires version 1 (8–64m) or version 2 (64–4096m).");
        }
        for (value, min, max) in [
            (self.biome_scale, 80., 512.),
            (self.relief, 8., 160.),
            (self.overhang, 0., 24.),
            (self.cave_radius, 0., 16.),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(
                    "Biome scale, relief, overhang or cave radius is outside its finite bounds.",
                );
            }
        }
        Ok(())
    }
    pub(crate) fn phase(&self) -> f32 {
        (self.seed % 101) as f32 / 100.
    }
    pub fn bounds(&self) -> (Vec3, Vec3) {
        if self.version == 1 {
            (
                Vec3::new(-0.5, -0.0625, -0.5) * self.size,
                Vec3::new(0.5, 0.1875, 0.5) * self.size,
            )
        } else {
            (
                Vec3::new(
                    -self.size * 0.5,
                    -self.relief.max(64.) * 0.5,
                    -self.size * 0.5,
                ),
                Vec3::new(
                    self.size * 0.5,
                    12. + self.relief + self.overhang,
                    self.size * 0.5,
                ),
            )
        }
    }
    pub(crate) fn density_bound(&self) -> f32 {
        let gradient = (42.427 + 9.546 * self.relief) / self.biome_scale;
        let extra = match self.biome {
            RaymarchedBiome::Island => (self.relief + 24.) * 4.546 / self.size,
            RaymarchedBiome::Rivers => self.relief * 3.83 / self.biome_scale,
            _ => 0.,
        };
        (1. + gradient * gradient).sqrt()
            + extra
            + self.overhang * (5.304 / self.biome_scale + 5.197 / (self.biome_scale * 0.08))
    }
    fn biome_distance(&self, p: Vec3) -> f32 {
        let s = self.biome_scale;
        let n = |x: f32, z: f32, frequency: f32, salt: u32| {
            value_noise(
                Vec3::new(x * frequency, 0., z * frequency),
                self.seed.wrapping_add(salt),
            )
        };
        let mask = if matches!(
            self.biome,
            RaymarchedBiome::Blend | RaymarchedBiome::Island | RaymarchedBiome::Rivers
        ) {
            smooth((n(p.x, p.z, 1. / (s * 2.), 11) - 0.35) / 0.3)
        } else {
            1.
        };
        let ridge = 1. - (2. * n(p.x, p.z, 1. / s, 29) - 1.).abs();
        let rolling = 4. * n(p.x, p.z, 1. / (s * 0.2), 53);
        let mut height = 8.
            + 4. * n(p.x, p.z, 1. / (s * 0.2), 53)
            + self.relief * mask * (0.15 + 0.7 * ridge + 0.15 * n(p.x, p.z, 1. / (s * 0.25), 71));
        let channel = p.x - s * 0.3 * (p.z / (s * 0.8) + self.phase() * 6.2831855).sin();
        match self.biome {
            RaymarchedBiome::CaveCavern => height = height.max(self.relief * 0.85 + rolling),
            RaymarchedBiome::Plain => height = 8. + rolling,
            RaymarchedBiome::Hilly => {
                height = 8. + rolling + self.relief * 0.25 * n(p.x, p.z, 1. / (s * 0.5), 31)
            }
            RaymarchedBiome::Underwater => {
                height =
                    -self.relief * 0.25 + rolling + self.relief * 0.18 * n(p.x, p.z, 1. / s, 31)
            }
            RaymarchedBiome::Island => {
                let coast = 1. - smooth((p.x.hypot(p.z) / self.size - 0.15) / 0.33);
                height = (height + 12.) * coast - 12.;
            }
            RaymarchedBiome::Rivers => {
                height -= self.relief * 0.5 * (-(channel / (s * 0.12)).powi(2)).exp()
            }
            RaymarchedBiome::Canyon => {
                height = 8. + rolling + self.relief * (0.6 + 0.15 * n(p.x, p.z, 1. / s, 31))
                    - self.relief * 0.8 * (1. - smooth((channel.abs() / s - 0.06) / 0.19))
            }
            _ => {}
        }
        let rock = (value_noise(p / (s * 0.08), self.seed.wrapping_add(97)) * 2. - 1.)
            * self.overhang
            * mask;
        let mut field = (p.y - height - rock) / self.density_bound();
        if self.biome == RaymarchedBiome::CaveCavern {
            field = field.max(
                -((p - Vec3::new(0., self.relief * 0.25, 0.)).length()
                    - self.cave_radius.mul_add(3., 0.).max(24.)),
            );
        }
        if self.cave_radius > 0. {
            let phase = self.phase() * 6.2831855;
            let a = Vec3::new(
                p.y - (self.relief * 0.28 + 8. * (p.x / 50. + phase).sin()),
                p.z - s * 0.3 * (p.x / (s * 0.5) + phase).sin(),
                0.,
            )
            .length()
                - self.cave_radius;
            let b = Vec3::new(
                p.y - (self.relief * 0.22 + 6. * (p.z / (s * 0.3) + phase).cos()),
                p.x - (-s * 0.2 + s * 0.15 * (p.z / (s * 0.4) + phase).sin()),
                0.,
            )
            .length()
                - self.cave_radius * 0.8;
            field = field.max(-(a / 1.18).min(b / 1.24));
        }
        let (lower, upper) = self.bounds();
        let q = (p - (lower + upper) * 0.5).abs() - (upper - lower) * 0.5;
        field.max(q.max(Vec3::ZERO).length() + q.max_element().min(0.))
    }
    /// Conservative distance bound, shared conceptually with the GPU field.
    pub fn distance(&self, point: Vec3) -> Result<f32, &'static str> {
        self.validate()?;
        if !point.is_finite() || point.abs().max_element() > 1_000_000. {
            return Err("Field point must be finite and within one million metres.");
        }
        if self.version == 2 {
            return Ok(self.biome_distance(point));
        }
        let scale = self.size / 32.;
        let p = point / scale;
        let q = (p - Vec3::new(0., -1., 0.)).abs() - Vec3::new(16., 1., 16.);
        let ground = q.max(Vec3::ZERO).length() + q.max_element().min(0.);
        let m = p - Vec3::new(-5., 0., self.phase() * 2. - 1.);
        let mountain = m.length() - 6.;
        let tunnel = (m.x * m.x + (m.y - 1.).powi(2)).sqrt() - 2.;
        let carved = ground.min(mountain).max(-tunnel);
        let a = p - Vec3::new(7., 0., 0.);
        let arch = Vec3::new(a.truncate().length() - 4., a.z, 0.).length() - 1.;
        Ok(carved.min(arch) * scale)
    }
    /// Bounded reference query, not a physics collider. Distance is measured from origin.
    pub fn trace(&self, origin: Vec3, direction: Vec3) -> Result<Option<f32>, &'static str> {
        self.validate()?;
        if !origin.is_finite() || !direction.is_finite() || direction.length_squared() < 0.000001 {
            return Err("Trace requires a finite origin and nonzero direction.");
        }
        let direction = direction
            .try_normalize()
            .ok_or("Trace direction exceeds finite bounds.")?;
        let (lower, upper) = self.bounds();
        let mut t = 0_f32;
        let mut exit = f32::INFINITY;
        for axis in 0..3 {
            if direction[axis].abs() < 0.0000001 {
                if origin[axis] < lower[axis] || origin[axis] > upper[axis] {
                    return Ok(None);
                }
            } else {
                let a = (lower[axis] - origin[axis]) / direction[axis];
                let b = (upper[axis] - origin[axis]) / direction[axis];
                t = t.max(a.min(b));
                exit = exit.min(a.max(b));
            }
        }
        if t > exit {
            return Ok(None);
        }
        for _ in 0..if self.version == 1 { 256 } else { 1024 } {
            let d = self.distance(origin + direction * t)?.abs();
            if d < if self.version == 1 {
                0.002
            } else {
                0.003_f32.max(t * 0.00003)
            } {
                return Ok(Some(t));
            }
            t += d * 0.9;
            if t > exit {
                break;
            }
        }
        Ok(None)
    }
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn random_cell(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ (z as u32).wrapping_mul(2147483647)
        ^ seed.wrapping_mul(1274126177);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 65535) as f32 / 65535.
}
fn value_noise(p: Vec3, seed: u32) -> f32 {
    let cell = p.floor();
    let f = p - cell;
    let f = Vec3::new(smooth(f.x), smooth(f.y), smooth(f.z));
    let x = cell.x as i32;
    let y = cell.y as i32;
    let z = cell.z as i32;
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let plane = |dy| {
        mix(
            mix(
                random_cell(x, y + dy, z, seed),
                random_cell(x + 1, y + dy, z, seed),
                f.x,
            ),
            mix(
                random_cell(x, y + dy, z + 1, seed),
                random_cell(x + 1, y + dy, z + 1, seed),
                f.x,
            ),
            f.z,
        )
    };
    mix(plane(0), plane(1), f.y)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn biome_defaults_legacy_loading_and_world_scale_are_explicit() {
        let field = RaymarchedTerrain::default();
        assert_eq!(field.version, 2);
        assert_eq!(field.size, 1024.);
        let legacy: RaymarchedTerrain = toml::from_str("seed=42\nsize=32\n").unwrap();
        assert_eq!(legacy.version, 1);
        let bigger = RaymarchedTerrain {
            size: 4096.,
            ..field
        };
        for x in [-100., 0., 100.] {
            for z in [-100., 0., 100.] {
                let p = Vec3::new(x, 24., z);
                assert_eq!(field.distance(p), bigger.distance(p));
            }
        }
        let encoded = toml::to_string(&field).unwrap();
        assert_eq!(
            toml::from_str::<RaymarchedTerrain>(&encoded).unwrap(),
            field
        );
        assert!(RaymarchedTerrain {
            size: 8192.,
            ..field
        }
        .validate()
        .is_err());
    }
    #[test]
    fn selectable_biomes_are_distinct_and_legacy_mixed_recipes_stay_mixed() {
        let old:RaymarchedTerrain=toml::from_str("version=2\nseed=42\nsize=1024\nbiome_scale=180\nrelief=120\noverhang=6\ncave_radius=8\n").unwrap();
        assert_eq!(old.biome, RaymarchedBiome::Blend);
        let mut signatures = Vec::new();
        for biome in [
            RaymarchedBiome::Plain,
            RaymarchedBiome::Hilly,
            RaymarchedBiome::Mountainous,
            RaymarchedBiome::Island,
            RaymarchedBiome::Rivers,
            RaymarchedBiome::Canyon,
            RaymarchedBiome::Underwater,
            RaymarchedBiome::CaveCavern,
        ] {
            let field = RaymarchedTerrain { biome, ..old };
            let signature = [
                Vec3::new(0., 30., 0.),
                Vec3::new(160., 40., 120.),
                Vec3::new(400., 20., 0.),
            ]
            .map(|p| field.distance(p).unwrap().to_bits());
            assert!(
                !signatures.contains(&signature),
                "Biome {biome:?} duplicated another field"
            );
            signatures.push(signature);
            let e = 0.05;
            for x in (-200..200).step_by(67) {
                for z in (-200..200).step_by(71) {
                    let p = Vec3::new(x as f32, 40., z as f32);
                    let d = field.distance(p).unwrap();
                    let grad = Vec3::new(
                        field.distance(p + Vec3::X * e).unwrap() - d,
                        field.distance(p + Vec3::Y * e).unwrap() - d,
                        field.distance(p + Vec3::Z * e).unwrap() - d,
                    ) / e;
                    assert!(grad.length() < 1.04, "Biome {biome:?} gradient {grad:?}");
                }
            }
        }
        let cave = RaymarchedTerrain {
            biome: RaymarchedBiome::CaveCavern,
            cave_radius: 16.,
            ..old
        };
        assert!(cave.distance(Vec3::new(0., 30., 0.)).unwrap() > 0.);
        assert!(cave.distance(Vec3::new(0., 90., 0.)).unwrap() < 0.);
    }
    #[test]
    fn biome_distance_is_finite_and_respects_its_conservative_gradient_bound() {
        let field = RaymarchedTerrain::default();
        let e = 0.05;
        for x in (-200..200).step_by(19) {
            for y in (0..140).step_by(13) {
                for z in (-200..200).step_by(23) {
                    let p = Vec3::new(x as f32, y as f32, z as f32);
                    let d = field.distance(p).unwrap();
                    assert!(d.is_finite());
                    let gradient = Vec3::new(
                        field.distance(p + Vec3::X * e).unwrap() - d,
                        field.distance(p + Vec3::Y * e).unwrap() - d,
                        field.distance(p + Vec3::Z * e).unwrap() - d,
                    ) / e;
                    assert!(gradient.length() < 1.04, "gradient {gradient:?} at {p:?}");
                }
            }
        }
    }
    #[test]
    fn biome_caves_carve_solid_mountains_and_can_be_disabled() {
        let field = RaymarchedTerrain::default();
        let solid = RaymarchedTerrain {
            cave_radius: 0.,
            ..field
        };
        let phase = field.phase() * std::f32::consts::TAU;
        let mut carved = false;
        for x in (-480..480).step_by(8) {
            let x = x as f32;
            let center = Vec3::new(
                x,
                field.relief * 0.28 + 8. * (x / 50. + phase).sin(),
                field.biome_scale * 0.3 * (x / (field.biome_scale * 0.5) + phase).sin(),
            );
            if solid.distance(center).unwrap() < -0.5 {
                assert!(field.distance(center).unwrap() > 0.);
                carved = true;
                break;
            }
        }
        assert!(carved, "Expected a tunnel through solid mountain rock");
    }
    #[test]
    fn field_is_bounded_and_caves_and_arch_have_openings() {
        let f = RaymarchedTerrain {
            version: 1,
            size: 32.,
            ..Default::default()
        };
        assert!(f.distance(Vec3::new(0., -1., 10.)).unwrap() < 0.);
        assert!(f.distance(Vec3::new(-5., 1., 0.)).unwrap() > 0.);
        assert!(f.distance(Vec3::new(-8., 2., 0.)).unwrap() < 0.);
        assert!(f.distance(Vec3::new(7., 2., 0.)).unwrap() > 0.);
        assert!(f.distance(Vec3::new(7., 4., 0.)).unwrap() < 0.);
        assert!(f.distance(Vec3::splat(100.)).unwrap() > 0.);
        assert!(RaymarchedTerrain {
            size: f32::NAN,
            ..f
        }
        .validate()
        .is_err());
        assert!(f.distance(Vec3::splat(f32::INFINITY)).is_err());
    }
    #[test]
    fn traces_reach_ground_and_exit_from_inside_without_unbounded_work() {
        let f = RaymarchedTerrain {
            version: 1,
            size: 32.,
            ..Default::default()
        };
        let t = f.trace(Vec3::new(0., 10., 10.), -Vec3::Y).unwrap().unwrap();
        assert!((t - 10.).abs() < 0.01);
        let exit = f.trace(Vec3::new(0., -1., 10.), Vec3::Y).unwrap().unwrap();
        assert!((exit - 1.).abs() < 0.01);
        assert_eq!(f.trace(Vec3::splat(100.), Vec3::Y).unwrap(), None);
        assert!(f.trace(Vec3::ZERO, Vec3::ZERO).is_err());
        let other = RaymarchedTerrain { seed: 100, ..f };
        assert_ne!(
            f.distance(Vec3::new(-8., 3., 2.)),
            other.distance(Vec3::new(-8., 3., 2.))
        );
    }
}
