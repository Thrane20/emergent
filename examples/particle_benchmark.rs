//! Simulation-only timings; this does not measure GPU rendering or overdraw.
use emergent::{
    Mat4, ParticleBlend, ParticleDefinition, ParticleEmitter, ParticleStore, PARTICLE_STEP,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for count in [1000u32, 10000] {
        let emitters = (0..3)
            .map(|i| {
                let cap = count / 3 + u32::from(i == 0) * (count % 3);
                ParticleEmitter {
                    name: format!("Layer {i}"),
                    max_particles: cap,
                    burst: cap,
                    rate: 0.,
                    duration: 120.,
                    lifetime: [120.; 2],
                    blend: if i == 0 {
                        ParticleBlend::Alpha
                    } else {
                        ParticleBlend::Additive
                    },
                    ..Default::default()
                }
            })
            .collect();
        let effect = ParticleDefinition {
            schema_version: 1,
            id: "benchmark".into(),
            name: "Three emitter workload".into(),
            emitters,
        }
        .procedural()?;
        let mut store = ParticleStore::default();
        store.insert(effect, Mat4::IDENTITY)?;
        store.step(PARTICLE_STEP)?;
        assert_eq!(store.live_count(), count as usize);
        let mut times = Vec::with_capacity(600);
        for _ in 0..600 {
            let start = std::time::Instant::now();
            store.step(PARTICLE_STEP)?;
            times.push(start.elapsed().as_secs_f64() * 1000.);
            std::hint::black_box(&store);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{count} live / 3 emitters / 600 steps: CPU p50 {:.4} ms, p95 {:.4} ms, max {:.4} ms",
            times[299], times[569], times[599]
        );
    }
    Ok(())
}
