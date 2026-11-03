//! `.audio.toml` clip assets, the scene components that play them, and the background
//! loader that decodes clips off the frame path.
use super::{AudioError, AudioWorld, Bus, Clip, ClipId, PlayAt, SoundId, SoundSettings};
use crate::{ActiveCamera, EntityId, Project, World};
use bevy_ecs::component::Component;
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc};

/// Largest source file accepted (compressed or not).
pub const AUDIO_MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const SOURCE_EXTENSIONS: [&str; 4] = ["wav", "mp3", "ogg", "flac"];
const MAX_TOML_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AudioFile {
    schema_version: u32,
    id: String,
    name: String,
    kind: Option<String>,
    #[serde(default)]
    description: String,
    /// File name of the audio data, in the same folder as the `.audio.toml`.
    source: String,
    #[serde(default = "default_category")]
    category: String,
    #[serde(default)]
    gain_db: f32,
    /// Seconds of the source to keep.
    trim: Option<[f32; 2]>,
    /// Loop points in seconds, measured from the trimmed start.
    #[serde(rename = "loop")]
    loop_points: Option<[f32; 2]>,
    /// Seconds of fade at the start and end of the trimmed clip.
    #[serde(default)]
    fade_in: f32,
    #[serde(default)]
    fade_out: f32,
    /// Stream from disk instead of decoding whole; defaults to true for music.
    stream: Option<bool>,
    /// Provenance written by the IDE generator; not used at runtime.
    generation: Option<toml::Table>,
}

fn default_category() -> String {
    "sfx".into()
}

/// A validated clip asset. Holds where the audio is and how to prepare it, not samples.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioClipAsset {
    /// Project-relative path of the `.audio.toml`.
    pub path: String,
    pub id: String,
    pub name: String,
    pub bus: Bus,
    pub gain_db: f32,
    pub trim: Option<[f32; 2]>,
    pub loop_points: Option<[f32; 2]>,
    pub fade_in: f32,
    pub fade_out: f32,
    stream: bool,
    file: PathBuf,
}

impl AudioClipAsset {
    pub fn load(project: &Project, path: &str) -> Result<Self, String> {
        let fail = |e: String| format!("{path}: {e}");
        if !path.ends_with(".audio.toml") {
            return Err(fail("audio clips must be .audio.toml files".into()));
        }
        let source = project
            .read_text_asset(path)
            .map_err(|e| fail(e.to_string()))?;
        Self::from_source(project, path, &source)
    }

    /// Validates `source` as the contents of `path` (an editor candidate before saving).
    pub fn from_source(project: &Project, path: &str, source: &str) -> Result<Self, String> {
        let fail = |e: String| format!("{path}: {e}");
        if !path.ends_with(".audio.toml") {
            return Err(fail("audio clips must be .audio.toml files".into()));
        }
        if source.len() > MAX_TOML_BYTES {
            return Err(fail("audio file exceeds 64 KiB".into()));
        }
        let file: AudioFile = toml::from_str(source).map_err(|e| fail(e.to_string()))?;
        let _metadata = (file.description, file.generation);
        if file.schema_version != 1 || !crate::scene::identifier(&file.id) {
            return Err(fail("requires schema_version = 1 and a valid id".into()));
        }
        if file.kind.as_deref().is_some_and(|kind| kind != "audio") {
            return Err(fail("kind must be audio".into()));
        }
        if file.name.trim().is_empty() || file.name.len() > 64 {
            return Err(fail("name must be 1-64 characters".into()));
        }
        let bus = match file.category.as_str() {
            "sfx" => Bus::Sfx,
            "music" => Bus::Music,
            "voice" => Bus::Voice,
            "ui" => Bus::Ui,
            _ => return Err(fail("category must be sfx, music, voice or ui".into())),
        };
        if !(file.gain_db.is_finite() && (-60.0..=24.0).contains(&file.gain_db)) {
            return Err(fail("gain_db must be within -60..24".into()));
        }
        let span = |range: Option<[f32; 2]>, name: &str| match range {
            Some([start, end])
                if !(start.is_finite() && end.is_finite() && 0.0 <= start && start < end) =>
            {
                Err(fail(format!(
                    "{name} must be [start, end] seconds with 0 <= start < end"
                )))
            }
            _ => Ok(()),
        };
        span(file.trim, "trim")?;
        span(file.loop_points, "loop")?;
        if ![file.fade_in, file.fade_out]
            .iter()
            .all(|fade| fade.is_finite() && (0.0..=60.0).contains(fade))
        {
            return Err(fail(
                "fade_in and fade_out must be within 0..60 seconds".into(),
            ));
        }
        if let (Some([start, end]), Some([_, loop_end])) = (file.trim, file.loop_points) {
            if loop_end > end - start {
                return Err(fail("loop must end inside the trimmed clip".into()));
            }
        }
        let extension = std::path::Path::new(&file.source)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let bare = !file.source.is_empty()
            && !file.source.contains(['/', '\\'])
            && !file.source.starts_with('.');
        if !bare || !extension.is_some_and(|e| SOURCE_EXTENSIONS.contains(&e.as_str())) {
            return Err(fail(
                "source must be a .wav, .mp3, .ogg or .flac file name in the same folder".into(),
            ));
        }
        let relative = std::path::Path::new(path)
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .join(&file.source);
        let resolved = project
            .asset_path(&relative)
            .map_err(|e| fail(e.to_string()))?;
        let size = std::fs::metadata(&resolved)
            .map_err(|e| fail(format!("{}: {e}", file.source)))?
            .len();
        if size > AUDIO_MAX_FILE_BYTES {
            return Err(fail(format!(
                "{} exceeds {} MiB",
                file.source,
                AUDIO_MAX_FILE_BYTES >> 20
            )));
        }
        Ok(Self {
            path: path.to_string(),
            id: file.id,
            name: file.name,
            bus,
            gain_db: file.gain_db,
            trim: file.trim,
            loop_points: file.loop_points,
            fade_in: file.fade_in,
            fade_out: file.fade_out,
            stream: file.stream.unwrap_or(bus == Bus::Music),
            file: resolved,
        })
    }

