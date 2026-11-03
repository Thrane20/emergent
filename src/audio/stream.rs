//! Streamed playback for long clips (music): a decoder thread prepares stereo chunks a
//! little ahead of the mixer, so a track never has to be decoded whole into memory.
// Without decoding nothing opens a stream, but the mixer still knows the type.
#![cfg_attr(not(feature = "audio"), allow(dead_code))]
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};

/// Stereo frames per chunk (~85 ms at 48 kHz).
pub(super) const CHUNK_FRAMES: usize = 4096;
/// Chunks buffered ahead of the mixer (~0.7 s at 48 kHz).
pub(super) const CHUNKS_AHEAD: usize = 8;

pub(super) struct Chunk {
    /// Interleaved stereo.
    pub(super) samples: Vec<f32>,
    pub(super) rate: u32,
    /// The last chunk of a stream that does not loop (or failed).
    pub(super) end: bool,
}

pub(super) enum Next {
    Frame([f32; 2]),
    /// No chunk ready yet; the decoder is behind.
    Starved,
    Ended,
}

/// The mixer's end of a stream. Used chunks go back to the decoder for reuse, so the audio
/// thread never frees them.
pub struct StreamReader {
    chunks: Receiver<Chunk>,
    recycle: SyncSender<Vec<f32>>,
    current: Vec<f32>,
    position: usize,
    rate: Option<u32>,
    ended: bool,
}

impl StreamReader {
    pub(super) fn new(chunks: Receiver<Chunk>, recycle: SyncSender<Vec<f32>>) -> Self {
        Self {
            chunks,
            recycle,
            current: Vec::new(),
            position: 0,
            rate: None,
            ended: false,
        }
    }

    /// Sample rate of the stream, once its first chunk has arrived.
    pub(super) fn rate(&self) -> Option<u32> {
        self.rate
    }

    pub(super) fn next_frame(&mut self) -> Next {
        while self.position >= self.current.len() {
            if self.ended {
                return Next::Ended;
            }
            match self.chunks.try_recv() {
                Ok(chunk) => {
                    let used = std::mem::replace(&mut self.current, chunk.samples);
                    // shortcut: a full recycle queue frees the chunk here; it holds at most CHUNKS_AHEAD.
                    let _ = self.recycle.try_send(used);
                    self.position = 0;
                    self.rate = Some(chunk.rate);
                    self.ended = chunk.end;
                }
                Err(TryRecvError::Empty) => return Next::Starved,
                Err(TryRecvError::Disconnected) => {
                    self.ended = true;
                    return Next::Ended;
                }
            }
        }
        let frame = [self.current[self.position], self.current[self.position + 1]];
        self.position += 2;
        Next::Frame(frame)
    }
}

/// Starts a decoder thread for `asset` and returns the mixer's end of it.
#[cfg(feature = "audio")]
pub(super) fn open(
    asset: std::sync::Arc<super::AudioClipAsset>,
    looping: bool,
) -> Result<StreamReader, super::AudioError> {
    use std::sync::mpsc::sync_channel;
    let (chunks, receiver) = sync_channel::<Chunk>(CHUNKS_AHEAD);
    let (recycle, recycled) = sync_channel::<Vec<f32>>(CHUNKS_AHEAD + 2);
    std::thread::Builder::new()
        .name("emergent-audio-stream".into())
        .spawn(move || {
            if let Err(error) = asset.stream_into(looping, &chunks, &recycled) {
                eprintln!("audio: {}: {error}", asset.path);
                let _ = chunks.send(Chunk {
                    samples: Vec::new(),
                    rate: 1,
                    end: true,
                });
            }
        })
        .map_err(|e| super::AudioError::Decode(e.to_string()))?;
    Ok(StreamReader::new(receiver, recycle))
}

#[cfg(not(feature = "audio"))]
pub(super) fn open(
    _asset: std::sync::Arc<super::AudioClipAsset>,
    _looping: bool,
) -> Result<StreamReader, super::AudioError> {
    Err(super::AudioError::Decode(
        "this build has no audio decoding; enable the engine's `audio` feature".into(),
    ))
}
