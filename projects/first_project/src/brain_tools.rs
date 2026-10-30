//! IDE tools for the Brain editor: validate an unsaved brain and simulate it for the preview.
use std::error::Error;
use std::io::Read;

use emergent::brain::{simulate, PreviewConfig};
use emergent::{BrainDefinition, BrainSettings, Project};

fn candidate(
    project: &Project,
    path: &str,
    file: Option<&str>,
) -> Result<BrainDefinition, Box<dyn Error>> {
    match file {
        Some(file) => {
            let mut source = String::new();
            std::fs::File::open(file)?
                .take(128 * 1024 + 1)
                .read_to_string(&mut source)?;
            Ok(BrainDefinition::from_source(project, path, &source)?)
        }
        None => Ok(BrainDefinition::load(project, path)?),
    }
}

/// `--validate-brain <path> [--candidate <file>]`
pub fn validate(project: &Project, args: &[String]) -> Result<(), Box<dyn Error>> {
    let (path, file) = match args {
        [path] => (path, None),
        [path, flag, file] if flag == "--candidate" => (path, Some(file.as_str())),
        _ => return Err("usage: --validate-brain <path> [--candidate <file>]".into()),
    };
    let brain = candidate(project, path, file)?;
    println!(
        "Brain '{}' validated: {} nodes",
        brain.name,
        brain.node_count()
    );
    Ok(())
}

/// `--simulate-brain <path> [--candidate <file>] [key=value ...]`; prints one line per frame.
pub fn preview(project: &Project, args: &[String]) -> Result<(), Box<dyn Error>> {
    let path = args
        .first()
        .ok_or("usage: --simulate-brain <path> [--candidate <file>] [key=value ...]")?;
    let (file, rest) = match args.get(1).map(String::as_str) {
        Some("--candidate") => (
            Some(args.get(2).ok_or("missing candidate file")?.as_str()),
            &args[3..],
        ),
        _ => (None, &args[1..]),
    };
    let brain = candidate(project, path, file)?;
    let mut config = PreviewConfig {
        agents: 20,
        agent_group: "agents".into(),
        targets: 1,
        target_group: "target".into(),
        target_speed: 3.0,
        ticks: 60 * 20,
        every: 2,
        seed: 1,
        settings: BrainSettings {
            max_speed: 3.0,
            acceleration: 8.0,
            planar: false,
            face_movement: true,
        },
    };
    for pair in rest {
        let (key, value) = pair.split_once('=').ok_or("arguments are key=value")?;
        let number = || {
            value
                .parse::<f32>()
                .map_err(|_| format!("{key} must be a number"))
        };
        match key {
            "agents" => config.agents = number()? as usize,
            "targets" => config.targets = number()? as usize,
            "target_speed" => config.target_speed = number()?,
            "seconds" => config.ticks = (number()?.clamp(0.1, 120.0) * 60.0) as u64,
            "seed" => config.seed = value.parse()?,
            "agent_group" => config.agent_group = value.into(),
            "target_group" => config.target_group = value.into(),
            "max_speed" => config.settings.max_speed = number()?,
            "acceleration" => config.settings.acceleration = number()?,
            "planar" => config.settings.planar = value == "true",
            _ => return Err(format!("unknown preview setting '{key}'").into()),
        }
    }
    let trace = simulate(&brain, &config)?;
    println!(
        "brain-preview 1 agents={} targets={} frame_seconds={}",
        trace.agents, trace.targets, trace.frame_seconds
    );
    let mut line = String::with_capacity(trace.agents * 30);
    for frame in &trace.frames {
        line.clear();
        for p in frame {
            use std::fmt::Write;
            let _ = write!(line, "{:.3} {:.3} {:.3} ", p.x, p.y, p.z);
        }
        println!("{}", line.trim_end());
    }
    Ok(())
}
