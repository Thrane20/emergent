//! IDE tools for the Locomotion editor: validate an unsaved profile and simulate it for the preview.
use std::error::Error;
use std::fmt::Write as _;
use std::io::Read;

use crate::locomotion::{simulate, Course};
use crate::{ComponentRegistry, LocomotionProfile, Project};

fn candidate(
    project: &Project,
    registry: &ComponentRegistry,
    path: &str,
    file: Option<&str>,
) -> Result<LocomotionProfile, Box<dyn Error>> {
    match file {
        Some(file) => {
            let mut source = String::new();
            std::fs::File::open(file)?
                .take(64 * 1024 + 1)
                .read_to_string(&mut source)?;
            Ok(LocomotionProfile::from_source_with(
                path, &source, registry,
            )?)
        }
        None => Ok(LocomotionProfile::load_with(project, path, registry)?),
    }
}

/// `--validate-locomotion <path> [--candidate <file>]`
pub fn validate(
    project: &Project,
    registry: &ComponentRegistry,
    args: &[String],
) -> Result<(), Box<dyn Error>> {
    let (path, file) = match args {
        [path] => (path, None),
        [path, flag, file] if flag == "--candidate" => (path, Some(file.as_str())),
        _ => return Err("usage: --validate-locomotion <path> [--candidate <file>]".into()),
    };
    let profile = candidate(project, registry, path, file)?;
    outln!(
        "Locomotion '{}' validated: {}",
        profile.name,
        profile.type_id
    );
    Ok(())
}

/// `--simulate-locomotion <path> [--candidate <file>] [course=waypoints|orbit|stop|launch|circuit] [seconds=N]`
pub fn preview(
    project: &Project,
    registry: &ComponentRegistry,
    args: &[String],
) -> Result<(), Box<dyn Error>> {
    let path = args
        .first()
        .ok_or("usage: --simulate-locomotion <path> [--candidate <file>] [course=…] [seconds=…]")?;
    let (file, rest) = match args.get(1).map(String::as_str) {
        Some("--candidate") => (
            Some(args.get(2).ok_or("missing candidate file")?.as_str()),
            &args[3..],
        ),
        _ => (None, &args[1..]),
    };
    let profile = candidate(project, registry, path, file)?;
    let (mut course, mut seconds) = (None, 20.0_f32);
    for pair in rest {
        match pair.split_once('=') {
            Some(("course", value)) => {
                course = Some(match value {
                    "waypoints" => Course::Waypoints,
                    "orbit" => Course::Orbit,
                    "stop" => Course::Stop,
                    "launch" => Course::Launch,
                    "circuit" => Course::Circuit,
                    other => return Err(format!("unknown course '{other}'").into()),
                })
            }
            Some(("seconds", value)) => seconds = value.parse()?,
            _ => return Err(format!("unknown preview setting '{pair}'").into()),
        }
    }
    let course = course.unwrap_or(match profile.type_id {
        "ballistic" | "rocket" => Course::Launch,
        "fixed_wing_physics" => Course::Circuit,
        _ => Course::Waypoints,
    });
    let trace = simulate(&profile, course, seconds)?;
    outln!(
        "locomotion-preview 1 type={} frames={} frame_seconds={}",
        profile.type_id,
        trace.frames.len(),
        trace.frame_seconds
    );
    let mut line = String::from("course");
    for p in &trace.course {
        let _ = write!(line, " {:.3} {:.3} {:.3}", p.x, p.y, p.z);
    }
    outln!("{line}");
    for (line_index, (p, a)) in trace.frames.iter().enumerate() {
        line.clear();
        let _ = write!(
            line,
            "{:.3} {:.3} {:.3} {:.4} {:.4} {:.4} {:.3} {:.4} {:.3} {:.3} {:.3} {:.3} {:.3} {}",
            p.x,
            p.y,
            p.z,
            a.yaw,
            a.pitch,
            a.bank,
            a.speed,
            a.steer,
            a.throttle,
            a.brake,
            a.rotor,
            a.thrust,
            a.fuel,
            u8::from(a.landed)
        );
        // Physical aircraft add their flight phase (see `FlightPhase`, in declaration order).
        if let Some(phase) = trace.phases.get(line_index) {
            let _ = write!(line, " {}", *phase as u8);
        }
        outln!("{line}");
    }
    Ok(())
}
