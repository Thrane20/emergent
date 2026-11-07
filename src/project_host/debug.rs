//! `--debug-scene <scene>`: the game running as in Run, rendered off-screen from its own camera
//! and streamed to the IDE's Debug editor, with pause, single-step, speed, forwarded keys and mouse
//! motion, the objects spawned since loading, and live
//! values (transform, velocity, brain modes and running nodes, locomotion and tower state) for one
//! selected object.
//!
//! Wire format: the inspection worker's `EMI1` packets. Kind 0 is a frame (36-byte header, then a
//! 960×540 PNG), kind 1 acknowledges a command, kind 2 is the live state as TOML, sent with every
//! third frame (about 10 Hz). Commands are one TOML line each: `request={seq=1, op="pause"}`.
use std::error::Error;
use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc::{sync_channel, TryRecvError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::inspection::{acknowledge, packet};
use super::{visual::ProjectGeometry, ProjectGame};
use crate::{Engine, EngineConfig, EntityId, EulerRot, GeometryRenderer, Key, KeySet};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    request: Command,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Command {
    Pause {
        seq: u32,
    },
    Resume {
        seq: u32,
    },
    /// Advance this many fixed ticks while paused (1–600).
    Step {
        seq: u32,
        ticks: u32,
    },
    /// Game time per real second (0.1–8).
    Speed {
        seq: u32,
        factor: f32,
    },
    /// The scene instance whose live values are reported; none clears it.
    Select {
        seq: u32,
        id: Option<String>,
    },
    /// Keys held in the IDE's game view.
    Keys {
        seq: u32,
        held: Vec<String>,
    },
    /// Relative pointer motion in pixels since the last report (mouse look).
    Mouse {
        seq: u32,
        dx: f32,
        dy: f32,
    },
    Stop {
        seq: u32,
    },
}
impl Command {
    fn seq(&self) -> u32 {
        match self {
            Self::Pause { seq }
            | Self::Resume { seq }
            | Self::Step { seq, .. }
            | Self::Speed { seq, .. }
            | Self::Select { seq, .. }
            | Self::Keys { seq, .. }
            | Self::Mouse { seq, .. }
            | Self::Stop { seq } => *seq,
        }
    }
}

fn key(name: &str) -> Option<Key> {
    Some(match name {
        "w" => Key::W,
        "a" => Key::A,
        "s" => Key::S,
        "d" => Key::D,
        "q" => Key::Q,
        "e" => Key::E,
        "space" => Key::Space,
        "shift" => Key::Shift,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        _ => return None,
    })
}

#[derive(Serialize)]
struct State {
    tick: u64,
    paused: bool,
    speed: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected: Option<Selected>,
    /// Objects created after the scene loaded (projectiles, spawned agents…), up to [`MAX_SPAWNED`].
    #[serde(skip_serializing_if = "Vec::is_empty")]
    spawned: Vec<Spawned>,
}

#[derive(Serialize)]
struct Spawned {
    /// Session-scoped label (`spawned-N`), never reused while the run lasts.
    id: String,
    name: String,
    /// A scene instance ID or another spawned label.
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
}
const MAX_SPAWNED: usize = 500;

/// Entities created after the scene loaded, labelled for the session (an explicit mapping; runtime
/// handles never leave the process). Deleted entities drop out; labels are never reused.
struct Spawns {
    initial: std::collections::HashSet<EntityId>,
    labels: std::collections::HashMap<EntityId, String>,
    next: u64,
}
impl Spawns {
    fn new(world: &crate::World) -> Self {
        Self {
            initial: world.entity_ids().collect(),
            labels: Default::default(),
            next: 0,
        }
    }
    fn find(&self, label: &str) -> Option<EntityId> {
        self.labels
            .iter()
            .find(|(_, l)| l.as_str() == label)
            .map(|(&e, _)| e)
    }
    /// Labels new entities (into `names` too, so flight orders name them) and lists them in `out`.
    fn refresh(
        &mut self,
        world: &crate::World,
        names: &mut std::collections::HashMap<EntityId, String>,
        out: &mut Vec<Spawned>,
    ) {
        self.labels.retain(|&entity, _| {
            let live = world.get(entity).is_ok();
            if !live {
                names.remove(&entity);
            }
            live
        });
        for entity in world.entity_ids() {
            if self.labels.len() >= MAX_SPAWNED {
                break;
            }
            if self.initial.contains(&entity) || self.labels.contains_key(&entity) {
                continue;
            }
            self.next += 1;
            let label = format!("spawned-{}", self.next);
            names.insert(entity, label.clone());
            self.labels.insert(entity, label);
        }
        out.clear();
        out.extend(self.labels.iter().map(|(&entity, label)| {
            Spawned {
                id: label.clone(),
                name: world.get(entity).map_or("", |e| e.name()).to_owned(),
                parent: world
                    .parent(entity)
                    .ok()
                    .flatten()
                    .and_then(|parent| names.get(&parent).cloned()),
            }
        }));
        // Creation order: spawned-9 before spawned-10.
        out.sort_by(|a, b| a.id.len().cmp(&b.id.len()).then_with(|| a.id.cmp(&b.id)));
    }
}

#[derive(Serialize, Default)]
struct Selected {
    id: String,
    /// False once the object has been deleted (its handle is stale).
    exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation_degrees: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scale: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    velocity: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    modes: Vec<crate::brain::ModeTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    intent: Option<IntentView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flight: Option<FlightView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tower: Option<TowerView>,
    /// Problems the engine otherwise handles silently (e.g. a landing target that isn't a runway).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct IntentView {
    velocity: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    altitude: Option<f32>,
    brake: bool,
    hover: bool,
    boost: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    flight_order: Option<String>,
}

#[derive(Serialize)]
struct FlightView {
    phase: String,
    airborne: bool,
    airspeed: f32,
    ground_speed: f32,
    heading: f32,
    altitude: f32,
    climb_rate: f32,
    throttle: f32,
    glide_error: f32,
    go_arounds: u32,
    landed: bool,
    stalled: bool,
}

#[derive(Serialize)]
struct TowerView {
    stage: String,
    goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    number: Option<u32>,
}

/// Rounded for display and a compact payload.
fn round(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}
fn round3(v: crate::Vec3) -> [f32; 3] {
    [round(v.x), round(v.y), round(v.z)]
}

/// A flight order with objects named by scene instance ID instead of runtime handles.
fn order(
    goal: crate::fixed_wing::FlightGoal,
    names: &std::collections::HashMap<EntityId, String>,
) -> String {
    use crate::fixed_wing::FlightGoal as G;
    let name = |id: EntityId| {
        names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| "(not in the scene)".into())
    };
    match goal {
        G::Land {
            zone,
            pattern_height,
        } => format!("Land on {} (pattern {pattern_height} m)", name(zone)),
        G::Arrive { airport } => format!("Arrive at {}", name(airport)),
        G::Depart { airport, altitude } => format!("Depart {} to {altitude} m", name(airport)),
        other => format!("{other:?}"),
    }
}

