//! Native RGBA terrain blend regression, using temporary material assets only.
use emergent::{
    Color, GeometryInstance, GeometryRenderer, Mat4, MaterialDefinition, MaterialShading, Project,
    TerrainPaint, TerrainSettings, Vec3,
};
use std::{fs, path::PathBuf};
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = Temporary(std::env::temp_dir().join(format!(
        "emergent-terrain-material-smoke-{}",
        std::process::id()
    )));
    fs::create_dir_all(temporary.0.join("assets"))?;
    fs::create_dir_all(temporary.0.join("src/generated"))?;
    fs::write(temporary.0.join("emergent.toml"),"schema_version=1\nid='paint-smoke'\nname='Paint smoke'\nentry_scene='assets/main.scene.toml'\ngenerated_code='src/generated'\nasset_roots=['assets']\n[[outputs]]\nid='host'\nos='macos'\narchitecture='aarch64'\nrust_target='aarch64-apple-darwin'\nprofile='development'\n")?;
    for (index, albedo) in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [1., 1., 1.]]
        .into_iter()
        .enumerate()
    {
        let definition = MaterialDefinition {
            shading: MaterialShading::Unlit,
            albedo: Some(albedo),
            ..Default::default()
        };
        fs::write(
            temporary.0.join(format!("assets/{index}.material.toml")),
            format!(
                "schema_version=1\nid='m{index}'\nname='Material {index}'\n[material]\n{}",
                toml::to_string(&definition)?
            ),
        )?;
    }
    fs::write(
        temporary.0.join("assets/main.scene.toml"),
        "schema_version=1\nid='main'\nentities=[]\n",
    )?;
    image::save_buffer(
        temporary.0.join("assets/red.png"),
        &[255, 0, 0, 255, 255, 0, 0, 255],
        2,
        1,
        image::ColorType::Rgba8,
    )?;
    fs::write(temporary.0.join("assets/0.material.toml"),"schema_version=1\nid='red'\nname='Textured red'\n[material]\nshading='unlit'\nalbedo=[1.0,1.0,1.0]\n[material.diffuse_map]\nasset='assets/red.png'\nscale=2.0\n")?;
    let project = Project::load(&temporary.0)?;
    let resolution: u32 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "32".into())
        .parse()?;
    let n = resolution as usize;
    let settings = TerrainSettings {
        resolution,
        size: 4.,
        height: 1.,
        ..Default::default()
    };
    let field = emergent::TerrainField::from_heights(settings, vec![0.; n * n])?;
    let geometry = emergent::Geometry::from(field.mesh(Color::WHITE)?);
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(384, 384))?;
    renderer.set_view(Vec3::new(0., 5., 0.1), Vec3::ZERO, Vec3::Z)?;
    let mut paint = TerrainPaint {
        schema_version: 1,
        resolution,
        materials: (0..4)
            .map(|i| format!("assets/{i}.material.toml"))
            .collect(),
        transitions: [0.1, 0.5, 0.8],
        blend: 0.1,
        weights: vec![0; n * n * 4],
    };
    let mut capture = |paint: &TerrainPaint| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        fs::write(
            temporary.0.join("assets/surface.paint.toml"),
            toml::to_string(paint)?,
        )?;
        let material = MaterialDefinition {
            terrain_paint: Some("assets/surface.paint.toml".into()),
            ..Default::default()
        }
        .resolve(&project)?;
        let handle = renderer.insert_with_material(&geometry, &material)?;
        let image =
            renderer.render_lit_rgba(&[GeometryInstance::new(handle, Mat4::IDENTITY)], &[])?;
        renderer.remove(handle);
        Ok(image.rgba)
    };
    for pixel in paint.weights.as_chunks_mut::<4>().0 {
        pixel[0] = 255;
    }
    let red = capture(&paint)?;
    for pixel in paint.weights.as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&[0, 255, 0, 0]);
    }
    let green = capture(&paint)?;
    assert!(
        red.iter().zip(&green).filter(|(a, b)| a != b).count() > 1000,
        "material selection did not change the frame"
    );
    for z in 0..n {
        for x in 0..n {
            let i = (z * n + x) * 4;
            let slot = usize::from(x >= n / 2) + usize::from(z >= n / 2) * 2;
            paint.weights[i..i + 4].fill(0);
            paint.weights[i + slot] = 255;
        }
    }
    let split = capture(&paint)?;
    let mut counts = [0; 4];
    for p in split.as_chunks::<4>().0 {
        if p[0] > 220 && p[1] < 20 && p[2] < 20 {
            counts[0] += 1;
        }
        if p[1] > 220 && p[0] < 20 && p[2] < 20 {
            counts[1] += 1;
        }
        if p[2] > 220 && p[0] < 20 && p[1] < 20 {
            counts[2] += 1;
        }
        if p[0] > 220 && p[1] > 220 && p[2] > 220 {
            counts[3] += 1;
        }
    }
    assert!(
        counts.iter().all(|n| *n > 1000),
        "missing painted material regions: {counts:?}"
    );
    for p in paint.weights.as_chunks_mut::<4>().0 {
        p.copy_from_slice(&[128, 127, 0, 0]);
    }
    let blended = capture(&paint)?;
    assert!(
        blended
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 100 && p[1] > 100 && p[2] < 20)
            .count()
            > 1000,
        "soft material blend missing"
    );
    image::save_buffer(
        "/tmp/emergent-terrain-material-native.png",
        &split,
        384,
        384,
        image::ColorType::Rgba8,
    )?;
    println!("Terrain material smoke passed: four regions {counts:?}, soft blend, library references; {}",renderer.info().adapter_name);
    Ok(())
}
