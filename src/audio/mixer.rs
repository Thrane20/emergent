//! Audio-thread mixing: voices (decoded clips or streams), a send reverb and an output
//! limiter. Nothing here blocks, locks or allocates once the [`Mixer`] exists.
use super::stream::{Next, StreamReader};
use std::sync::{
    mpsc::{Receiver, SyncSender},
    Arc,
};

/// Frames mixed per pass; larger device buffers are filled in several passes.
pub(super) const MAX_FRAMES: usize = 4096;
/// Output peaks are held just under full scale.
const LIMIT: f32 = 0.98;

/// Parameters the mixer ramps to over its next block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VoiceParams {
    pub(super) gains: [f32; 2],
    /// Reverb send, 0..1.
    pub(super) send: f32,
    /// Pitch × Doppler; the source's own rate conversion is applied by the mixer.
    pub(super) rate: f64,
    pub(super) audible: bool,
}

impl VoiceParams {
    pub(super) const SILENT: Self = Self {
        gains: [0.0; 2],
        send: 0.0,
        rate: 0.0,
        audible: false,
    };
}

/// What a voice plays.
pub(super) enum Source {
    Clip {
        samples: Arc<[f32]>,
        channels: usize,
        sample_rate: u32,
        loop_frames: Option<(usize, usize)>,
    },
    Stream(Box<StreamReader>),
}

/// Room settings for the shared reverb. `room` 0 switches it off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReverbSettings {
    /// 0..1: small room to large hall.
    pub room: f32,
    /// 0..1: bright to muffled tail.
    pub damping: f32,
}

impl ReverbSettings {
    pub const OFF: Self = Self {
        room: 0.0,
        damping: 0.5,
    };
}

pub(super) enum Command {
    Play {
        slot: usize,
        serial: u32,
        source: Source,
        looping: bool,
    },
    Params {
        slot: usize,
        params: VoiceParams,
    },
    Stop {
        slot: usize,
    },
    Reverb(ReverbSettings),
}

/// A voice that ended. Its source travels back so clip memory and stream threads are
/// released on the engine thread, not the audio thread.
pub(super) struct Finished {
    pub(super) slot: usize,
    pub(super) serial: u32,
    pub(super) _source: Source,
}

struct Voice {
    serial: u32,
    source: Source,
    looping: bool,
    /// Clips: read position in frames. Streams: fraction between `frames[0]` and `frames[1]`.
    position: f64,
    /// Streams only: the two source frames being interpolated.
    frames: [[f32; 2]; 2],
    current: VoiceParams,
    target: VoiceParams,
    started: bool,
    stopping: bool,
}

/// Audio-thread mixer. Call [`render`](Self::render) from the device callback, or
/// directly for headless output.
pub struct Mixer {
    commands: Receiver<Command>,
    finished: SyncSender<Finished>,
    voices: Box<[Option<Voice>]>,
    device_rate: f64,
    send: Box<[f32]>,
    reverb: Reverb,
    limiter_gain: f32,
    limiter_release: f32,
}

impl Mixer {
    pub(super) fn new(
        commands: Receiver<Command>,
        finished: SyncSender<Finished>,
        voices: usize,
        device_rate: u32,
    ) -> Self {
        Self {
            commands,
            finished,
            voices: (0..voices).map(|_| None).collect(),
            device_rate: device_rate as f64,
            send: vec![0.0; MAX_FRAMES].into_boxed_slice(),
            reverb: Reverb::new(device_rate),
            limiter_gain: 1.0,
            // Recovers from a peak over about 100 ms.
            limiter_release: 1.0 - (-1.0 / (0.1 * device_rate as f32)).exp(),
        }
    }

    /// Fills interleaved stereo `out` (overwriting it). Never blocks or allocates.
    pub fn render(&mut self, out: &mut [f32]) {
        self.apply_commands();
        for block in out.chunks_mut(MAX_FRAMES * 2) {
            self.render_block(block);
        }
    }

