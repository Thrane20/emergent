mod game;
mod generated;
mod visual;

use emergent::{Engine, EngineConfig, Project};

use crate::game::FirstProjectGame;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::load(env!("CARGO_MANIFEST_DIR"))?;
    let speed = project
        .read_text_asset("assets/data/leader_speed.txt")?
        .trim()
        .parse::<f32>()?;
    println!(
        "Validated project '{}' with {} enabled output(s)",
        project.manifest().name,
        project.enabled_outputs().count()
    );

    match std::env::args().nth(1).as_deref() {
        None => visual::run(speed, None),
        Some("--smoke") => visual::run(speed, Some(120)),
        Some("--headless") => run_headless(speed),
        Some(_) => Err("usage: emergent-first-project [--smoke | --headless]".into()),
    }
}

fn run_headless(speed: f32) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::new(EngineConfig::default())?;
    let mut game = FirstProjectGame::new(speed, Some(120));
    engine.run(&mut game)?;
    Ok(())
}