fn selected(
    engine: &Engine,
    id: &str,
    entity: Option<EntityId>,
    names: &std::collections::HashMap<EntityId, String>,
) -> Selected {
    let world = engine.world();
    let Some(entity) = entity.filter(|&e| world.global_transform(e).is_ok()) else {
        return Selected {
            id: id.into(),
            ..Selected::default()
        };
    };
    let transform = world.global_transform(entity).expect("checked above");
    let (scale, rotation, position) = transform.matrix().to_scale_rotation_translation();
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    #[cfg(feature = "physics")]
    let velocity = world.linear_velocity(entity).ok().flatten().map(round3);
    #[cfg(not(feature = "physics"))]
    let velocity = None;
    let agent = world
        .component::<crate::LocomotionAgent>(entity)
        .ok()
        .flatten();
    let mut warnings = Vec::new();
    if let Some(crate::fixed_wing::FlightGoal::Land { zone, .. }) =
        agent.and_then(|agent| agent.flight.or(agent.intent.flight))
    {
        if crate::tower::runway_geometry(world, zone, false).is_none() {
            warnings.push(
                "Land target is not a runway (a rectangle zone), so the aircraft holds its course. Target the runway zone, or use Arrive at airport.".into(),
            );
        }
    }
    Selected {
        warnings,
        id: id.into(),
        exists: true,
        position: Some(round3(position)),
        rotation_degrees: Some([
            round(x.to_degrees()),
            round(y.to_degrees()),
            round(z.to_degrees()),
        ]),
        scale: Some(round3(scale)),
        velocity,
        modes: world
            .component::<crate::BrainAgent>(entity)
            .ok()
            .flatten()
            .map(|brain| brain.trace())
            .unwrap_or_default(),
        intent: agent.map(|agent| {
            let intent = agent.intent;
            IntentView {
                velocity: round3(intent.velocity),
                target: intent.target.map(round3),
                altitude: intent.altitude.map(round),
                brake: intent.brake,
                hover: intent.hover,
                boost: intent.boost,
                flight_order: agent
                    .flight
                    .or(intent.flight)
                    .map(|goal| order(goal, names)),
            }
        }),
        flight: agent
            .and_then(|agent| agent.flight_telemetry())
            .map(|t| FlightView {
                phase: format!("{:?}", t.phase),
                airborne: t.airborne,
                airspeed: round(t.airspeed),
                ground_speed: round(t.ground_speed),
                heading: round(t.heading),
                altitude: round(t.altitude),
                climb_rate: round(t.climb_rate),
                throttle: round(t.throttle),
                glide_error: round(t.glide_error),
                go_arounds: t.go_arounds,
                landed: t.landed,
                stalled: t.stalled,
            }),
        tower: agent.and_then(|agent| agent.dispatch()).map(|d| TowerView {
            stage: format!("{:?}", d.stage),
            goal: order(d.goal, names),
            number: d.number.map(|n| n as u32),
        }),
    }
}

