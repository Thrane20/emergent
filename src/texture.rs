//! Bounded sRGB colour images prepared before GPU upload.
use crate::MaterialError;
use image::ImageDecoder;
use std::{
    io::{Cursor, Read},
    path::Path,
    sync::Arc,
};
pub const TEXTURE_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const TEXTURE_MAX_PIXELS: u64 = 4_194_304;
#[derive(Clone, Debug)]
pub struct TextureImage {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}
fn reader(bytes: &[u8]) -> Result<image::ImageReader<Cursor<&[u8]>>, MaterialError> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| MaterialError(e.to_string()))?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(MaterialError("Diffuse images must be PNG or JPEG.".into()));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader)
}
impl TextureImage {
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, MaterialError> {
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || u64::from(width) * u64::from(height) > TEXTURE_MAX_PIXELS
            || rgba.len() != width as usize * height as usize * 4
        {
            return Err(MaterialError(
                "Invalid RGBA image dimensions or byte count".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            rgba: rgba.into(),
        })
    }

    pub fn dimensions_from_bytes(bytes: &[u8]) -> Result<(u32, u32), MaterialError> {
        if bytes.is_empty() || bytes.len() > TEXTURE_MAX_BYTES {
            return Err(MaterialError(
                "Diffuse image exceeds 16 MiB or is empty.".into(),
            ));
        }
        let (w, h) = reader(bytes)?
            .into_dimensions()
            .map_err(|e| MaterialError(e.to_string()))?;
        if w == 0
            || h == 0
            || w > 4096
            || h > 4096
            || u64::from(w) * u64::from(h) > TEXTURE_MAX_PIXELS
        {
            return Err(MaterialError(
                "Diffuse images need at most 4096 pixels per axis and 4,194,304 pixels total."
                    .into(),
            ));
        }
        Ok((w, h))
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MaterialError> {
        Self::dimensions_from_bytes(bytes)?;
        let mut decoder = reader(bytes)?
            .into_decoder()
            .map_err(|e| MaterialError(e.to_string()))?;
        let orientation = decoder
            .orientation()
            .map_err(|e| MaterialError(e.to_string()))?;
        let mut image =
            image::DynamicImage::from_decoder(decoder).map_err(|e| MaterialError(e.to_string()))?;
        image.apply_orientation(orientation);
        let rgba = image.to_rgba8();
        Ok(Self {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw().into(),
        })
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, MaterialError> {
        Self::from_bytes(&Self::read(path)?)
    }
    pub(crate) fn read(path: impl AsRef<Path>) -> Result<Vec<u8>, MaterialError> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|file| {
                file.take(TEXTURE_MAX_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
            })
            .map_err(|e| MaterialError(e.to_string()))?;
        if bytes.len() > TEXTURE_MAX_BYTES {
            return Err(MaterialError("Diffuse image exceeds 16 MiB.".into()));
        }
        Ok(bytes)
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    #[cfg(feature = "graphics")]
    pub(crate) fn resized(&self, width: u32, height: u32) -> Self {
        let source = image::RgbaImage::from_raw(self.width, self.height, self.rgba.to_vec())
            .expect("validated image dimensions");
        let rgba = image::imageops::resize(
            &source,
            width,
            height,
            image::imageops::FilterType::Triangle,
        );
        Self {
            width,
            height,
            rgba: rgba.into_raw().into(),
        }
    }
    #[cfg(feature = "graphics")]
    pub(crate) fn mip_levels(&self) -> Vec<Self> {
        let linear: [f32; 256] = std::array::from_fn(|i| {
            let n = i as f32 / 255.;
            if n <= 0.04045 {
                n / 12.92
            } else {
                ((n + 0.055) / 1.055).powf(2.4)
            }
        });
        let mut levels = vec![self.clone()];
        while levels
            .last()
            .is_some_and(|level| level.width > 1 || level.height > 1)
        {
            let src = levels.last().expect("base level exists");
            let w = (src.width / 2).max(1);
            let h = (src.height / 2).max(1);
            let mut pixels = vec![0; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let mut sums = [0.; 4];
                    let mut count = 0.;
                    for yy in y * src.height / h..(y + 1) * src.height / h {
                        for xx in x * src.width / w..(x + 1) * src.width / w {
                            let index = ((yy * src.width + xx) * 4) as usize;
                            for c in 0..3 {
                                sums[c] += linear[src.rgba[index + c] as usize];
                            }
                            sums[3] += src.rgba[index + 3] as f32 / 255.;
                            count += 1.;
                        }
                    }
                    for c in 0..4 {
                        let n = sums[c] / count;
                        let n = if c == 3 || n <= 0.0031308 {
                            if c == 3 {
                                n
                            } else {
                                12.92 * n
                            }
                        } else {
                            1.055 * n.powf(1. / 2.4) - 0.055
                        };
                        pixels[((y * w + x) * 4) as usize + c] =
                            (n * 255.).round().clamp(0., 255.) as u8;
                    }
                }
            }
            levels.push(Self {
                width: w,
                height: h,
                rgba: pixels.into(),
            });
        }
        levels
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn png(w: u32, h: u32, pixels: Vec<u8>) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image::RgbaImage::from_raw(w, h, pixels)
            .unwrap()
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }
    #[test]
    fn validates_formats_bounds_and_colour_bytes() {
        let bytes = png(1, 1, vec![128, 64, 32, 255]);
        let image = TextureImage::from_bytes(&bytes).unwrap();
        assert_eq!(image.rgba(), [128, 64, 32, 255]);
        assert!(TextureImage::from_bytes(b"not an image").is_err());
        assert!(TextureImage::from_bytes(&bytes[..20]).is_err());
        assert!(TextureImage::from_bytes(&vec![0; TEXTURE_MAX_BYTES + 1]).is_err());
        let wide = png(4097, 1, vec![0; 4097 * 4]);
        assert!(TextureImage::from_bytes(&wide).is_err());
    }
    #[test]
    #[cfg(feature = "graphics")]
    fn mipmaps_average_colour_in_linear_light() {
        let image = TextureImage::from_bytes(&png(
            3,
            1,
            vec![0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        ))
        .unwrap();
        let mips = image.mip_levels();
        assert_eq!(mips.len(), 2);
        assert_eq!((mips[1].width(), mips[1].height()), (1, 1));
        assert!((mips[1].rgba()[0] as i32 - 213).abs() <= 1);
    }
}
