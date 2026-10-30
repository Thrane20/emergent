//! Persistent, camera-only terrain authoring worker using the EMI1 inspection protocol.
use emergent::{
    CameraProjection, GeometryRenderer, RaymarchedRenderOptions, RaymarchedTerrain, Vec3,
};
use serde::Deserialize;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::sync::mpsc::{sync_channel, TryRecvError};
use std::time::{Duration, Instant};
#[derive(Deserialize)]
struct Envelope {
    request: Command,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Command {
    Preview {
        seq: u32,
        quality: PreviewQuality,
        step_heatmap: bool,
    },
    View {
        seq: u32,
        yaw: f32,
        pitch: f32,
        distance: f32,
        target: [f32; 3],
        projection: String,
        field_of_view_degrees: f32,
        view_size: f32,
    },
    Select {
        seq: u32,
        #[serde(rename = "space")]
        _space: String,
        #[serde(rename = "tool")]
        _tool: String,
        id: Option<String>,
    },
    Stop {
        seq: u32,
    },
}
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PreviewQuality {
    #[default]
    Auto,
    Full,
    Half,
}
impl PreviewQuality {
    fn dimensions(self, moving: bool) -> (u32, u32) {
        if matches!(self, Self::Half) || (matches!(self, Self::Auto) && moving) {
            (480, 270)
        } else {
            (960, 540)
        }
    }
}
fn packet(out: &mut impl Write, kind: u32, parts: &[&[u8]]) -> std::io::Result<()> {
    let size: usize = parts.iter().map(|p| p.len()).sum();
    if size > 4 * 1024 * 1024 {
        return Err(std::io::Error::other("Preview packet exceeds 4 MiB"));
    }
    out.write_all(b"EMI1")?;
    out.write_all(&kind.to_le_bytes())?;
    out.write_all(&(size as u32).to_le_bytes())?;
    for part in parts {
        out.write_all(part)?;
    }
    out.flush()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::env::args().nth(1).ok_or("Missing terrain recipe")?;
    if source.len() > 4096 {
        return Err("Recipe exceeds 4 KiB".into());
    }
    let field: RaymarchedTerrain = toml::from_str(&source)?;
    field.validate()?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(960, 540))?;
    renderer.insert_raymarched_terrain(field)?;
    let target = Vec3::new(0., field.relief * 0.3, 0.);
    renderer.set_view(
        target + Vec3::new(0.6, 0.5, 0.7).normalize() * field.size * 1.6,
        target,
        Vec3::Y,
    )?;
    let (sender, receiver) = sync_channel(8);
    std::thread::spawn(move || {
        let mut input = BufReader::new(std::io::stdin());
        loop {
            let mut line = Vec::new();
            match input.by_ref().take(4097).read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() <= 4096 => {
                    let value = std::str::from_utf8(&line)
                        .map_err(|e| e.to_string())
                        .and_then(|s| {
                            toml::from_str::<Envelope>(s)
                                .map(|e| e.request)
                                .map_err(|e| e.to_string())
                        });
                    if sender.send(value).is_err() {
                        break;
                    }
                }
                _ => {
                    let _ = sender.send(Err("Command exceeds 4 KiB".into()));
                    break;
                }
            }
        }
    });
    let mut out = BufWriter::new(std::io::stdout().lock());
    let mut sequence = 0u32;
    let mut command_sequence = 0u32;
    let mut png = Vec::with_capacity(512 * 1024);
    let mut quality = PreviewQuality::Auto;
    let mut last_view: Option<Instant> = None;
    let mut dimensions = (960, 540);
    loop {
        let deadline = Instant::now() + Duration::from_millis(33);
        for _ in 0..8 {
            let command = match receiver.try_recv() {
                Ok(value) => value,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            };
            let mut stop = false;
            let mut seq = 0;
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                match command.map_err(std::io::Error::other)? {
                    Command::Preview {
                        seq: s,
                        quality: value,
                        step_heatmap,
                    } => {
                        seq = s;
                        quality = value;
                        renderer.set_raymarched_render_options(RaymarchedRenderOptions {
                            step_heatmap,
                            ..Default::default()
                        });
                    }
                    Command::Stop { seq: s } => {
                        seq = s;
                        stop = true;
                    }
                    Command::Select { seq: s, id, .. } => {
                        seq = s;
                        if id.is_some() {
                            return Err("Terrain preview is camera-only".into());
                        }
                    }
                    Command::View {
                        seq: s,
                        yaw,
                        pitch,
                        distance,
                        target,
                        projection,
                        field_of_view_degrees,
                        view_size,
                    } => {
                        seq = s;
                        if !yaw.is_finite()
                            || yaw.abs() > 10000.
                            || !pitch.is_finite()
                            || pitch.abs() > 1.45
                            || !distance.is_finite()
                            || !(0.25..=30000.).contains(&distance)
                            || target.iter().any(|n| !n.is_finite() || n.abs() > 10000.)
                        {
                            return Err("Invalid preview view".into());
                        }
                        let far = (distance * 8.).max(1000.);
                        let projection = match projection.as_str() {
                            "perspective" => {
                                CameraProjection::perspective(field_of_view_degrees, 0.1, far)?
                            }
                            "isometric" => CameraProjection::isometric(view_size, 0.1, far)?,
                            _ => return Err("Invalid preview projection".into()),
                        };
                        let target = Vec3::from_array(target);
                        let eye = target
                            + Vec3::new(
                                yaw.sin() * pitch.cos(),
                                pitch.sin(),
                                yaw.cos() * pitch.cos(),
                            ) * distance;
                        renderer.set_camera(eye, target, Vec3::Y, projection)?;
                        last_view = Some(Instant::now());
                    }
                }
                Ok(())
            })();
            let mut ack = [0u8; 8];
            ack[..4].copy_from_slice(&seq.to_le_bytes());
            ack[4..].copy_from_slice(&u32::from(result.is_err()).to_le_bytes());
            let message = result
                .as_ref()
                .err()
                .map(ToString::to_string)
                .unwrap_or_default();
            packet(&mut out, 1, &[&ack, message.as_bytes()])?;
            if result.is_ok() {
                command_sequence = seq;
            }
            if stop {
                return Ok(());
            }
        }
        let desired = quality
            .dimensions(last_view.is_some_and(|at| at.elapsed() < Duration::from_millis(250)));
        if desired != dimensions {
            renderer.resize(winit::dpi::PhysicalSize::new(desired.0, desired.1));
            dimensions = desired;
        }
        let start = Instant::now();
        let frame = renderer.render_lit_rgba(&[], &[])?;
        let render_us = start.elapsed().as_micros().min(u32::MAX as u128) as u32;
        let start = Instant::now();
        // Keep the inspection envelope fixed so scene gizmo/picking clients are unchanged.
        let upscaled = if frame.width != 960 {
            let pixels = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                frame.width,
                frame.height,
                frame.rgba.as_slice(),
            )
            .ok_or("Invalid preview pixel buffer")?;
            Some(image::imageops::resize(
                &pixels,
                960,
                540,
                image::imageops::FilterType::Triangle,
            ))
        } else {
            None
        };
        png.clear();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new_with_quality(
                &mut png,
                image::codecs::png::CompressionType::Fast,
                image::codecs::png::FilterType::NoFilter,
            ),
            upscaled
                .as_ref()
                .map_or(frame.rgba.as_slice(), |image| image.as_raw().as_slice()),
            960,
            540,
            image::ExtendedColorType::Rgba8,
        )?;
        let encode_us = start.elapsed().as_micros().min(u32::MAX as u128) as u32;
        sequence = sequence.checked_add(1).ok_or("Frame sequence exhausted")?;
        let mut meta = [0u8; 36];
        for (offset, value) in [
            (0, sequence),
            (4, command_sequence),
            (16, render_us),
            (20, encode_us),
            (24, 960),
            (28, 540),
            (32, std::process::id()),
        ] {
            meta[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        packet(&mut out, 0, &[&meta, &png])?;
        std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_quality_preserves_full_resolution_when_settled() {
        assert_eq!(PreviewQuality::Auto.dimensions(true), (480, 270));
        assert_eq!(PreviewQuality::Auto.dimensions(false), (960, 540));
        assert_eq!(PreviewQuality::Full.dimensions(true), (960, 540));
        assert_eq!(PreviewQuality::Half.dimensions(false), (480, 270));
        assert!(toml::from_str::<Envelope>(
            "request={op='preview',seq=1,quality='invalid',step_heatmap=false}"
        )
        .is_err());
    }
}