/// Runs the scene like Run, streamed to the IDE instead of a window.
pub fn run<G: ProjectGame>(
    project: &crate::Project,
    scene_path: &str,
) -> Result<(), Box<dyn Error>> {
    let config = EngineConfig::default();
    let fixed_timestep = config.fixed_timestep;
    let mut engine = Engine::new(config)?;
    let mut game = G::load(&mut engine, project, scene_path)?;
    // shortcut: PNG readback at up to 30 Hz like inspection; hardware video when measured scenes need it.
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(960, 540))?;
    let mut geometry = ProjectGeometry::upload(&mut renderer, &game, engine.world(), false)?;
    engine.start(&mut game)?;
    eprintln!(
        "Debug run: adapter={}, backend={}, instances={}",
        renderer.info().adapter_name,
        renderer.info().backend,
        game.loaded().renderables.len()
    );
    let (sender, receiver) = sync_channel::<Result<Command, String>>(16);
    std::thread::spawn(move || {
        let mut input = BufReader::new(std::io::stdin());
        let mut line = Vec::new();
        loop {
            line.clear();
            match (&mut input).take(4097).read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) if line.len() <= 4096 => {
                    let command = std::str::from_utf8(&line)
                        .map_err(|e| e.to_string())
                        .and_then(|text| {
                            toml::from_str::<Envelope>(text)
                                .map(|value| value.request)
                                .map_err(|e| e.to_string())
                        });
                    if sender.send(command).is_err() {
                        break;
                    }
                }
                _ => {
                    let _ = sender.send(Err("debug command exceeds 4096 bytes".into()));
                    break;
                }
            }
        }
    });
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    let (mut paused, mut speed) = (false, 1.0f32);
    let mut selection: Option<(String, Option<EntityId>)> = None;
    let mut names: std::collections::HashMap<EntityId, String> = game
        .loaded()
        .entities
        .iter()
        .map(|(id, &entity)| (entity, id.clone()))
        .collect();
    // Everything present after loading (model parts included) is the scene; later entities are spawned.
    let mut spawns = Spawns::new(engine.world());
    let mut spawned = Vec::new();
    let mut keys = KeySet::EMPTY;
    let mut frame_sequence = 0u32;
    let mut command_sequence = 0u32;
    let mut png = Vec::with_capacity(256 * 1024);
    let interval = Duration::from_secs_f64(1.0 / 30.0);
    let mut deadline = Instant::now();
    let mut previous = Instant::now();
    let result = (|| -> Result<(), Box<dyn Error>> {
        'session: loop {
            let mut step = 0u32;
            loop {
                let command = match receiver.try_recv() {
                    Ok(Ok(command)) => command,
                    Ok(Err(message)) => {
                        acknowledge(&mut output, 0, &Err(message.into()))?;
                        continue;
                    }
                    Err(TryRecvError::Disconnected) => break 'session,
                    Err(TryRecvError::Empty) => break,
                };
                let seq = command.seq();
                let outcome: Result<String, Box<dyn Error>> = match command {
                    Command::Pause { .. } => {
                        paused = true;
                        Ok(String::new())
                    }
                    Command::Resume { .. } => {
                        paused = false;
                        Ok(String::new())
                    }
                    Command::Step { ticks, .. } if (1..=600).contains(&ticks) => {
                        paused = true;
                        step += ticks;
                        Ok(String::new())
                    }
                    Command::Step { .. } => Err("step 1-600 ticks".into()),
                    Command::Speed { factor, .. } if (0.1..=8.0).contains(&factor) => {
                        speed = factor;
                        Ok(String::new())
                    }
                    Command::Speed { .. } => Err("speed must be 0.1-8".into()),
                    Command::Select { id: None, .. } => {
                        selection = None;
                        Ok(String::new())
                    }
                    Command::Select { id: Some(id), .. } => {
                        let entity = game
                            .loaded()
                            .entities
                            .get(&id)
                            .copied()
                            .or_else(|| spawns.find(&id));
                        match entity {
                            Some(entity) => {
                                selection = Some((id, Some(entity)));
                                Ok(String::new())
                            }
                            None => Err(format!("'{id}' is not in this scene").into()),
                        }
                    }
                    Command::Keys { held, .. } if held.len() <= 16 => {
                        keys = held
                            .iter()
                            .filter_map(|name| key(name))
                            .fold(KeySet::EMPTY, KeySet::with);
                        engine.set_keys(keys);
                        Ok(String::new())
                    }
                    Command::Keys { .. } => Err("too many keys".into()),
                    Command::Mouse { dx, dy, .. } => {
                        if engine.add_mouse_motion(crate::Vec2::new(dx, dy)) {
                            Ok(String::new())
                        } else {
                            Err("invalid mouse motion".into())
                        }
                    }
                    Command::Stop { .. } => {
                        acknowledge(&mut output, seq, &Ok(String::new()))?;
                        break 'session;
                    }
                };
                let ok = outcome.is_ok();
                acknowledge(&mut output, seq, &outcome)?;
                if ok {
                    command_sequence = seq;
                }
            }

            // Game time: real time × speed while running; exact ticks when stepping.
            let now = Instant::now();
            let elapsed = now.duration_since(previous);
            previous = now;
            if !paused {
                engine.advance_frame(&mut game, elapsed.mul_f32(speed))?;
            }
            for _ in 0..step {
                engine.advance_frame(&mut game, fixed_timestep)?;
            }
            if engine.exit_requested() {
                break 'session;
            }

            geometry.extract(&engine, &game, &mut renderer)?;
            super::visual::sync_camera(&mut renderer, engine.world(), game.loaded().active_camera)?;
            let render_start = Instant::now();
            geometry.extract_particles(&mut renderer, engine.world(), false)?;
            let frame = renderer.render_lit_rgba(&geometry.instances, &geometry.lights)?;
            let render_us = render_start.elapsed().as_micros().min(u32::MAX as u128) as u32;
            let encode_start = Instant::now();
            png.clear();
            {
                let mut encoder = png::Encoder::new(&mut png, frame.width, frame.height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.set_compression(png::Compression::Fastest);
                let mut writer = encoder.write_header()?;
                writer.write_image_data(&frame.rgba)?;
                writer.finish()?;
            }
            let encode_us = encode_start.elapsed().as_micros().min(u32::MAX as u128) as u32;
            frame_sequence = frame_sequence
                .checked_add(1)
                .ok_or("debug frame sequence exhausted")?;
            let mut meta = [0u8; 36];
            meta[..4].copy_from_slice(&frame_sequence.to_le_bytes());
            meta[4..8].copy_from_slice(&command_sequence.to_le_bytes());
            meta[8..16].copy_from_slice(&engine.tick().to_le_bytes());
            meta[16..20].copy_from_slice(&render_us.to_le_bytes());
            meta[20..24].copy_from_slice(&encode_us.to_le_bytes());
            meta[24..28].copy_from_slice(&frame.width.to_le_bytes());
            meta[28..32].copy_from_slice(&frame.height.to_le_bytes());
            meta[32..36].copy_from_slice(&std::process::id().to_le_bytes());
            packet(&mut output, 0, &[&meta, &png])?;
            if frame_sequence % 3 == 1 || step > 0 {
                spawns.refresh(engine.world(), &mut names, &mut spawned);
                let state = State {
                    tick: engine.tick(),
                    paused,
                    speed,
                    selected: selection
                        .as_ref()
                        .map(|(id, entity)| selected(&engine, id, *entity, &names)),
                    spawned: std::mem::take(&mut spawned),
                };
                packet(&mut output, 2, &[toml::to_string(&state)?.as_bytes()])?;
                spawned = state.spawned;
            }
            deadline += interval;
            let now = Instant::now();
            if deadline < now {
                deadline = now + interval;
            }
            std::thread::sleep(deadline.saturating_duration_since(now));
        }
        Ok(())
    })();
    engine.stop(&mut game);
    drop(output);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_parse_and_keys_map() {
        let parse = |text: &str| toml::from_str::<Envelope>(text).map(|e| e.request);
        assert!(matches!(
            parse("request={seq=1, op=\"step\", ticks=3}").unwrap(),
            Command::Step { ticks: 3, .. }
        ));
        assert!(matches!(
            parse("request={seq=2, op=\"select\"}").unwrap(),
            Command::Select { id: None, .. }
        ));
        assert!(matches!(
            parse("request={seq=3, op=\"keys\", held=[\"w\",\"space\"]}").unwrap(),
            Command::Keys { .. }
        ));
        assert!(matches!(
            parse("request={seq=5, op=\"mouse\", dx=3.5, dy=-2}").unwrap(),
            Command::Mouse {
                dx: 3.5,
                dy: -2.0,
                ..
            }
        ));
        assert!(parse("request={seq=4, op=\"teleport\"}").is_err());
        assert_eq!(key("space"), Some(Key::Space));
        assert_eq!(key("f1"), None);
    }

    #[test]
    fn spawned_objects_are_labelled_parented_and_dropped_when_deleted() {
        let mut world = crate::World::new();
        let scene = world.spawn("Ship");
        let mut names = std::collections::HashMap::from([(scene, "ship-1".to_string())]);
        let mut spawns = Spawns::new(&world);
        let mut out = Vec::new();
        spawns.refresh(&world, &mut names, &mut out);
        assert!(out.is_empty());

        let bullet = world.spawn_child(scene, "Bullet").unwrap();
        let spark = world.spawn_child(bullet, "Spark").unwrap();
        spawns.refresh(&world, &mut names, &mut out);
        let listed: Vec<_> = out
            .iter()
            .map(|s| (s.id.as_str(), s.name.as_str(), s.parent.as_deref()))
            .collect();
        assert_eq!(
            listed,
            [
                ("spawned-1", "Bullet", Some("ship-1")),
                ("spawned-2", "Spark", Some("spawned-1"))
            ]
        );
        assert_eq!(spawns.find("spawned-2"), Some(spark));

        world.remove_subtree(bullet).unwrap();
        let again = world.spawn("Bullet");
        spawns.refresh(&world, &mut names, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "spawned-3", "labels are not reused");
        assert_eq!(spawns.find("spawned-3"), Some(again));
        assert!(spawns.find("spawned-1").is_none() && !names.contains_key(&bullet));

        // The live state packet carries the list alongside the selection.
        let state = State {
            tick: 1,
            paused: false,
            speed: 1.0,
            selected: Some(Selected {
                id: "ship-1".into(),
                ..Selected::default()
            }),
            spawned: out,
        };
        let text = toml::to_string(&state).unwrap();
        let back: toml::Value = toml::from_str(&text).unwrap();
        assert_eq!(back["spawned"][0]["name"].as_str(), Some("Bullet"));
        assert_eq!(back["selected"]["id"].as_str(), Some("ship-1"));
    }
}
