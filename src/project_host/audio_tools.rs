//! `--inspect-audio`: validates a `.audio.toml` (saved or candidate) and decodes it, so
//! the IDE reports what the engine will actually play.
use std::error::Error;
use std::io::Read;

use crate::{AudioClipAsset, Project};

/// Points in the waveform outline the IDE draws on library tiles.
const WAVEFORM_POINTS: usize = 64;

/// Peak level per point, 0..1 to three decimals: the loudest block in each slice of the clip.
fn outline(blocks: &[f32], points: usize) -> Vec<toml::Value> {
    (0..points)
        .map(|i| {
            let start = i * blocks.len() / points;
            let end = ((i + 1) * blocks.len() / points)
                .max(start + 1)
                .min(blocks.len());
            let level = blocks
                .get(start..end)
                .map_or(0.0, |slice| slice.iter().fold(0.0f32, |a, b| a.max(*b)));
            toml::Value::Float((f64::from(level.min(1.0)) * 1000.0).round() / 1000.0)
        })
        .collect()
}

pub(super) fn inspect(project: &Project, args: &[String]) -> Result<(), Box<dyn Error>> {
    let asset = match args {
        [path] => AudioClipAsset::load(project, path)?,
        [path, flag, file] if flag == "--candidate" => {
            let mut source = String::new();
            std::fs::File::open(file)?
                .take(64 * 1024 + 1)
                .read_to_string(&mut source)?;
            AudioClipAsset::from_source(project, path, &source)?
        }
        _ => return Err("usage: --inspect-audio <path> [--candidate <file>]".into()),
    };
    // Streamed clips are checked through the streaming path, which has no length limit.
    let asset = std::sync::Arc::new(asset);
    let (channels, rate, frames, peak, blocks) = if asset.streams() {
        let scan = asset.scan_stream()?;
        (2, scan.rate, scan.frames, scan.peak, scan.blocks)
    } else {
        let clip = asset.decode()?;
        let channels = usize::from(clip.channels());
        let blocks = clip
            .samples()
            .chunks(crate::audio::SCAN_BLOCK * channels)
            .map(|block| block.iter().fold(0.0f32, |peak, s| peak.max(s.abs())))
            .collect();
        (
            clip.channels(),
            clip.sample_rate(),
            clip.frames(),
            clip.peak(),
            blocks,
        )
    };
    let mut summary = toml::Table::new();
    summary.insert("id".into(), asset.id.clone().into());
    summary.insert("name".into(), asset.name.clone().into());
    summary.insert("channels".into(), i64::from(channels).into());
    summary.insert("sample_rate".into(), i64::from(rate).into());
    summary.insert("frames".into(), (frames as i64).into());
    summary.insert(
        "duration_seconds".into(),
        (frames as f64 / f64::from(rate)).into(),
    );
    summary.insert("peak".into(), f64::from(peak).into());
    summary.insert("waveform".into(), outline(&blocks, WAVEFORM_POINTS).into());
    summary.insert("stream".into(), asset.streams().into());
    print!("{}", toml::to_string(&summary)?);
    Ok(())
}