    fn render_block(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = out.len() / 2;
        if frames == 0 {
            return;
        }
        self.send[..frames].fill(0.0);
        let step = 1.0 / frames as f32;
        for slot in 0..self.voices.len() {
            let Some(voice) = self.voices[slot].as_mut() else {
                continue;
            };
            if mix_voice(voice, out, &mut self.send[..frames], step, self.device_rate) {
                self.finish(slot);
            }
        }
        self.reverb.process(&self.send[..frames], out);
        for frame in out.as_chunks_mut::<2>().0 {
            let peak = frame[0].abs().max(frame[1].abs());
            if peak * self.limiter_gain > LIMIT {
                self.limiter_gain = LIMIT / peak;
            }
            frame[0] *= self.limiter_gain;
            frame[1] *= self.limiter_gain;
            self.limiter_gain += (1.0 - self.limiter_gain) * self.limiter_release;
        }
    }

    /// Voices currently held, mixed or virtual.
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|voice| voice.is_some()).count()
    }

    fn apply_commands(&mut self) {
        while let Ok(command) = self.commands.try_recv() {
            match command {
                Command::Play {
                    slot,
                    serial,
                    source,
                    looping,
                } => {
                    if slot >= self.voices.len() {
                        continue;
                    }
                    self.finish(slot);
                    self.voices[slot] = Some(Voice {
                        serial,
                        source,
                        looping,
                        position: 0.0,
                        frames: [[0.0; 2]; 2],
                        current: VoiceParams::SILENT,
                        target: VoiceParams::SILENT,
                        started: false,
                        stopping: false,
                    });
                }
                Command::Params { slot, params } => {
                    if let Some(Some(voice)) = self.voices.get_mut(slot) {
                        if !voice.stopping {
                            // The first update snaps, so a new sound starts at its level.
                            if !voice.started {
                                voice.current = params;
                                voice.started = true;
                            }
                            voice.target = params;
                        }
                    }
                }
                Command::Stop { slot } => {
                    if let Some(Some(voice)) = self.voices.get_mut(slot) {
                        voice.stopping = true;
                        voice.target.gains = [0.0; 2];
                        voice.target.send = 0.0;
                    }
                }
                Command::Reverb(settings) => self.reverb.set(settings),
            }
        }
    }

    fn finish(&mut self, slot: usize) {
        if let Some(voice) = self.voices[slot].take() {
            let done = Finished {
                slot,
                serial: voice.serial,
                _source: voice.source,
            };
            // shortcut: if the engine stops draining, the source is released here instead;
            // fine unless the clip was also removed, which makes this the last reference.
            let _ = self.finished.try_send(done);
        }
    }
}