    /// Gain and fades for the trimmed clip, which is `frames` long when known.
    fn shape(&self, rate: u32, frames: Option<usize>) -> Shape {
        Shape {
            gain: 10f32.powf(self.gain_db / 20.0),
            fade_in: (self.fade_in * rate as f32) as usize,
            fade_out: (self.fade_out * rate as f32) as usize,
            frames,
        }
    }

    /// Decodes, trims and applies gain and fades. Slow: call at load time or on the
    /// loader thread.
    pub fn decode(&self) -> Result<Clip, AudioError> {
        let (samples, channels, rate) = decode_file(&self.file)?;
        let channels_usize = channels as usize;
        let frames = samples.len() / channels_usize.max(1);
        let length = frames as f32 / rate as f32;
        if let Some([_, end]) = self.trim {
            // A hundredth of a second covers rounding at the very end of the file.
            if end > length + 0.01 {
                return Err(AudioError::Decode(format!(
                    "trim ends at {end:.2} s but the audio is {length:.2} s long"
                )));
            }
        }
        let (start, end) = match self.trim {
            // Rounded like loop points and streams, so a loop ending at the trim end fits.
            Some([start, end]) => (
                ((start * rate as f32).round() as usize).min(frames),
                ((end * rate as f32).round() as usize).min(frames),
            ),
            None => (0, frames),
        };
        if start >= end {
            return Err(AudioError::Decode("trim keeps no audio".into()));
        }
        let shape = self.shape(rate, Some(end - start));
        let kept = samples[start * channels_usize..end * channels_usize]
            .iter()
            .enumerate()
            .map(|(i, sample)| sample * shape.at(i / channels_usize))
            .collect();
        let clip = Clip::new(kept, channels, rate)?;
        match self.loop_points {
            Some([start, end]) => clip.with_loop(start, end),
            None => Ok(clip),
        }
    }

    /// Plays a streamed clip through once, off any frame path, and reports its frames,
    /// rate and peak exactly as the game would hear it. Streams are always stereo.
    pub fn scan_stream(self: &Arc<Self>) -> Result<StreamScan, AudioError> {
        use super::stream::Next;
        let mut reader = super::stream::open(self.clone(), false)?;
        let (mut frames, mut peak, mut blocks) = (0usize, 0.0f32, Vec::new());
        loop {
            match reader.next_frame() {
                Next::Frame([left, right]) => {
                    let level = left.abs().max(right.abs());
                    if frames % SCAN_BLOCK == 0 {
                        blocks.push(0.0);
                    }
                    if let Some(block) = blocks.last_mut() {
                        *block = f32::max(*block, level);
                    }
                    frames += 1;
                    peak = peak.max(level);
                }
                Next::Starved => std::thread::sleep(std::time::Duration::from_millis(1)),
                Next::Ended => break,
            }
        }
        let Some(rate) = reader.rate().filter(|_| frames > 0) else {
            return Err(AudioError::Decode("no audio decoded".into()));
        };
        // As for decoded clips: a trim past the end of the audio is refused, not shortened.
        if let Some([start, end]) = self.trim {
            let length = start + frames as f32 / rate as f32;
            if end > length + 0.01 {
                return Err(AudioError::Decode(format!(
                    "trim ends at {end:.2} s but the audio is {length:.2} s long"
                )));
            }
        }
        Ok(StreamScan {
            frames,
            rate,
            peak,
            blocks,
        })
    }

