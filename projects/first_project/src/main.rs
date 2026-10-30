mod avoidance;
mod bounce;
mod game;
mod generated;
mod inspection;
mod inspection_tools;
mod scene_tools;
mod spin;
mod visual;

use emergent::{Engine, EngineConfig, Project};

use crate::game::FirstProjectGame;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::load(env!("CARGO_MANIFEST_DIR"))?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--scene-transforms") && args.len() == 2 {
        return scene_tools::report(&project, &args[1]);
    }
    if args.first().map(String::as_str) == Some("--convert-global")
        && (args.len() == 4 || args.len() == 5 && args[4] == "--position-only")
    {
        return scene_tools::convert(&project, &args[1], &args[2], &args[3], args.len() == 5);
    }
    if args.first().map(String::as_str) == Some("--validate-scenes")
        && args.len() > 1
        && args.len() <= 129
    {
        for path in &args[1..] {
            emergent::SceneDefinition::load(&project, path)?;
        }
        println!("Scene candidates validated");
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--inspect-scene") {
        if args.len() != 2 {
            return Err("usage: --inspect-scene <scene>".into());
        }
        return inspection::run(&project, &args[1]);
    }
    if args.first().map(String::as_str) == Some("--validate-scene") {
        if args.len() == 2 {
            emergent::SceneDefinition::load(&project, &args[1])?;
        } else if args.len() == 4 && args[2] == "--candidate" {
            use std::io::Read;
            let mut source = String::new();
            std::fs::File::open(&args[3])?
                .take(256 * 1024 + 1)
                .read_to_string(&mut source)?;
            emergent::SceneDefinition::from_source(&project, &args[1], &source)?;
        } else {
            return Err("usage: --validate-scene <scene> [--candidate <source>]".into());
        }
        println!("Scene candidate validated without construction or simulation");
        return Ok(());
    }
    let speed = project
        .read_text_asset("assets/data/leader_speed.txt")?
        .trim()
        .parse::<f32>()?;
    println!(
        "Validated project '{}' with {} enabled output(s)",
        project.manifest().name,
        project.enabled_outputs().count()
    );

    if args.first().map(String::as_str) == Some("--run-scene")
        && (args.len() == 2 || args.len() == 3 && args[2] == "--smoke")
    {
        return visual::run_scene(
            &project,
            speed,
            if args.len() == 3 { Some(120) } else { None },
            &args[1],
        );
    }
    match args.first().map(String::as_str) {
        None => visual::run(&project, speed, None),
        Some("--smoke") => visual::run(&project, speed, Some(120)),
        Some("--headless") => run_headless(&project, speed),
        Some("--preview-scene") if args.len() == 4 && args[2] == "--preview-output" => {
            visual::preview(
                &project,
                speed,
                &args[1],
                std::path::Path::new(&args[3]),
            )
        }
        Some(_) => Err("usage: emergent-first-project [--smoke | --headless | --preview-scene <scene> --preview-output <png>]".into()),
    }
}

fn run_headless(project: &Project, speed: f32) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::new(EngineConfig::default())?;
    let mut game = FirstProjectGame::new(
        &mut engine,
        project,
        project
            .manifest()
            .entry_scene
            .to_str()
            .ok_or("non-UTF8 scene path")?,
        speed,
        Some(120),
    )?;
    engine.run(&mut game)?;
    Ok(())
}
