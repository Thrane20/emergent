//! Prepared, immutable UI surfaces shared by screen overlays and world panels.
use crate::{
    ui::UiDefinition, Color, Geometry, Material, MaterialDefinition, MaterialShading, Mesh,
    MeshVertex, TextureImage, Vec2, Vec3,
};
use std::sync::Arc;

#[derive(Clone, Debug, crate::Component)]
pub struct UiSurface {
    pub definition: Arc<UiDefinition>,
    /// Screen mode ignores entity transforms; world mode follows them.
    pub screen: bool,
    /// Pixels in screen mode (zero fills the viewport), metres in world mode.
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}
impl UiSurface {
    pub fn validate(&self) -> Result<(), String> {
        if [self.width, self.height, self.x, self.y]
            .iter()
            .any(|v| !v.is_finite())
            || self.width < 0.0
            || self.height < 0.0
            || self.width > 1e6
            || self.height > 1e6
            || self.x.abs() > 1e6
            || self.y.abs() > 1e6
        {
            return Err("Invalid UI surface dimensions".into());
        }
        if u64::from(self.definition.canvas[0]) * u64::from(self.definition.canvas[1])
            > crate::texture::TEXTURE_MAX_PIXELS
        {
            return Err("Runtime UI surface exceeds 4,194,304 pixels".into());
        }
        Ok(())
    }
    pub fn prepare(&self) -> Result<(Geometry, Material), String> {
        self.validate()?;
        let [w, h] = self.definition.canvas;
        let image = rasterize(&self.definition)?;
        let material = Material::with_uv_image(
            MaterialDefinition {
                shading: MaterialShading::Unlit,
                cast_shadows: false,
                receive_shadows: false,
                ..Default::default()
            },
            image,
        )
        .map_err(|e| e.to_string())?;
        let width = if self.width > 0.0 { self.width } else { 2.0 };
        let height = if self.height > 0.0 {
            self.height
        } else {
            width * h as f32 / w as f32
        };
        let points = if self.screen {
            [[0., 0.], [1., 0.], [1., 1.], [0., 1.]]
        } else {
            [
                [-width / 2., height / 2.],
                [width / 2., height / 2.],
                [width / 2., -height / 2.],
                [-width / 2., -height / 2.],
            ]
        };
        let mut vertices = Vec::with_capacity(4);
        for (point, uv) in points
            .into_iter()
            .zip([[0., 0.], [1., 0.], [1., 1.], [0., 1.]])
        {
            vertices.push(
                MeshVertex::new(Vec3::new(point[0], point[1], 0.), Color::WHITE)
                    .map_err(|e| e.to_string())?
                    .with_uv(Vec2::from_array(uv))
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok((
            Geometry::Mesh(Mesh::new(vertices, vec![0, 1, 2, 0, 2, 3]).map_err(|e| e.to_string())?),
            material,
        ))
    }
}

fn number(fields: &toml::Table, key: &str, fallback: f32) -> f32 {
    fields
        .get(key)
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|n| n as f64)))
        .map_or(fallback, |n| n as f32)
}
fn color(fields: &toml::Table, key: &str, fallback: [f32; 3]) -> [u8; 4] {
    let mut rgb = fallback;
    if let Some(values) = fields.get(key).and_then(toml::Value::as_array) {
        for (out, v) in rgb.iter_mut().zip(values) {
            *out = v
                .as_float()
                .or_else(|| v.as_integer().map(|n| n as f64))
                .unwrap_or(0.) as f32;
        }
    }
    let convert = |n: f32| {
        ((if n <= 0.0031308 {
            12.92 * n
        } else {
            1.055 * n.powf(1. / 2.4) - 0.055
        })
        .clamp(0., 1.)
            * 255.)
            .round() as u8
    };
    [convert(rgb[0]), convert(rgb[1]), convert(rgb[2]), 255]
}
struct Canvas {
    w: u32,
    h: u32,
    pixels: Vec<u8>,
}
impl Canvas {
    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, c: [u8; 4]) {
        let left = x.max(0.).min(self.w as f32) as u32;
        let top = y.max(0.).min(self.h as f32) as u32;
        let right = (x + w).max(0.).min(self.w as f32) as u32;
        let bottom = (y + h).max(0.).min(self.h as f32) as u32;
        for row in top..bottom {
            for col in left..right {
                let at = ((row * self.w + col) * 4) as usize;
                self.pixels[at..at + 4].copy_from_slice(&c);
            }
        }
    }
    fn text(&mut self, text: &str, x: f32, y: f32, scale: f32, c: [u8; 4]) {
        let mut cursor = x;
        for ch in text.chars().take(64) {
            for (row, bits) in glyph(ch.to_ascii_uppercase()).iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.fill(
                            cursor + col as f32 * scale,
                            y + row as f32 * scale,
                            scale,
                            scale,
                            c,
                        );
                    }
                }
            }
            cursor += 6. * scale;
        }
    }
}
/// Rasterizes authored state during preparation, never during a render tick.
pub fn rasterize(ui: &UiDefinition) -> Result<TextureImage, String> {
    let [w, h] = ui.canvas;
    if u64::from(w) * u64::from(h) > crate::texture::TEXTURE_MAX_PIXELS {
        return Err("Runtime UI image exceeds pixel budget".into());
    }
    let mut canvas = Canvas {
        w,
        h,
        pixels: vec![0; (w * h * 4) as usize],
    };
    for (node, r) in ui.nodes.iter().zip(ui.layout()) {
        let f = &node.fields;
        let ink = color(f, "colour", [0.62, 0.68, 0.72]);
        let fill = color(
            f,
            "fill",
            if node.kind == "panel" {
                [0.012, 0.016, 0.024]
            } else {
                [0.05, 0.06, 0.07]
            },
        );
        let scale = number(f, "size", 2.);
        let mut label = f
            .get("text")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut tx = r.x + 6.;
        if matches!(node.kind, "panel" | "button" | "textbox") || f.contains_key("fill") {
            canvas.fill(r.x, r.y, r.w, r.h, fill);
        }
        if node.kind == "panel" {
            let b = number(f, "border_width", 0.);
            let c = color(f, "border", [0.25, 0.3, 0.33]);
            canvas.fill(r.x, r.y, r.w, b, c);
            canvas.fill(r.x, r.y + r.h - b, r.w, b, c);
            canvas.fill(r.x, r.y, b, r.h, c);
            canvas.fill(r.x + r.w - b, r.y, b, r.h, c);
        }
        if node.kind == "divider" {
            canvas.fill(r.x, r.y, r.w, number(f, "thickness", 2.), ink);
        }
        if node.kind == "toggle" {
            let side = (r.h - 6.).clamp(0., 14.);
            canvas.fill(r.x + 2., r.y + 3., side, side, ink);
            if f.get("checked").and_then(toml::Value::as_bool) != Some(true) {
                canvas.fill(
                    r.x + 4.,
                    r.y + 5.,
                    (side - 4.).max(0.),
                    (side - 4.).max(0.),
                    [0, 0, 0, 255],
                );
            }
            tx += side + 4.;
        }
        if node.kind == "slider" {
            let min = number(f, "min", 0.);
            let max = number(f, "max", 1.);
            let t = if max > min {
                ((number(f, "default", 0.5) - min) / (max - min)).clamp(0., 1.)
            } else {
                0.
            };
            canvas.fill(r.x, r.y + r.h - 4., r.w * t, 3., ink);
        }
        if node.kind == "stepper" {
            let options = f.get("options").and_then(toml::Value::as_str).unwrap_or("");
            let choice = options
                .split(',')
                .nth(number(f, "index", 0.) as usize)
                .unwrap_or("");
            label = format!("{label} < {choice} >");
        }
        if node.kind == "label" {
            let tw = label.chars().count() as f32 * 6. * scale;
            tx = match f.get("align").and_then(toml::Value::as_str) {
                Some("center") => r.x + (r.w - tw) / 2.,
                Some("right") => r.x + r.w - tw,
                _ => r.x,
            };
        }
        if node.kind == "button" && f.get("selected").and_then(toml::Value::as_bool) == Some(true) {
            canvas.fill(r.x, r.y, r.w, r.h, ink);
            canvas.text(&label, tx, r.y + (r.h - 7. * scale) / 2., scale, fill);
        } else {
            canvas.text(&label, tx, r.y + (r.h - 7. * scale) / 2., scale, ink);
        }
    }
    TextureImage::from_rgba(w, h, canvas.pixels).map_err(|e| e.to_string())
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        ' ' => [0; 7],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '<' | '◂' => [2, 4, 8, 16, 8, 4, 2],
        '>' | '▸' => [8, 4, 2, 1, 2, 4, 8],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}