    /// Whether this clip plays by streaming instead of decoding whole.
    pub fn streams(&self) -> bool {
        self.stream
    }

    /// Decoder-thread body for a streamed clip: same trim, gain, fades and loop points as
    /// [`decode`](Self::decode), delivered as stereo chunks. Returns when the stream ends
    /// or the mixer drops its end.
    #[cfg(feature = "audio")]
    pub(super) fn stream_into(
        &self,
        looping: bool,
        chunks: &std::sync::mpsc::SyncSender<super::stream::Chunk>,
        recycled: &std::sync::mpsc::Receiver<Vec<f32>>,
    ) -> Result<(), AudioError> {
        use super::stream::{Chunk, CHUNK_FRAMES};
        let fresh = || {
            recycled.try_recv().map_or_else(
                |_| Vec::with_capacity(CHUNK_FRAMES * 2),
                |mut chunk| {
                    chunk.clear();
                    chunk
                },
            )
        };
        let mut first = true;
        loop {
            // Each pass reopens the file; the first plays from the start, later ones from the loop start.
            let mut reader = PcmReader::open(&self.file)?;
            let (mut chunk, mut emitted, mut source_frame) = (fresh(), 0usize, 0usize);
            let total = reader.frames;
            let mut plan: Option<Pass> = None;
            'decode: while let Some((packet, channels, rate)) = reader.next()? {
                let pass = plan.get_or_insert_with(|| {
                    let to = |seconds: f32| (seconds * rate as f32).round() as usize;
                    let (trim_start, trim_end) = match self.trim {
                        Some([start, end]) => (to(start), Some(to(end))),
                        None => (0, total),
                    };
                    let trimmed = trim_end.map(|end| end.saturating_sub(trim_start));
                    let from = if first {
                        0
                    } else {
                        self.loop_points.map_or(0, |[start, _]| to(start))
                    };
                    let until = if looping {
                        self.loop_points.map(|[_, end]| to(end)).or(trimmed)
                    } else {
                        trimmed
                    };
                    Pass {
                        rate,
                        trim_start,
                        shape: self.shape(rate, trimmed),
                        from,
                        until,
                    }
                });
                for frame in packet.chunks_exact(channels) {
                    let current = source_frame;
                    source_frame += 1;
                    let Some(at) = current.checked_sub(pass.trim_start) else {
                        continue;
                    };
                    if at < pass.from {
                        continue;
                    }
                    if pass.until.is_some_and(|until| at >= until) {
                        break 'decode;
                    }
                    let gain = pass.shape.at(at);
                    chunk.push(frame[0] * gain);
                    chunk.push(frame[channels - 1] * gain);
                    emitted += 1;
                    if chunk.len() >= CHUNK_FRAMES * 2 {
                        let full = std::mem::replace(&mut chunk, fresh());
                        if chunks
                            .send(Chunk {
                                samples: full,
                                rate,
                                end: false,
                            })
                            .is_err()
                        {
                            return Ok(());
                        }
                    }
                }
            }
            first = false;
            // A pass that plays nothing would loop forever, so it ends the stream.
            let last = !looping || emitted == 0;
            let rate = plan.map_or(1, |pass| pass.rate);
            if (!chunk.is_empty() || last)
                && chunks
                    .send(Chunk {
                        samples: chunk,
                        rate,
                        end: last,
                    })
                    .is_err()
            {
                return Ok(());
            }
            if last {
                return Ok(());
            }
        }
    }
}

/// Frames per entry in [`StreamScan::blocks`].
pub const SCAN_BLOCK: usize = 256;

