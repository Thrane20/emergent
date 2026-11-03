//! Spatial audio core: clips, playing sounds, a listener and a device-independent mixer.
//!
//! [`AudioWorld`] lives on the engine thread. It owns clips and sounds (SlotMap keys),
//! computes gain, pan and Doppler from fixed-tick transforms, and sends compact
//! commands to the [`Mixer`], which runs on the audio thread (or is driven directly in
//! headless runs and tests). The mixer never locks or allocates: commands travel over a
//! bounded preallocated channel, and finished clips are handed back so their memory is
//! released on the engine thread.
//!
//! Audio is output only. Parameters are derived from fixed-tick transforms, so they are
//! deterministic for a given simulation, but mixed samples are not part of any replay
//! contract.

mod asset;
mod mixer;
#[cfg(feature = "audio")]
pub mod output;
mod stream;

use mixer::{Command, Finished, Source, VoiceParams};
pub use mixer::{Mixer, ReverbSettings};

pub(crate) use asset::sync_scene;
pub use asset::{
    AudioClipAsset, AudioEmitter, AudioListener, StreamScan, AUDIO_MAX_FILE_BYTES, SCAN_BLOCK,
};

use crate::{ActiveCamera, EntityId, Vec3, World};
use asset::{ClipLoad, Loaded};
use slotmap::{new_key_type, SlotMap};
use std::{
    collections::HashMap,
    fmt,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender, TrySendError},
        Arc,
    },
    time::Duration,
};

new_key_type! {
    /// A loaded clip in one [`AudioWorld`].
    pub struct ClipId;
    /// One playing instance of a clip in one [`AudioWorld`].
    pub struct SoundId;
}

/// Speed of sound in air, metres per second.
pub const SPEED_OF_SOUND: f32 = 343.0;
pub const AUDIO_MAX_CLIP_SECONDS: f32 = 600.0;
pub const AUDIO_MIN_SAMPLE_RATE: u32 = 8_000;
pub const AUDIO_MAX_SAMPLE_RATE: u32 = 192_000;
/// Doppler pitch is clamped to this range (half to double).
pub const DOPPLER_RANGE: (f32, f32) = (0.5, 2.0);
/// A per-tick move faster than this is treated as a teleport: no Doppler that tick.
const TELEPORT_SPEED: f32 = 200.0;
const COMMAND_CAPACITY: usize = 1024;
/// Music drops by this much while speech plays, unless the listener says otherwise.
pub const DEFAULT_DUCKING_DB: f32 = -9.0;
/// Time constants (seconds) for ducking to pull music down and to let it back up.
const DUCK_ATTACK: f32 = 0.15;
const DUCK_RELEASE: f32 = 0.6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioError {
    InvalidClip(&'static str),
    Decode(String),
    InvalidSettings(&'static str),
    StaleClip,
    StaleSound,
    /// Every sound slot is in use; stop a sound or raise `max_sounds`.
    TooManySounds,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidClip(reason) => write!(f, "invalid audio clip: {reason}"),
            Self::Decode(reason) => write!(f, "cannot decode audio: {reason}"),
            Self::InvalidSettings(reason) => write!(f, "invalid sound settings: {reason}"),
            Self::StaleClip => f.write_str("audio clip no longer exists"),
            Self::StaleSound => f.write_str("sound is no longer playing"),
            Self::TooManySounds => f.write_str("too many sounds are playing"),
        }
    }
}

impl std::error::Error for AudioError {}

/// Decoded PCM audio, interleaved `f32` samples in -1..1.
#[derive(Clone, Debug)]
pub struct Clip {
    samples: Arc<[f32]>,
    channels: u16,
    sample_rate: u32,
    /// Loop start and end in frames; the whole clip when absent.
    loop_frames: Option<(usize, usize)>,
}

impl Clip {
    pub fn new(samples: Vec<f32>, channels: u16, sample_rate: u32) -> Result<Self, AudioError> {
        if !(1..=2).contains(&channels) {
            return Err(AudioError::InvalidClip("channels must be 1 or 2"));
        }
        if !(AUDIO_MIN_SAMPLE_RATE..=AUDIO_MAX_SAMPLE_RATE).contains(&sample_rate) {
            return Err(AudioError::InvalidClip("sample rate out of range"));
        }
        if samples.is_empty() || !samples.len().is_multiple_of(channels as usize) {
            return Err(AudioError::InvalidClip("sample count must be whole frames"));
        }
        let frames = samples.len() / channels as usize;
        if frames as f32 > AUDIO_MAX_CLIP_SECONDS * sample_rate as f32 {
            return Err(AudioError::InvalidClip("clip is too long"));
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(AudioError::InvalidClip("samples must be finite"));
        }
        Ok(Self {
            samples: samples.into(),
            channels,
            sample_rate,
            loop_frames: None,
        })
    }

