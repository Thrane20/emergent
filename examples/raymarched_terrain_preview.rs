//! Bounded native editor preview. PNG on stdout; diagnostics on stderr.
use emergent::{CameraProjection, GeometryRenderer, RaymarchedTerrain, Vec3};
use serde::Deserialize;
use std::io::{Read, Write};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    settings: RaymarchedTerrain,
    yaw: f32,
    pitch: f32,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().take(4097).read_to_string(&mut input)?;
    if input.len() > 4096 {
        return Err("Preview request exceeds 4 KiB".into());
    }
    let request: Request = toml::from_str(&input)?;
    request.settings.validate()?;
    if !request.yaw.is_finite()
        || request.yaw.abs() > 360.
        || !request.pitch.is_finite()
        || !(5. ..=85.).contains(&request.pitch)
    {
        return Err("Invalid preview camera".into());
    }
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(640, 480))?;
    let id = renderer.insert_raymarched_terrain(request.settings)?;
    renderer.transform_raymarched_terrain(id, emergent::Mat4::IDENTITY)?;
    renderer.set_projection(CameraProjection::perspective(
        45.,
        0.1,
        (request.settings.size * 8.).max(1000.),
    )?);
    let yaw = request.yaw.to_radians();
    let pitch = request.pitch.to_radians();
    let target = Vec3::new(
        0.,
        if request.settings.version == 1 {
            request.settings.size * 0.03
        } else {
            request.settings.relief * 0.3
        },
        0.,
    );
    renderer.set_view(
        target
            + Vec3::new(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                yaw.cos() * pitch.cos(),
            ) * request.settings.size
                * 1.6,
        target,
        Vec3::Y,
    )?;
    let frame = renderer.render_lit_rgba(&[], &[])?;
    let mut png = Vec::new();
    image::ImageEncoder::write_image(
        image::codecs::png::PngEncoder::new(&mut png),
        &frame.rgba,
        frame.width,
        frame.height,
        image::ExtendedColorType::Rgba8,
    )?;
    std::io::stdout().write_all(&png)?;
    Ok(())
}