/// What [`AudioClipAsset::scan_stream`] heard.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamScan {
    pub frames: usize,
    pub rate: u32,
    pub peak: f32,
    /// Peak level of each run of [`SCAN_BLOCK`] frames, for waveform outlines.
    pub blocks: Vec<f32>,
}

/// One streaming pass over the file: the trimmed frames `[from, until)` it plays.
#[cfg(feature = "audio")]
struct Pass {
    rate: u32,
    trim_start: usize,
    shape: Shape,
    from: usize,
    until: Option<usize>,
}

/// One decoded packet: interleaved samples, channel count and sample rate.
#[cfg(feature = "audio")]
type Packet<'a> = (&'a [f32], usize, u32);

/// Gain × fade-in × fade-out per trimmed frame.
struct Shape {
    gain: f32,
    fade_in: usize,
    fade_out: usize,
    frames: Option<usize>,
}

impl Shape {
    fn at(&self, frame: usize) -> f32 {
        let ramp_in = if frame < self.fade_in {
            frame as f32 / self.fade_in as f32
        } else {
            1.0
        };
        // Without a known length (a stream with no trim and no frame count) there is no fade out.
        let ramp_out = match self.frames {
            Some(frames) if frames > frame && frames - frame <= self.fade_out => {
                (frames - frame - 1) as f32 / self.fade_out as f32
            }
            _ => 1.0,
        };
        self.gain * ramp_in * ramp_out
    }
}

/// Packet-by-packet PCM from a file, interleaved `f32`, mono or stereo.
#[cfg(feature = "audio")]
struct PcmReader {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::audio::AudioDecoder>,
    track_id: u32,
    /// Playable frames, when the container states it.
    frames: Option<usize>,
    spec: Option<(usize, u32)>,
    samples: Vec<f32>,
}

#[cfg(feature = "audio")]
impl PcmReader {
    fn open(path: &std::path::Path) -> Result<Self, AudioError> {
        use symphonia::core::{
            codecs::audio::AudioDecoderOptions,
            formats::{probe::Hint, FormatOptions, TrackType},
            io::MediaSourceStream,
            meta::MetadataOptions,
        };
        let fail = |e: &dyn std::fmt::Display| AudioError::Decode(e.to_string());
        let file = std::fs::File::open(path).map_err(|e| fail(&e))?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(extension);
        }
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| fail(&e))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or(AudioError::Decode("no audio track".into()))?;
        let (track_id, frames) = (track.id, track.num_frames.map(|n| n as usize));
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or(AudioError::Decode("unsupported codec".into()))?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| fail(&e))?;
        Ok(Self {
            format,
            decoder,
            track_id,
            frames,
            spec: None,
            samples: Vec::new(),
        })
    }

    /// The next packet's samples with its channel count and rate, or `None` at the end.
    fn next(&mut self) -> Result<Option<Packet<'_>>, AudioError> {
        use symphonia::core::errors::Error;
        let fail = |e: &dyn std::fmt::Display| AudioError::Decode(e.to_string());
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) | Err(Error::ResetRequired) => return Ok(None),
                Err(e) => return Err(fail(&e)),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let buffer = match self.decoder.decode(&packet) {
                Ok(buffer) => buffer,
                // A corrupt packet is skipped, as players do.
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(fail(&e)),
            };
            let channels = buffer.spec().channels().count();
            if !(1..=2).contains(&channels) {
                return Err(AudioError::Decode(format!(
                    "{channels} channels; clips must be mono or stereo"
                )));
            }
            let spec = (channels, buffer.spec().rate());
            if self.spec.is_some_and(|known| known != spec) {
                return Err(AudioError::Decode("format changes mid-stream".into()));
            }
            self.spec = Some(spec);
            buffer.copy_to_vec_interleaved(&mut self.samples);
            return Ok(Some((&self.samples, spec.0, spec.1)));
        }
    }
}

#[cfg(feature = "audio")]
fn decode_file(path: &std::path::Path) -> Result<(Vec<f32>, u16, u32), AudioError> {
    let mut reader = PcmReader::open(path)?;
    let mut samples = Vec::new();
    while let Some((packet, channels, rate)) = reader.next()? {
        samples.extend_from_slice(packet);
        if samples.len() as f32 > super::AUDIO_MAX_CLIP_SECONDS * rate as f32 * channels as f32 {
            return Err(AudioError::InvalidClip(
                "clip is too long; mark it stream = true",
            ));
        }
    }
    let (channels, rate) = reader
        .spec
        .ok_or(AudioError::Decode("no audio decoded".into()))?;
    Ok((samples, channels as u16, rate))
}

