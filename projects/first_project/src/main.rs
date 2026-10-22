mod game;
mod generated;

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

    let mut engine = Engine::new(EngineConfig::default())?;
    let mut game = FirstProjectGame::new(speed);
    engine.run(&mut game)?;
    Ok(())
}
