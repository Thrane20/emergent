//! Optimized authoring workload: LowMountain-like recipe, no frame-rate claim.
use emergent::{Color, TerrainPreset, TerrainSettings};
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resolution: u32 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "256".into())
        .parse()?;
    let settings = TerrainSettings {
        resolution,
        preset: TerrainPreset::Island,
        seed: 190,
        size: 270.,
        height: 56.,
        frequency: 0.8,
        octaves: 5,
        roughness: 0.5,
        ..Default::default()
    };
    for trial in 0..3 {
        let start = Instant::now();
        let field = settings.generate()?;
        let generated = start.elapsed();
        let start = Instant::now();
        let mesh = field.mesh(Color::WHITE)?;
        let prepared = start.elapsed();
        println!("resolution={resolution} trial={trial} generation_ms={:.3} mesh_ms={:.3} heights={} vertices={} triangles={}",generated.as_secs_f64()*1000.,prepared.as_secs_f64()*1000.,field.values().len(),mesh.vertices().len(),mesh.indices().len()/3);
    }
    Ok(())
}
