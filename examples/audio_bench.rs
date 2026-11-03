//! Audio hot paths: engine-side update with 64 moving sources, and one 512-frame
//! mixer block with 32 real voices (plus virtual ones). Run with --release.
use emergent::{
    ActiveCamera, AudioConfig, AudioWorld, Clip, PlayAt, SoundSettings, Transform, Vec3, World,
};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut audio, mut mixer) = AudioWorld::new(AudioConfig::default())?;
    let mut world = World::new();
    let listener = world.spawn("listener");
    world.insert_resource(ActiveCamera(listener));
    let samples = (0..48_000).map(|i| (i as f32 * 0.0577).sin()).collect();
    let clip = audio.add_clip(Clip::new(samples, 1, 48_000)?.with_loop(0.0, 1.0)?);
    let sources: Vec<_> = (0..64)
        .map(|i| world.spawn(format!("source {i}")))
        .collect();
    let settings = SoundSettings {
        looping: true,
        max_distance: 200.0,
        ..Default::default()
    };
    for &source in &sources {
        audio.play(clip, PlayAt::Entity(source), settings)?;
    }
    let dt = Duration::from_millis(10);
    let mut out = vec![0.0; 1024];
    if std::env::args().any(|arg| arg == "--reverb") {
        audio.set_reverb(emergent::ReverbSettings {
            room: 0.7,
            damping: 0.5,
        });
    }
    let (warmup, samples) = (200, 2000);
    let (mut update, mut render) = (Vec::with_capacity(samples), Vec::with_capacity(samples));
    for tick in 0..warmup + samples {
        for (i, &source) in sources.iter().enumerate() {
            let angle = tick as f32 * 0.01 + i as f32;
            let position = Vec3::new(angle.cos(), 0.0, angle.sin()) * (5.0 + i as f32);
            world.set_local_transform(source, Transform::from_translation(position))?;
        }
        world.propagate_transforms();
        let start = Instant::now();
        audio.update(&world, dt);
        let updated = start.elapsed();
        let start = Instant::now();
        mixer.render(&mut out);
        let rendered = start.elapsed();
        if tick >= warmup {
            update.push(updated);
            render.push(rendered);
        }
    }
    let report = |name: &str, times: &mut Vec<Duration>| {
        times.sort();
        let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize].as_secs_f64() * 1e6;
        println!(
            "{name}: p50={:.1}us p99={:.1}us max={:.1}us",
            at(0.5),
            at(0.99),
            at(1.0)
        );
    };
    println!(
        "sources=64, real_voices=32, block=512 frames @48 kHz (budget 10667us), reverb={}, samples={samples}",
        std::env::args().any(|arg| arg == "--reverb")
    );
    report("update", &mut update);
    report("render", &mut render);
    Ok(())
}
