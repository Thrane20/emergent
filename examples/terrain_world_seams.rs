//! Terrain world stage-1 check: a lit 2 × 2 chunk fixture with height-blended, triplanar
//! image layers must render like one merged mesh built from the same samples. A copy with
//! chunk-local (unoffset) projection shows the check detects texture seams.
use emergent::{
    height_band_weights, CameraProjection, ChunkCoord, Color, Geometry, GeometryInstance,
    GeometryRenderer, LightInstance, LightSource, Mat4, Material, MaterialDefinition, Mesh,
    MeshVertex, ShadowCasting, TerrainWorld, TextureImage, Vec3, TERRAIN_CHUNK_CELLS,
};
use std::path::PathBuf;

const BANDS: [f32; 3] = [35., 60., 90.];
const BLEND: f32 = 10.;

fn image(a: [u8; 3], b: [u8; 3]) -> Result<TextureImage, Box<dyn std::error::Error>> {
    let mut rgba = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64u32 {
        for x in 0..64u32 {
            // Diagonal stripes plus a checker: any projection offset error is visible.
            let c = if ((x + y) / 8 + (x / 16 + y / 16)) % 2 == 0 {
                a
            } else {
                b
            };
            rgba.extend([c[0], c[1], c[2], 255]);
        }
    }
    Ok(TextureImage::from_rgba(64, 64, rgba)?)
}

fn differing(a: &[u8], b: &[u8]) -> f64 {
    let pixels = a.len() / 4;
    let count = (0..pixels)
        .filter(|&i| (0..3).any(|c| a[i * 4 + c].abs_diff(b[i * 4 + c]) > 8))
        .count();
    count as f64 * 100. / pixels as f64
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/terrain-world-seams".into()),
    );
    std::fs::create_dir_all(&output)?;
    let world = TerrainWorld::default();
    let chunks =
        [(-1, -1), (0, -1), (-1, 0), (0, 0)].map(|(x, z)| world.prepare_chunk(ChunkCoord { x, z }));
    let layer = |albedo: [f32; 3], a, b| -> Result<Material, Box<dyn std::error::Error>> {
        Ok(Material::with_projected_image(
            MaterialDefinition {
                albedo: Some(albedo),
                // A 10m image period: 128m chunks are not a multiple, so seams would show.
                texture_tiling: [0.1, 0.1],
                ..Default::default()
            },
            image(a, b)?,
        )?)
    };
    let layers = vec![
        layer([0.6, 0.9, 0.5], [90, 140, 60], [60, 100, 40])?,
        layer([0.8, 0.7, 0.6], [130, 110, 90], [90, 80, 70])?,
        layer([1., 1., 1.], [240, 240, 245], [200, 205, 215])?,
    ];

    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(960, 540))?;
    renderer.set_projection(CameraProjection::perspective(45., 0.1, 5000.)?);
    // Low over the shared corner at the origin, so all four chunk borders are in view.
    renderer.set_view(Vec3::new(150., 170., 190.), Vec3::new(0., 30., 0.), Vec3::Y)?;
    let sun = [LightInstance::new(
        LightSource::directional(Color::WHITE, 1.)?.with_shadow_casting(ShadowCasting::Enabled)?,
        Mat4::from_rotation_x(-0.9) * Mat4::from_rotation_y(0.6),
    )];

    let (mut tiled, mut naive) = (Vec::new(), Vec::new());
    for chunk in &chunks {
        let chunk = chunk.as_ref().map_err(|e| e.to_string())?;
        let geometry = Geometry::from(chunk.mesh(Color::WHITE)?);
        let weights = height_band_weights(chunk.heights(), BANDS, BLEND, layers.len())?;
        let origin = chunk.origin();
        let place = Mat4::from_translation(Vec3::new(origin[0] as f32, 0., origin[1] as f32));
        for (offset, list) in [(origin, &mut tiled), ([0.; 2], &mut naive)] {
            let material =
                Material::terrain_blend(layers.clone(), chunk.samples(), weights.clone(), offset)?;
            list.push(GeometryInstance::new(
                renderer.insert_with_material(&geometry, &material)?,
                place,
            ));
        }
    }

    // One mesh over the same samples: chunk (cx, cz) owns local samples 0..=128.
    let cells = TERRAIN_CHUNK_CELLS as usize;
    let side = 2 * cells + 1;
    let (mut heights, mut vertices, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for z in 0..side {
        for x in 0..side {
            let (cx, cz) = ((x / cells).min(1), (z / cells).min(1));
            let chunk = chunks[cz * 2 + cx].as_ref().map_err(|e| e.to_string())?;
            let i = (z - cz * cells) * (cells + 1) + x - cx * cells;
            heights.push(chunk.heights()[i]);
            vertices.push(MeshVertex::with_normal(
                Vec3::new(
                    x as f32 * world.cell_size as f32,
                    chunk.heights()[i],
                    z as f32 * world.cell_size as f32,
                ),
                chunk.normals()[i],
                Color::WHITE,
            )?);
            if x < side - 1 && z < side - 1 {
                let (i, n) = ((z * side + x) as u32, side as u32);
                indices.extend([i, i + n, i + 1, i + 1, i + n, i + n + 1]);
            }
        }
    }
    let corner = chunks[0].as_ref().map_err(|e| e.to_string())?.origin();
    let merged_material = Material::terrain_blend(
        layers.clone(),
        side as u32,
        height_band_weights(&heights, BANDS, BLEND, layers.len())?,
        corner,
    )?;
    let merged = [GeometryInstance::new(
        renderer.insert_with_material(
            &Geometry::from(Mesh::new(vertices, indices)?),
            &merged_material,
        )?,
        Mat4::from_translation(Vec3::new(corner[0] as f32, 0., corner[1] as f32)),
    )];

    let reference = renderer.render_lit_rgba(&merged, &sun)?;
    let chunked = renderer.render_lit_rgba(&tiled, &sun)?;
    let unoffset = renderer.render_lit_rgba(&naive, &sun)?;
    renderer.set_mesh_lod(false);
    let chunked_full = renderer.render_lit_rgba(&tiled, &sun)?;
    let reference_full = renderer.render_lit_rgba(&merged, &sun)?;
    for (name, frame) in [
        ("merged", &reference),
        ("chunks", &chunked),
        ("chunks-unoffset", &unoffset),
    ] {
        image::save_buffer(
            output.join(format!("{name}.png")),
            &frame.rgba,
            frame.width,
            frame.height,
            image::ColorType::Rgba8,
        )?;
    }
    let lod = differing(&chunked.rgba, &reference.rgba);
    let full = differing(&chunked_full.rgba, &reference_full.rgba);
    let seams = differing(&unoffset.rgba, &reference.rgba);
    println!("2x2 chunks vs merged mesh, pixels differing by >8/255: {lod:.3}% (LOD), {full:.3}% (full detail)");
    println!("chunk-local projection vs merged (seam sensitivity): {seams:.3}%");
    println!("images: {}", output.display());
    assert!(
        lod < 0.05 && full < 0.05,
        "chunk fixture differs from merged mesh"
    );
    assert!(seams > 1., "unoffset projection should visibly differ");
    println!("PASS terrain world 2x2 fixture matches merged geometry, shading and materials");
    Ok(())
}