#[cfg(not(feature = "audio"))]
fn decode_file(_path: &std::path::Path) -> Result<(Vec<f32>, u16, u32), AudioError> {
    Err(AudioError::Decode(
        "this build has no audio decoding; enable the engine's `audio` feature".into(),
    ))
}

/// Plays a clip from its entity. Added by scene loading for `emergent.audio_source`.
#[derive(Clone, Component, Debug)]
pub struct AudioEmitter {
    pub clip: Arc<AudioClipAsset>,
    pub settings: SoundSettings,
    /// Hash of the brain signal that (re)starts the clip, from this object or aimed at it.
    play_on: Option<u64>,
    /// Whether that signal was present last tick: a signal held over several ticks plays once.
    signalled: bool,
    requested: bool,
    stop_requested: bool,
    sound: Option<SoundId>,
}

impl AudioEmitter {
    pub fn new(clip: Arc<AudioClipAsset>, settings: SoundSettings, autoplay: bool) -> Self {
        Self {
            clip,
            settings,
            play_on: None,
            signalled: false,
            requested: autoplay,
            stop_requested: false,
            sound: None,
        }
    }

    /// Plays the clip each time a brain starts sending `signal` (from this object, or with
    /// this object as its target). An empty name clears it.
    pub fn with_play_on(mut self, signal: &str) -> Self {
        self.play_on = (!signal.is_empty()).then(|| signal_hash(signal));
        self
    }

    /// Starts (or restarts) the clip once it has loaded.
    pub fn play(&mut self) {
        self.requested = true;
        self.stop_requested = false;
    }

    pub fn stop(&mut self) {
        self.requested = false;
        self.stop_requested = true;
    }

    /// The playing sound, once started.
    pub fn sound(&self) -> Option<SoundId> {
        self.sound
    }
}

pub(super) fn signal_hash(name: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish()
}

/// Hears the scene from its entity. Without one, the active camera listens.
#[derive(Clone, Copy, Component, Debug, PartialEq)]
pub struct AudioListener {
    pub gain: f32,
    pub reverb: super::ReverbSettings,
    /// Music bus change in dB while speech plays; 0 turns ducking off.
    pub ducking_db: f32,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self {
            gain: 1.0,
            reverb: super::ReverbSettings::OFF,
            ducking_db: super::DEFAULT_DUCKING_DB,
        }
    }
}

pub(super) enum ClipLoad {
    Pending,
    Ready(ClipId),
    Failed,
}

pub(super) struct Loaded {
    pub(super) path: String,
    pub(super) result: Result<Clip, AudioError>,
}