/// Mixes one voice into `out` (and its reverb send into `send`), ramping gain, send and
/// rate across the block. Returns true when the voice has finished.
fn mix_voice(
    voice: &mut Voice,
    out: &mut [f32],
    send: &mut [f32],
    step: f32,
    device_rate: f64,
) -> bool {
    let (start, target) = (voice.current, voice.target);
    let audible = start.audible || target.audible;
    let frames = send.len();
    // Inaudible (virtual) voices still advance so they keep their place.
    let mut write = |frame: usize, t: f32, sample: [f32; 2]| {
        if audible {
            let gains = [0, 1].map(|c| lerp(start.gains[c], target.gains[c], t));
            out[frame * 2] += sample[0] * gains[0];
            out[frame * 2 + 1] += sample[1] * gains[1];
            send[frame] += (sample[0] + sample[1])
                * 0.5
                * lerp(start.send, target.send, t)
                * gains[0].max(gains[1]);
        }
    };
    let rate_at = |t: f32| start.rate + (target.rate - start.rate) * t as f64;
    let mut finished = false;
    match &mut voice.source {
        Source::Clip {
            samples,
            channels,
            sample_rate,
            loop_frames,
        } => {
            let channels = *channels;
            let total = samples.len() / channels;
            let (loop_start, loop_end) = loop_frames.unwrap_or((0, total));
            let native = *sample_rate as f64 / device_rate;
            let read = |frame: usize, channel: usize| {
                if frame < total {
                    samples[frame * channels + channel.min(channels - 1)]
                } else {
                    0.0
                }
            };
            for frame in 0..frames {
                let t = (frame + 1) as f32 * step;
                if audible {
                    let index = voice.position as usize;
                    let fraction = (voice.position - index as f64) as f32;
                    let next = if voice.looping && index + 1 >= loop_end {
                        loop_start
                    } else {
                        index + 1
                    };
                    write(
                        frame,
                        t,
                        [0, 1]
                            .map(|c| read(index, c) + (read(next, c) - read(index, c)) * fraction),
                    );
                }
                voice.position += rate_at(t) * native;
                if voice.looping {
                    while voice.position >= loop_end as f64 {
                        voice.position -= (loop_end - loop_start) as f64;
                    }
                } else if voice.position >= total as f64 {
                    finished = true;
                    break;
                }
            }
        }
        Source::Stream(reader) => {
            for frame in 0..frames {
                let t = (frame + 1) as f32 * step;
                let fraction = voice.position as f32;
                let [a, b] = voice.frames;
                write(frame, t, [0, 1].map(|c| a[c] + (b[c] - a[c]) * fraction));
                if let Some(rate) = reader.rate() {
                    voice.position += rate_at(t) * rate as f64 / device_rate;
                }
                // Before the first frame arrives `rate` is unknown, so this pulls frames until it is.
                while voice.position >= 1.0 || reader.rate().is_none() {
                    match reader.next_frame() {
                        Next::Frame(next) => {
                            voice.frames = [voice.frames[1], next];
                            if voice.position >= 1.0 {
                                voice.position -= 1.0;
                            }
                        }
                        // Underrun: hold position and play silence until data arrives.
                        Next::Starved => {
                            voice.frames = [[0.0; 2]; 2];
                            break;
                        }
                        Next::Ended => {
                            finished = true;
                            break;
                        }
                    }
                }
                if finished {
                    break;
                }
            }
        }
    }
    voice.current = target;
    finished || voice.stopping
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Freeverb-style reverb: eight parallel damped combs into four allpasses per channel.
/// Buffers are sized for the device rate when the mixer is built.
struct Reverb {
    combs: [[Comb; 8]; 2],
    allpasses: [[Allpass; 4]; 2],
    settings: ReverbSettings,
}

struct Comb {
    buffer: Box<[f32]>,
    index: usize,
    filter: f32,
}

struct Allpass {
    buffer: Box<[f32]>,
    index: usize,
}

impl Reverb {
    fn new(device_rate: u32) -> Self {
        // Freeverb's tunings at 44.1 kHz, with the right channel spread by 23 samples.
        const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
        let scale = |n: usize, spread: usize| {
            (((n + spread) as f64 * device_rate as f64 / 44_100.0) as usize).max(1)
        };
        let side = |spread: usize| {
            (
                COMBS.map(|n| Comb {
                    buffer: vec![0.0; scale(n, spread)].into_boxed_slice(),
                    index: 0,
                    filter: 0.0,
                }),
                ALLPASSES.map(|n| Allpass {
                    buffer: vec![0.0; scale(n, spread)].into_boxed_slice(),
                    index: 0,
                }),
            )
        };
        let (left, right) = (side(0), side(23));
        Self {
            combs: [left.0, right.0],
            allpasses: [left.1, right.1],
            settings: ReverbSettings::OFF,
        }
    }

    fn set(&mut self, settings: ReverbSettings) {
        self.settings = settings;
    }

    fn process(&mut self, send: &[f32], out: &mut [f32]) {
        if self.settings.room <= 0.0 {
            return;
        }
        let feedback = 0.7 + self.settings.room.clamp(0.0, 1.0) * 0.28;
        let damp = self.settings.damping.clamp(0.0, 1.0) * 0.4;
        for (frame, &input) in send.iter().enumerate() {
            let input = input * 0.015;
            for channel in 0..2 {
                let mut sum = 0.0;
                for comb in &mut self.combs[channel] {
                    let delayed = comb.buffer[comb.index];
                    comb.filter = delayed * (1.0 - damp) + comb.filter * damp;
                    comb.buffer[comb.index] = input + comb.filter * feedback;
                    comb.index = (comb.index + 1) % comb.buffer.len();
                    sum += delayed;
                }
                for allpass in &mut self.allpasses[channel] {
                    let delayed = allpass.buffer[allpass.index];
                    allpass.buffer[allpass.index] = sum + delayed * 0.5;
                    allpass.index = (allpass.index + 1) % allpass.buffer.len();
                    sum = delayed - sum;
                }
                // Freeverb's wet scale.
                out[frame * 2 + channel] += sum * 3.0;
            }
        }
    }
}
