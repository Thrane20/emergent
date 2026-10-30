//! Bounded, deterministic particle simulation. Lengths are metres; time is seconds.
use crate::{EntityId, Mat4, Project, TextureImage, Vec3};
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, SlotMap};
use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

pub const PARTICLE_MAX_LIVE: usize = 100_000;
pub const PARTICLE_STEP: f32 = 1. / 60.;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticleError(pub String);
impl fmt::Display for ParticleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ParticleError {}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ParticleShape {
    Point,
    Cone,
    Box,
    Sphere,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ParticleBlend {
    Alpha,
    Additive,
    Screen,
    Multiply,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ParticleEmitter {
    pub name: String,
    pub seed: u32,
    pub max_particles: u32,
    pub rate: f32,
    pub burst: u32,
    pub duration: f32,
    pub looping: bool,
    pub local_space: bool,
    pub shape: ParticleShape,
    pub position: [f32; 3],
    /// Box half extents, sphere radii, or cone base radii (X/Z) and height (Y), in metres.
    pub extent: [f32; 3],
    pub speed: f32,
    pub spread: f32,
    pub lifetime: [f32; 2],
    pub gravity: [f32; 3],
    pub wind: [f32; 3],
    pub drag: f32,
    pub size: [f32; 2],
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
    pub rotation_speed: f32,
    pub blend: ParticleBlend,
    pub soft_distance: f32,
    pub texture: Option<String>,
    pub columns: u32,
    pub rows: u32,
    pub frames: u32,
    pub fps: f32,
    pub random_frame: bool,
    /// Play the flipbook forwards then backwards without holding endpoint frames.
    pub ping_pong: bool,
}
impl Default for ParticleEmitter {
    fn default() -> Self {
        Self {
            name: "Smoke".into(),
            seed: 1,
            max_particles: 1000,
            rate: 40.,
            burst: 0,
            duration: 5.,
            looping: true,
            local_space: false,
            shape: ParticleShape::Cone,
            position: [0.; 3],
            extent: [0.2; 3],
            speed: 1.,
            spread: 0.3,
            lifetime: [2., 4.],
            gravity: [0., 0.1, 0.],
            wind: [0.; 3],
            drag: 0.2,
            size: [0.25, 1.5],
            color_start: [0.7, 0.7, 0.7, 0.5],
            color_end: [0.3, 0.3, 0.3, 0.],
            rotation_speed: 0.3,
            blend: ParticleBlend::Alpha,
            soft_distance: 0.3,
            texture: None,
            columns: 1,
            rows: 1,
            frames: 1,
            fps: 12.,
            random_frame: false,
            ping_pong: false,
        }
    }
}
/// Maps animation phase to a cell in a validated flipbook, without duplicating endpoints.
#[cfg(any(feature = "graphics", test))]
pub(crate) fn sprite_frame_at(step: u32, frames: u32, ping_pong: bool) -> u32 {
    if frames <= 1 {
        return 0;
    }
    let cycle = if ping_pong { 2 * (frames - 1) } else { frames };
    let phase = step % cycle;
    if ping_pong && phase >= frames {
        cycle - phase
    } else {
        phase
    }
}

impl ParticleEmitter {
    pub fn validate(&self) -> Result<(), ParticleError> {
        let finite = |v: f32, min: f32, max: f32| v.is_finite() && v >= min && v <= max;
        let valid = self.name.chars().count() <= 64
            && !self.name.trim().is_empty()
            && !self.name.chars().any(char::is_control)
            && self.max_particles > 0
            && self.max_particles as usize <= PARTICLE_MAX_LIVE
            && self.burst <= self.max_particles
            && finite(self.rate, 0., 100_000.)
            && finite(self.duration, PARTICLE_STEP, 3600.)
            && self.position.iter().all(|&v| finite(v, -10000., 10000.))
            && self.extent.iter().all(|&v| finite(v, 0., 1000.))
            && finite(self.speed, 0., 1000.)
            && finite(self.spread, 0., std::f32::consts::PI)
            && self
                .lifetime
                .iter()
                .all(|&v| finite(v, PARTICLE_STEP, 120.))
            && self.lifetime[0] <= self.lifetime[1]
            && self
                .gravity
                .iter()
                .chain(&self.wind)
                .all(|&v| finite(v, -1000., 1000.))
            && finite(self.drag, 0., 100.)
            && self.size.iter().all(|&v| finite(v, 0., 1000.))
            && self
                .color_start
                .iter()
                .chain(&self.color_end)
                .all(|&v| finite(v, 0., 1.))
            && finite(self.rotation_speed, -100., 100.)
            && finite(self.soft_distance, 0., 10.)
            && self.columns > 0
            && self.rows > 0
            && self.columns <= 16
            && self.rows <= 16
            && self.frames > 0
            && self.frames <= self.columns * self.rows
            && finite(self.fps, 0., 120.);
        if !valid {
            return Err(ParticleError(
                "Invalid particle emitter: check bounds, colors, lifetime and flipbook layout."
                    .into(),
            ));
        }
        if let Some(p) = &self.texture {
            if p.is_empty()
                || p.len() > 512
                || p.starts_with('/')
                || p.contains('\\')
                || p.contains(':')
                || p.split('/').any(|s| s == ".." || s.is_empty() || s == ".")
            {
                return Err(ParticleError(
                    "Particle texture needs a project-relative asset path.".into(),
                ));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParticleDefinition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub emitters: Vec<ParticleEmitter>,
}
impl ParticleDefinition {
    pub fn validate(&self) -> Result<(), ParticleError> {
        if self.schema_version != 1
            || self.id.is_empty()
            || self.id.len() > 128
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            || self.name.trim().is_empty()
            || self.name.chars().count() > 64
            || self.name.chars().any(char::is_control)
            || self.emitters.is_empty()
            || self.emitters.len() > 16
            || self
                .emitters
                .iter()
                .map(|e| e.max_particles as u64)
                .sum::<u64>()
                > PARTICLE_MAX_LIVE as u64
        {
            return Err(ParticleError("Effect needs schema 1, a printable name, durable ID, 1–16 emitters and at most 100,000 total particles.".into()));
        }
        for e in &self.emitters {
            e.validate()?;
        }
        Ok(())
    }
    pub fn from_source(source: &str) -> Result<Self, ParticleError> {
        if source.len() > 128 * 1024 {
            return Err(ParticleError("Particle asset exceeds 128 KiB.".into()));
        }
        let d: Self = toml::from_str(source).map_err(|e| ParticleError(e.to_string()))?;
        d.validate()?;
        Ok(d)
    }
    pub fn load(project: &Project, path: &str) -> Result<Self, ParticleError> {
        use std::io::Read;
        let path = project
            .asset_path(path)
            .map_err(|e| ParticleError(e.to_string()))?;
        let mut source = String::new();
        std::fs::File::open(path)
            .and_then(|f| f.take(128 * 1024 + 1).read_to_string(&mut source))
            .map_err(|e| ParticleError(e.to_string()))?;
        Self::from_source(&source)
    }
    pub fn resolve(self, project: &Project) -> Result<ParticleEffect, ParticleError> {
        self.validate()?;
        let mut textures = Vec::with_capacity(self.emitters.len());
        let mut images: HashMap<String, TextureImage> = HashMap::new();
        let mut bytes = 0usize;
        for e in &self.emitters {
            textures.push(match &e.texture {
                Some(p) => {
                    let path = project
                        .asset_path(p)
                        .map_err(|e| ParticleError(e.to_string()))?;
                    let image = if let Some(image) = images.get(p) {
                        image.clone()
                    } else {
                        let image =
                            TextureImage::load(path).map_err(|e| ParticleError(e.to_string()))?;
                        bytes += image.rgba().len();
                        if bytes > 64 * 1024 * 1024 {
                            return Err(ParticleError(
                                "Effect sprite images exceed 64 MiB decoded.".into(),
                            ));
                        }
                        images.insert(p.clone(), image.clone());
                        image
                    };
                    if image.width() % e.columns != 0 || image.height() % e.rows != 0 {
                        return Err(ParticleError(
                            "Sprite dimensions must divide evenly into the flipbook grid.".into(),
                        ));
                    }
                    Some(image)
                }
                None => None,
            });
        }
        Ok(ParticleEffect {
            definition: Arc::new(self),
            textures: Arc::new(textures),
        })
    }
    pub fn procedural(self) -> Result<ParticleEffect, ParticleError> {
        self.validate()?;
        if self.emitters.iter().any(|e| e.texture.is_some()) {
            return Err(ParticleError(
                "Resolve textured effects against a project.".into(),
            ));
        }
        let n = self.emitters.len();
        Ok(ParticleEffect {
            definition: Arc::new(self),
            textures: Arc::new(vec![None; n]),
        })
    }
}
#[derive(Clone, Debug, crate::Component)]
pub struct ParticleEffect {
    pub(crate) definition: Arc<ParticleDefinition>,
    pub(crate) textures: Arc<Vec<Option<TextureImage>>>,
}
impl ParticleEffect {
    pub fn definition(&self) -> &ParticleDefinition {
        &self.definition
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f32,
    pub lifetime: f32,
    pub rotation: f32,
    pub frame: f32,
    pub scale: f32,
}
#[derive(Debug)]
struct EmitterState {
    particles: Vec<Particle>,
    rng: u32,
    credit: f32,
    elapsed: f32,
    started: bool,
}
#[derive(Debug)]
pub(crate) struct ParticleInstance {
    pub effect: ParticleEffect,
    pub model: Mat4,
    states: Vec<EmitterState>,
}
new_key_type! {struct ParticleKey;}
static NEXT_PARTICLE_STORE: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParticleSystemId {
    owner: u64,
    key: ParticleKey,
}
#[derive(Debug)]
pub struct ParticleStore {
    owner: u64,
    systems: SlotMap<ParticleKey, ParticleInstance>,
    reserved: usize,
    pub(crate) bindings: HashMap<EntityId, ParticleSystemId>,
}
impl Default for ParticleStore {
    fn default() -> Self {
        Self {
            owner: NEXT_PARTICLE_STORE.fetch_add(1, Ordering::Relaxed),
            systems: SlotMap::with_key(),
            reserved: 0,
            bindings: HashMap::new(),
        }
    }
}
fn random(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
    (*rng >> 8) as f32 / 16777216.
}
impl ParticleStore {
    pub fn insert(
        &mut self,
        effect: ParticleEffect,
        model: Mat4,
    ) -> Result<ParticleSystemId, ParticleError> {
        if !model.is_finite() {
            return Err(ParticleError("Particle transform must be finite.".into()));
        }
        let capacity = effect
            .definition
            .emitters
            .iter()
            .map(|e| e.max_particles as usize)
            .sum::<usize>();
        if self.reserved + capacity > PARTICLE_MAX_LIVE {
            return Err(ParticleError(
                "Particle world capacity exceeds 100,000.".into(),
            ));
        }
        self.reserved += capacity;
        let states = effect
            .definition
            .emitters
            .iter()
            .map(|e| EmitterState {
                particles: Vec::with_capacity(e.max_particles as usize),
                rng: e.seed,
                credit: 0.,
                elapsed: 0.,
                started: false,
            })
            .collect();
        let key = self.systems.insert(ParticleInstance {
            effect,
            model,
            states,
        });
        Ok(ParticleSystemId {
            owner: self.owner,
            key,
        })
    }
    pub fn remove(&mut self, id: ParticleSystemId) -> bool {
        if id.owner != self.owner {
            return false;
        }
        if let Some(system) = self.systems.remove(id.key) {
            self.reserved -= system
                .effect
                .definition
                .emitters
                .iter()
                .map(|e| e.max_particles as usize)
                .sum::<usize>();
            true
        } else {
            false
        }
    }
    pub fn contains(&self, id: ParticleSystemId) -> bool {
        id.owner == self.owner && self.systems.contains_key(id.key)
    }
    pub fn set_transform(
        &mut self,
        id: ParticleSystemId,
        model: Mat4,
    ) -> Result<(), ParticleError> {
        if !model.is_finite() {
            return Err(ParticleError("Particle transform must be finite.".into()));
        }
        let s = self.get_mut(id)?;
        s.model = model;
        Ok(())
    }
    fn get_mut(&mut self, id: ParticleSystemId) -> Result<&mut ParticleInstance, ParticleError> {
        if id.owner != self.owner {
            return Err(ParticleError(
                "Particle handle belongs to another store.".into(),
            ));
        }
        self.systems
            .get_mut(id.key)
            .ok_or_else(|| ParticleError("Stale particle system handle.".into()))
    }
    pub fn restart(&mut self, id: ParticleSystemId) -> Result<(), ParticleError> {
        let s = self.get_mut(id)?;
        for (state, e) in s.states.iter_mut().zip(&s.effect.definition.emitters) {
            state.particles.clear();
            state.rng = e.seed;
            state.credit = 0.;
            state.elapsed = 0.;
            state.started = false;
        }
        Ok(())
    }
    pub fn live_count(&self) -> usize {
        self.systems
            .values()
            .flat_map(|s| &s.states)
            .map(|s| s.particles.len())
            .sum()
    }
    /// Advance one bounded step. Call from a fixed simulation loop; never wall-clock rendering.
    pub fn step(&mut self, dt: f32) -> Result<(), ParticleError> {
        if !dt.is_finite() || dt <= 0. || dt > 0.1 {
            return Err(ParticleError(
                "Particle step must be in (0, 0.1] seconds.".into(),
            ));
        }
        for system in self.systems.values_mut() {
            for (state, e) in system
                .states
                .iter_mut()
                .zip(&system.effect.definition.emitters)
            {
                state.particles.retain_mut(|p| {
                    p.age += dt;
                    if p.age >= p.lifetime {
                        return false;
                    }
                    p.velocity += (Vec3::from_array(e.gravity) + Vec3::from_array(e.wind)) * dt;
                    p.velocity *= (-e.drag * dt).exp();
                    p.position += p.velocity * dt;
                    p.rotation += e.rotation_speed * dt;
                    true
                });
                if e.looping && state.elapsed >= e.duration {
                    state.elapsed %= e.duration;
                    state.started = false;
                }
                let active = e.looping || state.elapsed < e.duration;
                let active_dt = if e.looping {
                    dt
                } else {
                    dt.min((e.duration - state.elapsed).max(0.))
                };
                let mut spawn = 0u32;
                if active {
                    state.credit += e.rate * active_dt;
                    spawn = state.credit.floor() as u32;
                    state.credit -= spawn as f32;
                    if !state.started {
                        spawn = spawn.saturating_add(e.burst);
                        state.started = true;
                    }
                }
                state.elapsed = if e.looping {
                    state.elapsed + dt
                } else {
                    (state.elapsed + dt).min(e.duration)
                };
                // Emission credit is consumed even at capacity, so freeing a slot cannot create a backlog burst.
                for _ in 0..spawn.min(e.max_particles - state.particles.len() as u32) {
                    let r = &mut state.rng;
                    let a = random(r) * std::f32::consts::TAU;
                    let z = random(r) * 2. - 1.;
                    let radial = (1. - z * z).sqrt();
                    let unit = Vec3::new(radial * a.cos(), z, radial * a.sin());
                    let offset = match e.shape {
                        ParticleShape::Point => Vec3::ZERO,
                        ParticleShape::Box => Vec3::new(
                            random(r) * 2. - 1.,
                            random(r) * 2. - 1.,
                            random(r) * 2. - 1.,
                        ),
                        ParticleShape::Sphere => unit * random(r).cbrt(),
                        ParticleShape::Cone => {
                            let h = random(r).cbrt();
                            Vec3::new(radial * a.cos() * h, h, radial * a.sin() * h)
                        }
                    } * Vec3::from_array(e.extent);
                    let cos = 1. - random(r) * (1. - e.spread.cos());
                    let sin = (1. - cos * cos).max(0.).sqrt();
                    let direction = Vec3::new(sin * a.cos(), cos, sin * a.sin());
                    let mut position = Vec3::from_array(e.position) + offset;
                    let mut velocity = direction * e.speed;
                    if !e.local_space {
                        position = system.model.transform_point3(position);
                        velocity = system.model.transform_vector3(velocity);
                    }
                    let lifetime = e.lifetime[0] + random(r) * (e.lifetime[1] - e.lifetime[0]);
                    let rotation = random(r) * std::f32::consts::TAU;
                    let frame = if e.random_frame {
                        (random(r) * e.frames as f32).floor()
                    } else {
                        0.
                    };
                    state.particles.push(Particle {
                        position,
                        velocity,
                        age: 0.,
                        lifetime,
                        rotation,
                        frame,
                        scale: if e.local_space {
                            1.
                        } else {
                            system
                                .model
                                .x_axis
                                .truncate()
                                .length()
                                .max(system.model.y_axis.truncate().length())
                                .max(system.model.z_axis.truncate().length())
                        },
                    });
                }
            }
        }
        Ok(())
    }
    pub fn visit(
        &self,
        mut visitor: impl FnMut(&ParticleEmitter, Option<&TextureImage>, Mat4, &[Particle]),
    ) {
        for s in self.systems.values() {
            for (i, (state, e)) in s
                .states
                .iter()
                .zip(&s.effect.definition.emitters)
                .enumerate()
            {
                visitor(e, s.effect.textures[i].as_ref(), s.model, &state.particles);
            }
        }
    }
    pub(crate) fn bound_effect(&self, id: ParticleSystemId) -> Option<&ParticleEffect> {
        if id.owner != self.owner {
            return None;
        }
        self.systems.get(id.key).map(|s| &s.effect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn effect() -> ParticleEffect {
        ParticleDefinition {
            schema_version: 1,
            id: "test".into(),
            name: "Test".into(),
            emitters: vec![ParticleEmitter {
                max_particles: 8,
                rate: 60.,
                lifetime: [0.1; 2],
                ..Default::default()
            }],
        }
        .procedural()
        .unwrap()
    }
    #[test]
    fn flipbook_ping_pong_reflects_without_endpoint_holds() {
        let forward: Vec<_> = (0..9).map(|step| sprite_frame_at(step, 4, false)).collect();
        let reflected: Vec<_> = (0..13).map(|step| sprite_frame_at(step, 4, true)).collect();
        assert_eq!(forward, vec![0, 1, 2, 3, 0, 1, 2, 3, 0]);
        assert_eq!(reflected, vec![0, 1, 2, 3, 2, 1, 0, 1, 2, 3, 2, 1, 0]);
        for frames in 1..=256 {
            for step in 0..(frames * 4) {
                assert!(sprite_frame_at(step, frames, true) < frames);
            }
        }
        assert_eq!(sprite_frame_at(u32::MAX, 1, true), 0);
        assert_eq!(
            (0..5)
                .map(|s| sprite_frame_at(s, 2, true))
                .collect::<Vec<_>>(),
            vec![0, 1, 0, 1, 0]
        );
        let old: ParticleEmitter = toml::from_str("frames = 4\ncolumns = 2\nrows = 2").unwrap();
        assert!(!old.ping_pong);
        let enabled = ParticleEmitter {
            ping_pong: true,
            ..old
        };
        let decoded: ParticleEmitter = toml::from_str(&toml::to_string(&enabled).unwrap()).unwrap();
        assert!(decoded.ping_pong);
    }

    #[test]
    fn blend_modes_round_trip_and_reject_unknown_modes() {
        for blend in [
            ParticleBlend::Alpha,
            ParticleBlend::Additive,
            ParticleBlend::Screen,
            ParticleBlend::Multiply,
        ] {
            let emitter = ParticleEmitter {
                blend,
                ..Default::default()
            };
            let source = toml::to_string(&emitter).unwrap();
            assert_eq!(
                toml::from_str::<ParticleEmitter>(&source).unwrap().blend,
                blend
            );
        }
        assert!(toml::from_str::<ParticleEmitter>("blend='invalid'").is_err());
    }
    #[test]
    fn world_lifecycle_follows_entity_and_component_deletion() {
        let mut world = crate::World::new();
        let id = world.spawn("emitter");
        world.insert_component(id, effect()).unwrap();
        world.propagate_transforms();
        world.step_particles(PARTICLE_STEP).unwrap();
        assert!(world.particles().live_count() > 0);
        world.remove_subtree(id).unwrap();
        assert_eq!(world.particles().live_count(), 0);
        let replacement = world.spawn("replacement");
        assert_ne!(id, replacement);
        world.insert_component(replacement, effect()).unwrap();
        world.step_particles(PARTICLE_STEP).unwrap();
        world
            .remove_component::<ParticleEffect>(replacement)
            .unwrap();
        world.step_particles(PARTICLE_STEP).unwrap();
        assert_eq!(world.particles().live_count(), 0);
    }
    #[test]
    fn local_particles_follow_transform_and_loop_bursts_repeat() {
        let mut d = effect().definition().clone();
        d.emitters[0].local_space = true;
        d.emitters[0].duration = PARTICLE_STEP * 2.;
        d.emitters[0].rate = 0.;
        d.emitters[0].burst = 1;
        let mut store = ParticleStore::default();
        let id = store
            .insert(d.procedural().unwrap(), Mat4::IDENTITY)
            .unwrap();
        store.step(PARTICLE_STEP).unwrap();
        store
            .set_transform(id, Mat4::from_translation(Vec3::X * 5.))
            .unwrap();
        store.visit(|_, _, model, particles| {
            assert!(model.transform_point3(particles[0].position).x > 4.)
        });
        store.step(PARTICLE_STEP).unwrap();
        store.step(PARTICLE_STEP).unwrap();
        assert_eq!(store.live_count(), 2);
    }
    #[test]
    fn deterministic_capped_and_restart() {
        let mut a = ParticleStore::default();
        let id = a.insert(effect(), Mat4::IDENTITY).unwrap();
        let mut b = ParticleStore::default();
        let other = b.insert(effect(), Mat4::IDENTITY).unwrap();
        for _ in 0..600 {
            a.step(PARTICLE_STEP).unwrap();
            b.step(PARTICLE_STEP).unwrap();
            assert!(a.live_count() <= 8);
        }
        let snapshot = |s: &ParticleStore| {
            let mut v = Vec::new();
            s.visit(|_, _, _, p| v.extend(p.iter().map(|p| (p.position, p.age, p.rotation))));
            v
        };
        assert_eq!(snapshot(&a), snapshot(&b));
        a.restart(id).unwrap();
        b.restart(other).unwrap();
        assert_eq!(a.live_count(), 0);
        a.step(PARTICLE_STEP).unwrap();
        b.step(PARTICLE_STEP).unwrap();
        assert_eq!(snapshot(&a), snapshot(&b));
    }
    #[test]
    fn stale_and_foreign_handles() {
        let mut a = ParticleStore::default();
        let id = a.insert(effect(), Mat4::IDENTITY).unwrap();
        let mut b = ParticleStore::default();
        let _ = b.insert(effect(), Mat4::IDENTITY).unwrap();
        assert!(!b.remove(id));
        assert!(a.remove(id));
        let replacement = a.insert(effect(), Mat4::IDENTITY).unwrap();
        assert_ne!(id, replacement);
        assert!(a.restart(id).is_err());
    }
    #[test]
    fn invalid_and_roundtrip() {
        let d = effect().definition().clone();
        assert_eq!(
            ParticleDefinition::from_source(&toml::to_string(&d).unwrap()).unwrap(),
            d
        );
        let mut bad = d;
        bad.emitters[0].rate = f32::NAN;
        assert!(bad.validate().is_err());
        bad.emitters[0].rate = 1.;
        bad.emitters[0].frames = 2;
        assert!(bad.validate().is_err());
    }
    #[test]
    fn one_shot_and_world_space() {
        let mut d = effect().definition().clone();
        d.emitters[0].looping = false;
        d.emitters[0].rate = 0.;
        d.emitters[0].burst = 1;
        let mut s = ParticleStore::default();
        let id = s
            .insert(
                d.procedural().unwrap(),
                Mat4::from_translation(Vec3::X * 5.),
            )
            .unwrap();
        s.step(PARTICLE_STEP).unwrap();
        s.set_transform(id, Mat4::IDENTITY).unwrap();
        s.visit(|_, _, _, p| assert!(p[0].position.x > 4.));
        for _ in 0..60 {
            s.step(PARTICLE_STEP).unwrap();
        }
        assert_eq!(s.live_count(), 0);
    }
}