    /// Sets loop points in seconds. Looping sounds wrap from `end` back to `start`.
    pub fn with_loop(mut self, start: f32, end: f32) -> Result<Self, AudioError> {
        let rate = self.sample_rate as f32;
        let (start, end) = (
            (start * rate).round() as usize,
            (end * rate).round() as usize,
        );
        if !(start < end && end <= self.frames() && end - start >= 2) {
            return Err(AudioError::InvalidClip(
                "loop points must lie inside the clip",
            ));
        }
        self.loop_frames = Some((start, end));
        Ok(self)
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames() as f64 / self.sample_rate as f64)
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Interleaved samples, as the mixer plays them.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Largest absolute sample value (0..1 for normalised audio).
    pub fn peak(&self) -> f32 {
        self.samples
            .iter()
            .fold(0.0, |peak, sample| peak.max(sample.abs()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bus {
    Sfx,
    Music,
    Voice,
    Ui,
}

impl Bus {
    const COUNT: usize = 4;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rolloff {
    /// Natural 1/distance falloff, eased to silence at `max_distance`.
    Inverse,
    Linear,
}

/// Where a sound is heard from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayAt {
    /// Not positioned: music, UI and narration.
    Flat,
    Point(Vec3),
    /// Follows an entity; the sound stops when the entity is deleted.
    Entity(EntityId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundSettings {
    pub looping: bool,
    pub volume: f32,
    pub pitch: f32,
    /// 0 plays flat, 1 is fully positioned.
    pub spatial: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff: Rolloff,
    /// Doppler strength; 0 disables it.
    pub doppler: f32,
    /// Send to the listener's reverb, 0..1 (heard only when the listener has a room).
    pub reverb: f32,
    pub bus: Bus,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            looping: false,
            volume: 1.0,
            pitch: 1.0,
            spatial: 1.0,
            min_distance: 1.0,
            max_distance: 50.0,
            rolloff: Rolloff::Inverse,
            doppler: 1.0,
            reverb: 0.3,
            bus: Bus::Sfx,
        }
    }
}

impl SoundSettings {
    pub fn validate(&self) -> Result<(), AudioError> {
        let finite = [
            self.volume,
            self.pitch,
            self.spatial,
            self.min_distance,
            self.max_distance,
            self.doppler,
            self.reverb,
        ]
        .iter()
        .all(|value| value.is_finite());
        if !finite {
            return Err(AudioError::InvalidSettings("values must be finite"));
        }
        if self.volume < 0.0
            || !(0.0..=1.0).contains(&self.spatial)
            || !(0.0..=1.0).contains(&self.reverb)
            || self.doppler < 0.0
        {
            return Err(AudioError::InvalidSettings(
                "volume and doppler must be non-negative and spatial within 0..1",
            ));
        }
        if !(0.1..=4.0).contains(&self.pitch) {
            return Err(AudioError::InvalidSettings("pitch must be within 0.1..4"));
        }
        if self.min_distance <= 0.0 || self.max_distance <= self.min_distance {
            return Err(AudioError::InvalidSettings(
                "distances must satisfy 0 < min_distance < max_distance",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioConfig {
    /// Output rate of the device (or of headless rendering).
    pub device_rate: u32,
    /// Sounds mixed at once; quieter sounds beyond this keep time but are not mixed.
    pub real_voices: usize,
    /// Sounds that may exist at once, mixed or virtual.
    pub max_sounds: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_rate: 48_000,
            real_voices: 32,
            max_sounds: 128,
        }
    }
}

struct Sound {
    /// `None` for a stream.
    clip: Option<ClipId>,
    at: PlayAt,
    settings: SoundSettings,
    slot: usize,
    serial: u32,
    last_position: Option<Vec3>,
    last_params: Option<VoiceParams>,
    loudness: f32,
}

/// Engine-thread audio state. Create with [`AudioWorld::new`], which also returns the
/// [`Mixer`] to hand to an output device.
pub struct AudioWorld {
    config: AudioConfig,
    clips: SlotMap<ClipId, Clip>,
    sounds: SlotMap<SoundId, Sound>,
    /// Slot index -> owning sound; slots index the mixer's voice array.
    slots: Vec<Option<SoundId>>,
    free_slots: Vec<usize>,
    next_serial: u32,
    commands: SyncSender<Command>,
    finished: Receiver<Finished>,
    /// Play and Stop commands that did not fit; Params are simply resent next tick.
    backlog: Vec<Command>,
    listener: Option<EntityId>,
    listener_position: Option<Vec3>,
    bus_gains: [f32; Bus::COUNT],
    master_gain: f32,
    /// Set from the scene's audio listener each tick.
    listener_gain: f32,
    /// Clip assets by path, for scene emitters and [`load_clip`](Self::load_clip).
    clip_cache: HashMap<String, ClipLoad>,
    loader: Option<(
        std::sync::mpsc::Sender<Arc<AudioClipAsset>>,
        Receiver<Loaded>,
    )>,
    order: Vec<SoundId>,
    ended: Vec<SoundId>,
    audible: usize,
    reverb: ReverbSettings,
    /// Music bus change in dB while a voice-bus sound is audible (0 = off).
    pub(crate) ducking_db: f32,
    /// Current music multiplier from ducking, eased toward its target each tick.
    duck: f32,
    /// Signals sent this tick as (object, name hash); reused by scene sync.
    fired: Vec<(EntityId, u64)>,
}

impl AudioWorld {
    pub fn new(config: AudioConfig) -> Result<(Self, Mixer), AudioError> {
        if !(AUDIO_MIN_SAMPLE_RATE..=AUDIO_MAX_SAMPLE_RATE).contains(&config.device_rate)
            || config.real_voices == 0
            || config.max_sounds < config.real_voices
            || config.max_sounds > 4096
        {
            return Err(AudioError::InvalidSettings(
                "audio config needs a valid rate and 0 < real_voices <= max_sounds <= 4096",
            ));
        }
        let (commands, command_rx) = sync_channel(COMMAND_CAPACITY);
        let (finished_tx, finished) = sync_channel(config.max_sounds);
        let mixer = Mixer::new(
            command_rx,
            finished_tx,
            config.max_sounds,
            config.device_rate,
        );
        let world = Self {
            config,
            clips: SlotMap::with_key(),
            sounds: SlotMap::with_capacity_and_key(config.max_sounds),
            slots: vec![None; config.max_sounds],
            free_slots: (0..config.max_sounds).rev().collect(),
            next_serial: 0,
            commands,
            finished,
            backlog: Vec::with_capacity(config.max_sounds * 2),
            listener: None,
            listener_position: None,
            bus_gains: [1.0; Bus::COUNT],
            master_gain: 1.0,
            listener_gain: 1.0,
            clip_cache: HashMap::new(),
            loader: None,
            order: Vec::with_capacity(config.max_sounds),
            ended: Vec::with_capacity(config.max_sounds),
            audible: 0,
            reverb: ReverbSettings::OFF,
            ducking_db: DEFAULT_DUCKING_DB,
            duck: 1.0,
            fired: Vec::new(),
        };
        Ok((world, mixer))
    }

    pub fn add_clip(&mut self, clip: Clip) -> ClipId {
        self.clips.insert(clip)
    }

    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.get(id)
    }

    /// Decodes a clip asset now (slow: call while loading), or returns the cached clip.
    pub fn load_clip(&mut self, asset: &AudioClipAsset) -> Result<ClipId, AudioError> {
        if let Some(ClipLoad::Ready(id)) = self.clip_cache.get(&asset.path) {
            if self.clips.contains_key(*id) {
                return Ok(*id);
            }
        }
        let id = self.add_clip(asset.decode()?);
        self.clip_cache
            .insert(asset.path.clone(), ClipLoad::Ready(id));
        Ok(id)
    }

    /// The decoded clip for `asset`, or `None` while it loads on the background
    /// loader (or after it failed, which is reported once).
    pub fn clip_for(&mut self, asset: &Arc<AudioClipAsset>) -> Option<ClipId> {
        match self.clip_cache.get(&asset.path) {
            Some(ClipLoad::Ready(id)) if self.clips.contains_key(*id) => return Some(*id),
            Some(ClipLoad::Pending | ClipLoad::Failed) => return None,
            _ => {}
        }
        if self.loader.is_none() {
            self.loader = asset::spawn_loader();
        }
        let sent = self
            .loader
            .as_ref()
            .is_some_and(|(requests, _)| requests.send(asset.clone()).is_ok());
        let state = if sent {
            ClipLoad::Pending
        } else {
            ClipLoad::Failed
        };
        if !sent {
            eprintln!("audio: {}: the clip loader is unavailable", asset.path);
        }
        self.clip_cache.insert(asset.path.clone(), state);
        None
    }

    fn drain_loads(&mut self) {
        let Some((_, results)) = &self.loader else {
            return;
        };
        while let Ok(Loaded { path, result }) = results.try_recv() {
            let state = match result {
                Ok(clip) => ClipLoad::Ready(self.clips.insert(clip)),
                Err(error) => {
                    eprintln!("audio: {path}: {error}");
                    ClipLoad::Failed
                }
            };
            self.clip_cache.insert(path, state);
        }
    }

    /// Removes a clip and stops every sound playing it.
    pub fn remove_clip(&mut self, id: ClipId) -> Result<Clip, AudioError> {
        let playing: Vec<SoundId> = self
            .sounds
            .iter()
            .filter(|(_, sound)| sound.clip == Some(id))
            .map(|(sound, _)| sound)
            .collect();
        for sound in playing {
            self.stop(sound)?;
        }
        self.clip_cache
            .retain(|_, state| !matches!(state, ClipLoad::Ready(ready) if *ready == id));
        self.clips.remove(id).ok_or(AudioError::StaleClip)
    }

    /// Starts a sound. It is positioned and becomes audible on the next [`update`](Self::update).
    pub fn play(
        &mut self,
        clip: ClipId,
        at: PlayAt,
        settings: SoundSettings,
    ) -> Result<SoundId, AudioError> {
        settings.validate()?;
        let data = self.clips.get(clip).ok_or(AudioError::StaleClip)?;
        let source = Source::Clip {
            samples: data.samples.clone(),
            channels: data.channels as usize,
            sample_rate: data.sample_rate,
            loop_frames: data.loop_frames,
        };
        self.start(source, Some(clip), at, settings)
    }

    /// Streams a clip asset from disk on its own decoder thread, for long music. Trim,
    /// gain, fades and loop points apply as for decoded clips.
    pub fn play_stream(
        &mut self,
        asset: &Arc<AudioClipAsset>,
        at: PlayAt,
        settings: SoundSettings,
    ) -> Result<SoundId, AudioError> {
        settings.validate()?;
        if self.free_slots.is_empty() {
            return Err(AudioError::TooManySounds);
        }
        let reader = stream::open(asset.clone(), settings.looping)?;
        self.start(Source::Stream(Box::new(reader)), None, at, settings)
    }

    fn start(
        &mut self,
        source: Source,
        clip: Option<ClipId>,
        at: PlayAt,
        settings: SoundSettings,
    ) -> Result<SoundId, AudioError> {
        let slot = self.free_slots.pop().ok_or(AudioError::TooManySounds)?;
        self.next_serial = self.next_serial.wrapping_add(1);
        let serial = self.next_serial;
        self.send(Command::Play {
            slot,
            serial,
            source,
            looping: settings.looping,
        });
        let id = self.sounds.insert(Sound {
            clip,
            at,
            settings,
            slot,
            serial,
            last_position: None,
            last_params: None,
            loudness: 0.0,
        });
        self.slots[slot] = Some(id);
        Ok(id)
    }

    /// Stops a sound with a short fade. A stale handle is an error, never another sound.
    pub fn stop(&mut self, id: SoundId) -> Result<(), AudioError> {
        let sound = self.sounds.remove(id).ok_or(AudioError::StaleSound)?;
        self.release_slot(sound.slot);
        self.send(Command::Stop { slot: sound.slot });
        Ok(())
    }

    pub fn is_playing(&self, id: SoundId) -> bool {
        self.sounds.contains_key(id)
    }

    pub fn set_volume(&mut self, id: SoundId, volume: f32) -> Result<(), AudioError> {
        if !volume.is_finite() || volume < 0.0 {
            return Err(AudioError::InvalidSettings(
                "volume must be finite and non-negative",
            ));
        }
        let sound = self.sounds.get_mut(id).ok_or(AudioError::StaleSound)?;
        sound.settings.volume = volume;
        Ok(())
    }

    /// Hears from this entity. `None` hears from the [`ActiveCamera`].
    pub fn set_listener(&mut self, listener: Option<EntityId>) {
        if self.listener == listener {
            return;
        }
        self.listener = listener;
        self.listener_position = None;
    }

    pub fn set_bus_gain(&mut self, bus: Bus, gain: f32) -> Result<(), AudioError> {
        if !gain.is_finite() || gain < 0.0 {
            return Err(AudioError::InvalidSettings(
                "gain must be finite and non-negative",
            ));
        }
        self.bus_gains[bus as usize] = gain;
        Ok(())
    }

    /// The shared room every sound's reverb send feeds. `room` 0 turns it off.
    pub fn set_reverb(&mut self, settings: ReverbSettings) {
        let settings = ReverbSettings {
            room: settings.room.clamp(0.0, 1.0),
            damping: settings.damping.clamp(0.0, 1.0),
        };
        if settings != self.reverb {
            self.reverb = settings;
            self.send(Command::Reverb(settings));
        }
    }

    /// How far music drops while speech plays, in dB (0 turns ducking off).
    pub fn set_ducking(&mut self, db: f32) -> Result<(), AudioError> {
        if !db.is_finite() || !(-60.0..=0.0).contains(&db) {
            return Err(AudioError::InvalidSettings(
                "ducking must be within -60..0 dB",
            ));
        }
        self.ducking_db = db;
        Ok(())
    }

    pub fn set_master_gain(&mut self, gain: f32) -> Result<(), AudioError> {
        if !gain.is_finite() || gain < 0.0 {
            return Err(AudioError::InvalidSettings(
                "gain must be finite and non-negative",
            ));
        }
        self.master_gain = gain;
        Ok(())
    }

    /// Speed multiplier from the last update: pitch × Doppler (1 is the clip's own speed).
    pub fn playback_rate(&self, id: SoundId) -> Option<f32> {
        Some(self.sounds.get(id)?.last_params?.rate as f32)
    }

    pub fn sound_count(&self) -> usize {
        self.sounds.len()
    }

    /// Sounds mixed after the last update (the rest are virtual or silent).
    pub fn audible_count(&self) -> usize {
        self.audible
    }

    /// Positions every sound from propagated transforms. Call once per fixed tick with
    /// that tick's delta. Allocation-free in steady state.
    pub fn update(&mut self, world: &World, dt: Duration) {
        self.drain_finished();
        self.drain_loads();
        self.flush_backlog();
        let dt = dt.as_secs_f32().max(f32::EPSILON);

        let listener = self
            .listener
            .or_else(|| world.resource::<ActiveCamera>().map(|camera| camera.0))
            .and_then(|id| world.global_transform(id).ok().copied());
        let (listener_position, right) = match listener {
            Some(transform) => (
                transform.translation(),
                transform
                    .matrix()
                    .transform_vector3(Vec3::X)
                    .try_normalize()
                    .unwrap_or(Vec3::X),
            ),
            None => (Vec3::ZERO, Vec3::X),
        };
        let listener_velocity = velocity(self.listener_position, listener_position, dt);
        self.listener_position = Some(listener_position);

        self.order.clear();
        let mut speaking = false;
        self.ended.clear();
        for (id, sound) in &mut self.sounds {
            let position = match sound.at {
                PlayAt::Flat => None,
                PlayAt::Point(point) => Some(point),
                PlayAt::Entity(entity) => match world.global_transform(entity) {
                    Ok(transform) => Some(transform.translation()),
                    Err(_) => {
                        self.ended.push(id);
                        continue;
                    }
                },
            };
            let settings = sound.settings;
            let mut gains = [1.0, 1.0];
            let mut doppler = 1.0;
            if let Some(position) = position {
                let offset = listener_position - position;
                let distance = offset.length();
                let attenuation = distance_gain(
                    distance,
                    settings.min_distance,
                    settings.max_distance,
                    settings.rolloff,
                );
                let toward_listener = offset.try_normalize();
                let pan = toward_listener.map_or(0.0, |direction| (-direction).dot(right));
                let positioned = pan_gains(pan).map(|gain| gain * attenuation);
                gains = [0, 1].map(|i| lerp(1.0, positioned[i], settings.spatial));
                if let Some(direction) = toward_listener {
                    let source_velocity = velocity(sound.last_position, position, dt);
                    doppler = doppler_factor(
                        direction,
                        source_velocity * settings.doppler,
                        listener_velocity * settings.doppler,
                    );
                }
                sound.last_position = Some(position);
            }
            let level = settings.volume
                * self.bus_gains[settings.bus as usize]
                * self.master_gain
                * self.listener_gain;
            let gains = gains.map(|gain| gain * level);
            sound.loudness = gains[0].max(gains[1]);
            speaking |= settings.bus == Bus::Voice && sound.loudness > 1e-3;
            sound.last_params = Some(VoiceParams {
                gains,
                send: settings.reverb,
                rate: settings.pitch as f64 * doppler as f64,
                audible: false,
            });
            self.order.push(id);
        }
        for index in 0..self.ended.len() {
            let _ = self.stop(self.ended[index]);
        }
        // Ducking eases music toward its target: quickly down, slowly back up.
        let target = if speaking && self.ducking_db < 0.0 {
            10f32.powf(self.ducking_db / 20.0)
        } else {
            1.0
        };
        let seconds = if target < self.duck {
            DUCK_ATTACK
        } else {
            DUCK_RELEASE
        };
        let step = (dt / seconds).min(1.0);
        self.duck += (target - self.duck) * step;
        if (self.duck - target).abs() < 1e-3 {
            self.duck = target;
        }

        // Loudest sounds get the real voices; sort_unstable_by does not allocate.
        let sounds = &self.sounds;
        self.order
            .sort_unstable_by(|a, b| sounds[*b].loudness.total_cmp(&sounds[*a].loudness));
        self.audible = 0;
        for (rank, &id) in self.order.iter().enumerate() {
            let sound = &mut self.sounds[id];
            let Some(params) = sound.last_params.as_mut() else {
                continue;
            };
            if sound.settings.bus == Bus::Music {
                params.gains = params.gains.map(|gain| gain * self.duck);
            }
            params.audible = rank < self.config.real_voices && sound.loudness > 0.0;
            self.audible += params.audible as usize;
            let command = Command::Params {
                slot: sound.slot,
                params: *params,
            };
            // A full channel only skips this tick's update; the next tick resends.
            let _ = self.commands.try_send(command);
        }
    }

    fn drain_finished(&mut self) {
        while let Ok(done) = self.finished.try_recv() {
            let Some(id) = self.slots[done.slot] else {
                continue;
            };
            if self
                .sounds
                .get(id)
                .is_some_and(|sound| sound.serial == done.serial)
            {
                self.sounds.remove(id);
                self.release_slot(done.slot);
            }
        }
    }

    fn release_slot(&mut self, slot: usize) {
        self.slots[slot] = None;
        self.free_slots.push(slot);
    }

    fn send(&mut self, command: Command) {
        if !self.backlog.is_empty() {
            self.backlog.push(command);
            return;
        }
        match self.commands.try_send(command) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(command)) => self.backlog.push(command),
        }
    }

    fn flush_backlog(&mut self) {
        let mut sent = 0;
        while sent < self.backlog.len() {
            // Swap the command out so a failed send can put it straight back.
            let command =
                std::mem::replace(&mut self.backlog[sent], Command::Stop { slot: usize::MAX });
            match self.commands.try_send(command) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => sent += 1,
                Err(TrySendError::Full(command)) => {
                    self.backlog[sent] = command;
                    break;
                }
            }
        }
        self.backlog.drain(..sent);
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn velocity(previous: Option<Vec3>, current: Vec3, dt: f32) -> Vec3 {
    let Some(previous) = previous else {
        return Vec3::ZERO;
    };
    let velocity = (current - previous) / dt;
    if velocity.length() > TELEPORT_SPEED {
        Vec3::ZERO
    } else {
        velocity
    }
}

/// Gain for a source `distance` metres away: 1 inside `min`, 0 beyond `max`.
pub fn distance_gain(distance: f32, min: f32, max: f32, rolloff: Rolloff) -> f32 {
    if distance <= min {
        return 1.0;
    }
    if distance >= max {
        return 0.0;
    }
    match rolloff {
        Rolloff::Linear => 1.0 - (distance - min) / (max - min),
        // min/d, shifted and rescaled so it reaches exactly zero at max.
        Rolloff::Inverse => {
            let floor = min / max;
            (min / distance - floor) / (1.0 - floor)
        }
    }
}

/// Equal-power stereo gains for `pan` in -1 (left) ..1 (right); centre is [1, 1].
fn pan_gains(pan: f32) -> [f32; 2] {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    let scale = std::f32::consts::SQRT_2;
    [angle.cos() * scale, angle.sin() * scale]
}

/// Pitch multiplier. `direction` is the unit vector from source to listener.
pub fn doppler_factor(direction: Vec3, source_velocity: Vec3, listener_velocity: Vec3) -> f32 {
    let c = SPEED_OF_SOUND;
    // Keep both terms subsonic so the ratio stays finite and positive.
    let source = source_velocity.dot(direction).min(c * 0.9);
    let listener = listener_velocity.dot(direction).min(c * 0.9);
    ((c - listener) / (c - source)).clamp(DOPPLER_RANGE.0, DOPPLER_RANGE.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transform;

    const RATE: u32 = 48_000;
    const DT: Duration = Duration::from_millis(10);

    fn sine(frequency: f32, seconds: f32) -> Clip {
        let frames = (seconds * RATE as f32) as usize;
        let samples = (0..frames)
            .map(|i| (i as f32 / RATE as f32 * frequency * std::f32::consts::TAU).sin())
            .collect();
        Clip::new(samples, 1, RATE).unwrap()
    }

    fn setup() -> (AudioWorld, Mixer, World, EntityId) {
        let (audio, mixer) = AudioWorld::new(AudioConfig::default()).unwrap();
        let mut world = World::new();
        let listener = world.spawn("listener");
        world.insert_resource(ActiveCamera(listener));
        world.propagate_transforms();
        (audio, mixer, world, listener)
    }

    fn move_to(world: &mut World, id: EntityId, position: Vec3) {
        world
            .set_local_transform(id, Transform::from_translation(position))
            .unwrap();
        world.propagate_transforms();
    }

    fn render(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        mixer.render(&mut out);
        out
    }

    fn rate_of(audio: &AudioWorld, id: SoundId) -> f64 {
        audio.sounds[id].last_params.unwrap().rate
    }

    fn peak(out: &[f32]) -> f32 {
        out.iter().fold(0.0, |peak, sample| peak.max(sample.abs()))
    }

    #[test]
    fn the_limiter_keeps_loud_mixes_under_full_scale() {
        let (mut audio, mut mixer, world, _) = setup();
        let clip = audio.add_clip(sine(220.0, 1.0));
        for _ in 0..4 {
            audio
                .play(
                    clip,
                    PlayAt::Flat,
                    SoundSettings {
                        volume: 2.0,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        audio.update(&world, DT);
        let out = render(&mut mixer, 4800);
        assert!(peak(&out) <= 0.98 + 1e-4, "peak {}", peak(&out));
        assert!(peak(&out) > 0.9, "still loud: {}", peak(&out));
    }

    #[test]
    fn speech_ducks_music_and_music_recovers() {
        let (mut audio, _mixer, world, _) = setup();
        let clip = audio.add_clip(sine(220.0, 1.0));
        let flat = |bus| SoundSettings {
            looping: true,
            spatial: 0.0,
            bus,
            ..Default::default()
        };
        let music = audio.play(clip, PlayAt::Flat, flat(Bus::Music)).unwrap();
        audio.update(&world, DT);
        let full = audio.sounds[music].last_params.unwrap().gains[0];
        let voice = audio.play(clip, PlayAt::Flat, flat(Bus::Voice)).unwrap();
        for _ in 0..100 {
            audio.update(&world, DT);
        }
        let ducked = audio.sounds[music].last_params.unwrap().gains[0];
        assert!(
            (ducked / full - 10f32.powf(DEFAULT_DUCKING_DB / 20.0)).abs() < 0.01,
            "{ducked} of {full}"
        );
        audio.stop(voice).unwrap();
        for _ in 0..500 {
            audio.update(&world, DT);
        }
        assert!((audio.sounds[music].last_params.unwrap().gains[0] - full).abs() < 1e-3);
        audio.set_ducking(0.0).unwrap();
        assert!(audio.set_ducking(5.0).is_err());
    }

    #[test]
    fn reverb_adds_a_tail_only_when_the_listener_has_a_room() {
        let tail = |room: f32| {
            let (mut audio, mut mixer, world, _) = setup();
            audio.set_reverb(ReverbSettings { room, damping: 0.3 });
            let clip = audio.add_clip(sine(440.0, 0.02));
            let settings = SoundSettings {
                spatial: 0.0,
                reverb: 1.0,
                ..Default::default()
            };
            audio.play(clip, PlayAt::Flat, settings).unwrap();
            audio.update(&world, DT);
            let out = render(&mut mixer, RATE as usize / 2);
            // Everything after 0.1 s is reverb: the clip itself lasts 20 ms.
            peak(&out[RATE as usize / 5..])
        };
        assert_eq!(tail(0.0), 0.0);
        assert!(tail(0.8) > 1e-3, "tail {}", tail(0.8));
    }

    #[test]
    fn clips_are_validated() {
        assert!(Clip::new(vec![], 1, RATE).is_err());
        assert!(Clip::new(vec![0.0; 3], 2, RATE).is_err());
        assert!(Clip::new(vec![0.0; 4], 3, RATE).is_err());
        assert!(Clip::new(vec![0.0; 4], 1, 1_000).is_err());
        assert!(Clip::new(vec![f32::NAN], 1, RATE).is_err());
        assert!(sine(440.0, 0.1).with_loop(0.05, 0.2).is_err());
        assert_eq!(sine(440.0, 0.5).duration(), Duration::from_millis(500));
    }

    #[test]
    fn one_shot_ends_and_frees_its_sound() {
        let (mut audio, mut mixer, world, _) = setup();
        let clip = audio.add_clip(sine(440.0, 0.005));
        let sound = audio
            .play(clip, PlayAt::Flat, SoundSettings::default())
            .unwrap();
        audio.update(&world, DT);
        let out = render(&mut mixer, 512);
        assert!(out.iter().any(|sample| sample.abs() > 0.1));
        assert_eq!(mixer.active_voices(), 0);
        audio.update(&world, DT);
        assert!(!audio.is_playing(sound));
        assert_eq!(audio.stop(sound), Err(AudioError::StaleSound));
    }

    #[test]
    fn loop_wraps_without_a_click() {
        let (mut audio, mut mixer, world, _) = setup();
        // 100 Hz over exactly 10 periods loops seamlessly.
        let clip = audio.add_clip(sine(100.0, 0.1).with_loop(0.0, 0.1).unwrap());
        let settings = SoundSettings {
            looping: true,
            ..Default::default()
        };
        let sound = audio.play(clip, PlayAt::Flat, settings).unwrap();
        audio.update(&world, DT);
        let out = render(&mut mixer, RATE as usize / 2);
        let max_step = out
            .chunks(2)
            .zip(out.chunks(2).skip(1))
            .map(|(a, b)| (b[0] - a[0]).abs())
            .fold(0.0, f32::max);
        // A 100 Hz sine moves at most ~0.013 per sample at 48 kHz.
        assert!(max_step < 0.02, "discontinuity {max_step}");
        assert!(audio.is_playing(sound));
        assert_eq!(mixer.active_voices(), 1);
    }

    #[test]
    fn stop_fades_out_over_one_block() {
        let (mut audio, mut mixer, world, _) = setup();
        let clip = audio.add_clip(sine(100.0, 1.0));
        let sound = audio
            .play(clip, PlayAt::Flat, SoundSettings::default())
            .unwrap();
        audio.update(&world, DT);
        render(&mut mixer, 120);
        audio.stop(sound).unwrap();
        let out = render(&mut mixer, 256);
        assert!(out[0].abs() > 0.05, "fade starts at the playing level");
        assert!(out[out.len() - 2].abs() < 0.01, "fade ends silent");
        assert_eq!(mixer.active_voices(), 0);
    }

    #[test]
    fn distance_gain_follows_rolloff() {
        assert_eq!(distance_gain(0.5, 1.0, 10.0, Rolloff::Inverse), 1.0);
        assert_eq!(distance_gain(10.0, 1.0, 10.0, Rolloff::Inverse), 0.0);
        assert!((distance_gain(5.5, 1.0, 10.0, Rolloff::Linear) - 0.5).abs() < 1e-6);
        let near = distance_gain(2.0, 1.0, 10.0, Rolloff::Inverse);
        let far = distance_gain(4.0, 1.0, 10.0, Rolloff::Inverse);
        assert!(near > far && far > 0.0);
    }

    #[test]
    fn sources_pan_toward_their_side() {
        let (mut audio, _mixer, mut world, _) = setup();
        let clip = audio.add_clip(sine(440.0, 1.0));
        let sound = audio
            .play(
                clip,
                PlayAt::Point(Vec3::new(5.0, 0.0, 0.0)),
                SoundSettings::default(),
            )
            .unwrap();
        audio.update(&world, DT);
        let [left, right] = audio.sounds[sound].last_params.unwrap().gains;
        assert!(right > left * 4.0, "left {left} right {right}");
        // Turn the listener to face +X: the source is now straight ahead.
        world
            .set_local_transform(
                world.resource::<ActiveCamera>().unwrap().0,
                Transform::from_rotation(crate::Quat::from_rotation_y(
                    -std::f32::consts::FRAC_PI_2,
                )),
            )
            .unwrap();
        world.propagate_transforms();
        audio.update(&world, DT);
        let [left, right] = audio.sounds[sound].last_params.unwrap().gains;
        assert!((left - right).abs() < 1e-4);
    }

    #[test]
    fn doppler_raises_approaching_and_lowers_receding_pitch() {
        let (mut audio, _mixer, mut world, listener) = setup();
        let car = world.spawn("car");
        move_to(&mut world, car, Vec3::new(0.0, 0.0, -40.0));
        let clip = audio.add_clip(sine(440.0, 1.0));
        let settings = SoundSettings {
            max_distance: 200.0,
            ..Default::default()
        };
        let sound = audio.play(clip, PlayAt::Entity(car), settings).unwrap();
        audio.update(&world, DT);
        assert_eq!(rate_of(&audio, sound), 1.0);

        // 30 m/s toward the listener: 343 / 313.
        move_to(&mut world, car, Vec3::new(0.0, 0.0, -39.7));
        audio.update(&world, DT);
        let expected = SPEED_OF_SOUND / (SPEED_OF_SOUND - 30.0);
        assert!((rate_of(&audio, sound) as f32 - expected).abs() < 1e-3);

        move_to(&mut world, car, Vec3::new(0.0, 0.0, -40.0));
        audio.update(&world, DT);
        assert!(rate_of(&audio, sound) < 1.0);

        // A listener moving toward a still source also hears it higher.
        audio.update(&world, DT);
        move_to(&mut world, listener, Vec3::new(0.0, 0.0, -0.3));
        audio.update(&world, DT);
        assert!(rate_of(&audio, sound) > 1.0);
    }

    #[test]
    fn teleports_and_disabled_doppler_keep_pitch() {
        let (mut audio, _mixer, mut world, _) = setup();
        let car = world.spawn("car");
        move_to(&mut world, car, Vec3::new(0.0, 0.0, -40.0));
        let clip = audio.add_clip(sine(440.0, 1.0));
        let settings = SoundSettings {
            max_distance: 200.0,
            ..Default::default()
        };
        let teleported = audio.play(clip, PlayAt::Entity(car), settings).unwrap();
        let disabled = audio
            .play(
                clip,
                PlayAt::Entity(car),
                SoundSettings {
                    doppler: 0.0,
                    ..settings
                },
            )
            .unwrap();
        audio.update(&world, DT);
        move_to(&mut world, car, Vec3::new(0.0, 0.0, -10.0));
        audio.update(&world, DT);
        assert_eq!(rate_of(&audio, teleported), 1.0);
        move_to(&mut world, car, Vec3::new(0.0, 0.0, -9.7));
        audio.update(&world, DT);
        assert_eq!(rate_of(&audio, disabled), 1.0);
        assert!(rate_of(&audio, teleported) > 1.0);
    }

    #[test]
    fn deleted_entities_stop_their_sounds() {
        let (mut audio, mut mixer, mut world, _) = setup();
        let car = world.spawn("car");
        world.propagate_transforms();
        let clip = audio.add_clip(sine(440.0, 1.0));
        let sound = audio
            .play(clip, PlayAt::Entity(car), SoundSettings::default())
            .unwrap();
        audio.update(&world, DT);
        world.remove_subtree(car).unwrap();
        audio.update(&world, DT);
        assert!(!audio.is_playing(sound));
        render(&mut mixer, 64);
        assert_eq!(mixer.active_voices(), 0);
    }

    #[test]
    fn removing_a_clip_stops_its_sounds() {
        let (mut audio, _mixer, _world, _) = setup();
        let clip = audio.add_clip(sine(440.0, 1.0));
        let sound = audio
            .play(clip, PlayAt::Flat, SoundSettings::default())
            .unwrap();
        audio.remove_clip(clip).unwrap();
        assert!(!audio.is_playing(sound));
        assert_eq!(
            audio.play(clip, PlayAt::Flat, SoundSettings::default()),
            Err(AudioError::StaleClip)
        );
    }

    #[test]
    fn quietest_sounds_beyond_the_voice_budget_are_virtual() {
        let (mut audio, mut mixer, world, _) = setup();
        let clip = audio.add_clip(sine(440.0, 1.0));
        for i in 0..40 {
            let at = PlayAt::Point(Vec3::new(0.0, 0.0, -(1.0 + i as f32)));
            audio.play(clip, at, SoundSettings::default()).unwrap();
        }
        audio.update(&world, DT);
        assert_eq!(audio.audible_count(), 32);
        render(&mut mixer, 64);
        assert_eq!(mixer.active_voices(), 40, "virtual sounds keep their place");
    }

    #[test]
    fn sound_slots_are_bounded_and_reused() {
        let config = AudioConfig {
            real_voices: 2,
            max_sounds: 2,
            ..Default::default()
        };
        let (mut audio, mut mixer) = AudioWorld::new(config).unwrap();
        let world = World::new();
        let clip = audio.add_clip(sine(440.0, 1.0));
        let first = audio
            .play(clip, PlayAt::Flat, SoundSettings::default())
            .unwrap();
        audio
            .play(clip, PlayAt::Flat, SoundSettings::default())
            .unwrap();
        assert_eq!(
            audio.play(clip, PlayAt::Flat, SoundSettings::default()),
            Err(AudioError::TooManySounds)
        );
        audio.stop(first).unwrap();
        let third = audio
            .play(clip, PlayAt::Flat, SoundSettings::default())
            .unwrap();
        render(&mut mixer, 64);
        // The stopped voice's late finish notice must not end the sound now in its slot.
        audio.update(&world, DT);
        assert!(audio.is_playing(third));
    }

    #[test]
    fn settings_are_validated() {
        let (mut audio, _mixer, _world, _) = setup();
        let clip = audio.add_clip(sine(440.0, 1.0));
        for settings in [
            SoundSettings {
                volume: -1.0,
                ..Default::default()
            },
            SoundSettings {
                pitch: 0.0,
                ..Default::default()
            },
            SoundSettings {
                min_distance: 5.0,
                max_distance: 5.0,
                ..Default::default()
            },
            SoundSettings {
                spatial: f32::NAN,
                ..Default::default()
            },
        ] {
            assert!(audio.play(clip, PlayAt::Flat, settings).is_err());
        }
    }
}
