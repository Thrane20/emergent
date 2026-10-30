//! Bounded, headless tween validation and sample bridge for editor parity checks.
use emergent::TweenDefinition;
use std::io::{Read, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    std::io::stdin().take(8193).read_to_string(&mut source)?;
    let definition = TweenDefinition::from_source(&source)?;
    let tween = definition.sampler()?;
    let samples = (0..=200)
        .map(|i| tween.sample(i as f64 / 100. * definition.tween.duration_seconds as f64))
        .collect::<Result<Vec<_>, _>>()?;
    #[derive(serde::Serialize)]
    struct Report {
        samples: Vec<f32>,
    }
    std::io::stdout().write_all(toml::to_string(&Report { samples })?.as_bytes())?;
    Ok(())
}
