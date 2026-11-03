//! Device output through `cpal`: opens the default output and runs the [`Mixer`] in its
//! callback.
use super::{AudioConfig, AudioWorld, Mixer};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

/// Frames mixed per pass; device buffers larger than this are filled in several passes.
const BLOCK_FRAMES: usize = 4096;

/// Keeps the device stream open; audio stops when this is dropped.
pub struct AudioOutput {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Opens the default output device. `config.device_rate` is replaced by the device's rate.
pub fn open(config: AudioConfig) -> Result<(AudioWorld, AudioOutput), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no audio output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let stream_config: StreamConfig = supported.into();
    let (sample_rate, channels) = (stream_config.sample_rate, stream_config.channels);
    let (audio, mixer) = AudioWorld::new(AudioConfig {
        device_rate: sample_rate,
        ..config
    })
    .map_err(|e| e.to_string())?;
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, stream_config, mixer),
        SampleFormat::I16 => build::<i16>(&device, stream_config, mixer),
        SampleFormat::U16 => build::<u16>(&device, stream_config, mixer),
        SampleFormat::I32 => build::<i32>(&device, stream_config, mixer),
        other => Err(format!("unsupported output sample format {other}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((
        audio,
        AudioOutput {
            _stream: stream,
            sample_rate,
            channels,
        },
    ))
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: StreamConfig,
    mut mixer: Mixer,
) -> Result<cpal::Stream, String> {
    let channels = config.channels.max(1) as usize;
    // Allocated once here; the callback only reuses it.
    let mut stereo = vec![0.0f32; BLOCK_FRAMES * 2];
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                for chunk in data.chunks_mut(BLOCK_FRAMES * channels) {
                    let frames = chunk.len() / channels;
                    let mixed = &mut stereo[..frames * 2];
                    mixer.render(mixed);
                    for (frame, out) in chunk.chunks_mut(channels).enumerate() {
                        let (left, right) = (mixed[frame * 2], mixed[frame * 2 + 1]);
                        for (channel, sample) in out.iter_mut().enumerate() {
                            let value = match (channels, channel) {
                                (1, _) => (left + right) * 0.5,
                                (_, 0) => left,
                                (_, 1) => right,
                                _ => 0.0,
                            };
                            // shortcut: hard clip; add a limiter when mixes regularly exceed 0 dBFS.
                            *sample = T::from_sample(value.clamp(-1.0, 1.0));
                        }
                    }
                }
            },
            |error| eprintln!("audio output: {error}"),
            None,
        )
        .map_err(|e| e.to_string())
}