pub(super) fn spawn_loader() -> Option<(
    std::sync::mpsc::Sender<Arc<AudioClipAsset>>,
    std::sync::mpsc::Receiver<Loaded>,
)> {
    let (requests, inbox) = std::sync::mpsc::channel::<Arc<AudioClipAsset>>();
    let (outbox, results) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("emergent-audio-loader".into())
        .spawn(move || {
            // Ends when the AudioWorld drops its sender.
            for asset in inbox {
                let result = asset.decode();
                if outbox
                    .send(Loaded {
                        path: asset.path.clone(),
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .ok()?;
    Some((requests, results))
}

/// Starts and stops emitters, fires signal-triggered clips and applies the listener's
/// settings. Runs each tick before the audio update.
pub(crate) fn sync_scene(world: &mut World, audio: &mut AudioWorld) {
    let camera = world.resource::<ActiveCamera>().map(|camera| camera.0);
    let mut chosen: Option<(EntityId, AudioListener)> = None;
    world.for_each_audio_listener(|id, listener| {
        if chosen.is_none() || Some(id) == camera {
            chosen = Some((id, *listener));
        }
    });
    let listener = chosen.map_or_else(AudioListener::default, |(_, listener)| listener);
    audio.set_listener(chosen.map(|(id, _)| id));
    audio.listener_gain = listener.gain;
    audio.set_reverb(listener.reverb);
    audio.ducking_db = listener.ducking_db;
    // Signals sent this tick, as (object, signal hash) for sender and target; buffer reused.
    let mut fired = std::mem::take(&mut audio.fired);
    fired.clear();
    for (signal, name) in world.signals() {
        let hash = signal_hash(name);
        fired.push((signal.from, hash));
        if let Some(target) = signal.target {
            fired.push((target, hash));
        }
    }
    world.for_each_audio_emitter(|id, emitter| {
        if let Some(signal) = emitter.play_on {
            let now = fired.contains(&(id, signal));
            if now && !emitter.signalled {
                emitter.play();
            }
            emitter.signalled = now;
        }
        if let Some(sound) = emitter.sound {
            if emitter.stop_requested || !audio.is_playing(sound) {
                let _ = audio.stop(sound);
                emitter.sound = None;
            }
        }
        emitter.stop_requested = false;
        if !emitter.requested {
            return;
        }
        let settings = SoundSettings {
            bus: emitter.clip.bus,
            ..emitter.settings
        };
        let started = if emitter.clip.streams() {
            audio.play_stream(&emitter.clip, PlayAt::Entity(id), settings)
        } else {
            let Some(clip) = audio.clip_for(&emitter.clip) else {
                return;
            };
            if let Some(sound) = emitter.sound.take() {
                let _ = audio.stop(sound);
            }
            audio.play(clip, PlayAt::Entity(id), settings)
        };
        emitter.requested = false;
        match started {
            Ok(sound) => {
                if let Some(previous) = emitter.sound.replace(sound) {
                    let _ = audio.stop(previous);
                }
            }
            Err(error) => eprintln!("audio: {}: {error}", emitter.clip.path),
        }
    });
    audio.fired = fired;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::{GroupNames, PublishedSignals, Signal};
    use crate::{AudioConfig, Vec3};

    fn emitter_with_clip(audio: &mut AudioWorld) -> AudioEmitter {
        let asset = Arc::new(AudioClipAsset {
            path: "assets/audio/shot.audio.toml".into(),
            id: "shot".into(),
            name: "Shot".into(),
            bus: Bus::Sfx,
            gain_db: 0.0,
            trim: None,
            loop_points: None,
            fade_in: 0.0,
            fade_out: 0.0,
            stream: false,
            file: PathBuf::new(),
        });
        // Seed the cache so no file is decoded.
        let id = audio.add_clip(Clip::new(vec![0.5; 4800], 1, 48_000).unwrap());
        audio
            .clip_cache
            .insert(asset.path.clone(), ClipLoad::Ready(id));
        let settings = SoundSettings {
            looping: false,
            ..Default::default()
        };
        AudioEmitter::new(asset, settings, false).with_play_on("fire")
    }

    fn send(world: &mut World, from: EntityId, target: Option<EntityId>, name: u32) {
        world.insert_resource(PublishedSignals(vec![Signal {
            from,
            name,
            target,
            position: Vec3::ZERO,
        }]));
    }

    #[test]
    fn signals_start_clips_once_per_signal_start() {
        let (mut audio, _mixer) = AudioWorld::new(AudioConfig::default()).unwrap();
        let mut world = World::new();
        let (gun, target, other) = (
            world.spawn("gun"),
            world.spawn("target"),
            world.spawn("other"),
        );
        world.insert_resource(GroupNames(vec!["fire".into(), "reload".into()]));
        let emitter = emitter_with_clip(&mut audio);
        world.insert_component(gun, emitter.clone()).unwrap();
        world.insert_component(target, emitter).unwrap();
        let sound = |world: &World, id| {
            world
                .component::<AudioEmitter>(id)
                .unwrap()
                .unwrap()
                .sound()
        };

        sync_scene(&mut world, &mut audio);
        assert_eq!(sound(&world, gun), None, "no autoplay");
        send(&mut world, other, None, 1);
        sync_scene(&mut world, &mut audio);
        assert_eq!(sound(&world, gun), None, "another signal name");
        send(&mut world, gun, Some(target), 0);
        sync_scene(&mut world, &mut audio);
        let first = sound(&world, gun).expect("the sender plays");
        assert!(sound(&world, target).is_some(), "the target plays too");
        // Held for another tick: no restart.
        sync_scene(&mut world, &mut audio);
        assert_eq!(sound(&world, gun), Some(first));
        // Released, then sent again: restarts.
        world.insert_resource(PublishedSignals(Vec::new()));
        sync_scene(&mut world, &mut audio);
        send(&mut world, gun, None, 0);
        sync_scene(&mut world, &mut audio);
        let second = sound(&world, gun).unwrap();
        assert_ne!(second, first);
        assert!(!audio.is_playing(first));
    }
}
