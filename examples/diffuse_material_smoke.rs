//! Native pixel regression for object textures, sRGB and terrain palette selection.
use emergent::*;
use std::{error::Error, io::Cursor, path::PathBuf};
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let fixture =
        Fixture(std::env::temp_dir().join(format!("emergent-diffuse-{}", std::process::id())));
    std::fs::create_dir_all(fixture.0.join("assets"))?;
    std::fs::create_dir_all(fixture.0.join("src/generated"))?;
    std::fs::write(fixture.0.join("emergent.toml"),"schema_version=1\nid='diffuse-smoke'\nname='Diffuse smoke'\nentry_scene='assets/main.scene.toml'\ngenerated_code='src/generated'\nasset_roots=['assets']\n[[outputs]]\nid='host'\nos='macos'\narchitecture='aarch64'\nrust_target='aarch64-apple-darwin'\nprofile='development'\n")?;
    let mut checker = image::RgbImage::new(32, 32);
    for (x, y, pixel) in checker.enumerate_pixels_mut() {
        *pixel = if (x / 4 + y / 4) % 2 == 0 {
            image::Rgb([240, 25, 20])
        } else {
            image::Rgb([20, 30, 240])
        };
    }
    checker.save(fixture.0.join("assets/checker.png"))?;
    let green = image::RgbImage::from_pixel(8, 8, image::Rgb([20, 230, 30]));
    green.save(fixture.0.join("assets/green.jpg"))?;
    image::RgbImage::from_pixel(4, 4, image::Rgb([128, 128, 128]))
        .save(fixture.0.join("assets/gray.png"))?;
    std::fs::write(
        fixture.0.join("assets/main.scene.toml"),
        "schema_version=1\nid='main'\nentities=[]\n",
    )?;
    let project = Project::load(&fixture.0)?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(256, 256))?;
    let base = MaterialDefinition {
        shading: MaterialShading::Unlit,
        albedo: Some([1.; 3]),
        ..Default::default()
    };
    let texture = |path: &str| {
        Some(MaterialTextureMap {
            asset: format!("assets/{path}"),
            scale: 1.,
        })
    };
    renderer.set_view(Vec3::new(3., 2., 4.), Vec3::ZERO, Vec3::Y)?;
    let mut render =
        |geometry: &Geometry, material: MaterialDefinition| -> Result<Vec<u8>, Box<dyn Error>> {
            let material = material.resolve(&project)?;
            let handle = renderer.insert_with_material(geometry, &material)?;
            let result =
                renderer.render_lit_rgba(&[GeometryInstance::new(handle, Mat4::IDENTITY)], &[])?;
            renderer.remove(handle);
            Ok(result.rgba)
        };
    for primitive in [
        Primitive::cube(1., Color::WHITE)?,
        Primitive::sphere(1., 32, 24, Color::WHITE)?,
    ] {
        let geometry = Geometry::from(primitive);
        let plain = render(&geometry, base.clone())?;
        let patterned = render(
            &geometry,
            MaterialDefinition {
                diffuse_map: texture("checker.png"),
                ..base.clone()
            },
        )?;
        assert!(
            plain.iter().zip(&patterned).filter(|(a, b)| a != b).count() > 1000,
            "diffuse image must change native object pixels"
        );
        let gray = render(
            &geometry,
            MaterialDefinition {
                diffuse_map: texture("gray.png"),
                ..base.clone()
            },
        )?;
        let center = (128 * 256 + 128) * 4;
        assert!(
            (gray[center] as i32 - 128).abs() <= 2,
            "sRGB round trip: {}",
            gray[center]
        );
    }
    let terrain = Geometry::Mesh(
        TerrainSettings {
            resolution: 32,
            size: 4.,
            height: 0.4,
            ..Default::default()
        }
        .generate()?
        .mesh(Color::WHITE)?,
    );
    let palette = vec![
        MaterialLayer {
            id: "rock".into(),
            name: "Rock".into(),
            diffuse_map: texture("checker.png"),
        },
        MaterialLayer {
            id: "grass".into(),
            name: "Grass".into(),
            diffuse_map: texture("green.jpg"),
        },
    ];
    let rock = render(
        &terrain,
        MaterialDefinition {
            terrain_layers: palette.clone(),
            active_layer: Some("rock".into()),
            ..base.clone()
        },
    )?;
    let grass = render(
        &terrain,
        MaterialDefinition {
            terrain_layers: palette,
            active_layer: Some("grass".into()),
            ..base
        },
    )?;
    assert!(
        rock.iter().zip(&grass).filter(|(a, b)| a != b).count() > 1000,
        "active terrain layer must change actual native pixels"
    );
    // Ensure invalid file data does not pass full native decoding.
    assert!(TextureImage::from_bytes(b"bad.png").is_err());
    let mut bytes = Cursor::new(Vec::new());
    checker.write_to(&mut bytes, image::ImageFormat::Png)?;
    assert_eq!(TextureImage::from_bytes(bytes.get_ref())?.width(), 32);
    println!(
        "Native diffuse smoke passed: cube, sphere, sRGB gray and two PNG/JPEG terrain layers."
    );
    Ok(())
}
