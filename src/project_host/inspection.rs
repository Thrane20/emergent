use std::error::Error;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::sync::mpsc::{sync_channel, TryRecvError};
use std::time::{Duration, Instant};

use super::inspection_tools::{ToolCommand, Tools};
use crate::{CameraProjection, Engine, EngineConfig, GeometryRenderer, Quat, Transform, Vec3};
use serde::Deserialize;

use super::{visual::ProjectGeometry, ProjectGame};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    request: Request,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Request {
    Normal(Command),
    Tool(ToolCommand),
}
impl Request {
    fn seq(&self) -> u32 {
        match self {
            Self::Normal(c) => c.seq(),
            Self::Tool(c) => c.seq(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Command {
    Classification {
        seq: u32,
        id: String,
    },
    Material {
        seq: u32,
        id: String,
    },
    Physics {
        seq: u32,
        id: String,
    },
    Camera {
        seq: u32,
        id: String,
        projection: String,
        field_of_view_degrees: Option<f32>,
        view_size: Option<f32>,
        near: f32,
        far: f32,
    },
    View {
        seq: u32,
        yaw: f32,
        pitch: f32,
        distance: f32,
        target: [f32; 3],
        projection: String,
        field_of_view_degrees: f32,
        view_size: f32,
    },
    Transform {
        seq: u32,
        id: String,
        translation: [f32; 3],
        rotation_degrees: [f32; 3],
        scale: [f32; 3],
    },
    Stop {
        seq: u32,
    },
    Reparent {
        seq: u32,
        id: String,
        parent: Option<String>,
    },
}
impl Command {
    fn seq(&self) -> u32 {
        match self {
            Self::Classification { seq, .. }
            | Self::Material { seq, .. }
            | Self::Physics { seq, .. }
            | Self::Camera { seq, .. }
            | Self::View { seq, .. }
            | Self::Transform { seq, .. }
            | Self::Stop { seq }
            | Self::Reparent { seq, .. } => *seq,
        }
    }
}

fn packet(writer: &mut impl Write, kind: u32, payload: &[&[u8]]) -> std::io::Result<()> {
    let size: usize = payload.iter().map(|slice| slice.len()).sum();
    if size > 4 * 1024 * 1024 {
        return Err(std::io::Error::other("inspection packet exceeds 4 MiB"));
    }
    writer.write_all(b"EMI1")?;
    writer.write_all(&kind.to_le_bytes())?;
    writer.write_all(&(size as u32).to_le_bytes())?;
    for slice in payload {
        writer.write_all(slice)?;
    }
    writer.flush()
}

fn acknowledge(
    writer: &mut impl Write,
    seq: u32,
    result: &Result<String, Box<dyn Error>>,
) -> std::io::Result<()> {
    let status = u32::from(result.is_err());
    let message = match result {
        Ok(data) => data.clone(),
        Err(error) => error.to_string(),
    };
    let mut end = message.len().min(if result.is_ok() {
        2 * 1024 * 1024
    } else {
        2048
    });
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    packet(
        writer,
        1,
        &[
            &seq.to_le_bytes(),
            &status.to_le_bytes(),
            &message.as_bytes()[..end],
        ],
    )
}

fn apply(
    command: &Command,
    engine: &mut Engine,
    game: &impl ProjectGame,
    renderer: &mut GeometryRenderer,
) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Camera {
            id,
            projection,
            field_of_view_degrees,
            view_size,
            near,
            far,
            ..
        } => {
            let entity = *game
                .loaded()
                .entities
                .get(id)
                .ok_or("Camera is not in this scene")?;
            if engine.world().component::<crate::Camera>(entity)?.is_none() {
                return Err("Selected entity is not a camera".into());
            }
            let projection = match projection.as_str() {
                "perspective" if view_size.is_none() => CameraProjection::perspective(
                    field_of_view_degrees.ok_or("Camera FOV is required")?,
                    *near,
                    *far,
                )?,
                "isometric" | "orthographic" if field_of_view_degrees.is_none() => {
                    CameraProjection::isometric(
                        view_size.ok_or("Orthographic view size is required")?,
                        *near,
                        *far,
                    )?
                }
                _ => return Err("Unsupported camera projection or inconsistent settings".into()),
            };
            engine
                .world_mut()
                .insert_component(entity, crate::Camera::new(projection))?;
        }
        Command::View {
            yaw,
            pitch,
            distance,
            target,
            projection,
            field_of_view_degrees,
            view_size,
            ..
        } => {
            let eye = inspection_eye(*yaw, *pitch, *distance, *target)?;
            let far = (*distance * 8.0).clamp(100.0, 1_000_000.0);
            let projection = match projection.as_str() {
                "perspective" => CameraProjection::perspective(*field_of_view_degrees, 0.1, far)?,
                "isometric" => CameraProjection::isometric(*view_size, 0.1, far)?,
                _ => return Err("unsupported inspection camera projection".into()),
            };
            renderer.set_camera(eye, Vec3::from_array(*target), Vec3::Y, projection)?;
        }
        Command::Transform {
            id,
            translation,
            rotation_degrees,
            scale,
            ..
        } => {
            if id.len() > 128
                || !translation
                    .iter()
                    .chain(rotation_degrees)
                    .chain(scale)
                    .all(|n| n.is_finite())
                || scale.iter().any(|n| n.abs() < 0.0001)
            {
                return Err("invalid instance transform".into());
            }
            let entity = *game
                .loaded()
                .entities
                .get(id)
                .ok_or("instance is not in this inspection scene")?;
            let before = *engine.world().local_transform(entity)?;
            let [x, y, z] = rotation_degrees.map(f32::to_radians);
            engine.world_mut().set_local_transform(
                entity,
                Transform::new(
                    Vec3::from_array(*translation),
                    Quat::from_euler(crate::EulerRot::XYZ, x, y, z),
                    Vec3::from_array(*scale),
                ),
            )?;
            engine.world_mut().propagate_transforms();
            let valid =
                game.loaded()
                    .renderables
                    .iter()
                    .map(|(id, _)| id)
                    .chain(game.loaded().lights.iter())
                    .all(|id| {
                        engine.world().global_transform(*id).is_ok_and(|t| {
                            t.matrix().is_finite() && t.matrix().inverse().is_finite()
                        })
                    });
            if !valid {
                engine.world_mut().set_local_transform(entity, before)?;
                engine.world_mut().propagate_transforms();
                return Err("transform would overflow the inspection world".into());
            }
        }
        Command::Reparent { id, parent, .. } => {
            let entity = *game
                .loaded()
                .entities
                .get(id)
                .ok_or("instance is not in this inspection scene")?;
            let target = parent
                .as_ref()
                .map(|id| {
                    game.loaded()
                        .entities
                        .get(id)
                        .copied()
                        .ok_or("parent is not in this inspection scene")
                })
                .transpose()?
                .unwrap_or(game.loaded().root);
            let before = engine.world().parent(entity)?;
            engine.world_mut().reparent(entity, Some(target))?;
            engine.world_mut().propagate_transforms();
            if !game.loaded().entities.values().all(|id| {
                engine
                    .world()
                    .global_transform(*id)
                    .is_ok_and(|t| t.matrix().is_finite() && t.matrix().inverse().is_finite())
            }) {
                engine.world_mut().reparent(entity, before)?;
                engine.world_mut().propagate_transforms();
                return Err("parenting would overflow the inspection world".into());
            }
        }
        Command::Stop { .. }
        | Command::Physics { .. }
        | Command::Material { .. }
        | Command::Classification { .. } => (),
    }
    Ok(())
}

fn inspection_eye(
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: [f32; 3],
) -> Result<Vec3, Box<dyn Error>> {
    if ![yaw, pitch, distance]
        .iter()
        .chain(target.iter())
        .all(|n| n.is_finite())
        || !(-1.45..=1.45).contains(&pitch)
        || !(0.25..=30000.0).contains(&distance)
        || target.iter().any(|n| n.abs() > 10000.)
    {
        return Err("inspection camera is outside its finite orbit bounds".into());
    }
    Ok(Vec3::from_array(target)
        + distance
            * Vec3::new(
                pitch.cos() * yaw.sin(),
                pitch.sin(),
                pitch.cos() * yaw.cos(),
            ))
}

/// One retained authoring world; no calls to advance_frame, run_ticks or fixed_update.
pub fn run<G: ProjectGame>(
    project: &crate::Project,
    scene_path: &str,
) -> Result<(), Box<dyn Error>> {
    let mut engine = Engine::new(EngineConfig::default())?;
    let mut game = G::load(&mut engine, project, scene_path)?;
    // shortcut: bounded PNG/readback at 30 Hz; replace when measured scenes exceed the budget.
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(960, 540))?;
    let mut geometry = ProjectGeometry::upload(&mut renderer, &game, engine.world(), true)?;
    geometry.add_collider_overlays(&mut renderer, &game, engine.world(), None)?;
    let mut tools = Tools::new(&game);
    engine.start(&mut game)?;
    // Upload synced the camera before start propagated world transforms; resync from the committed pose.
    super::visual::sync_camera(&mut renderer, engine.world(), game.loaded().active_camera)?;
    eprintln!(
        "Inspection constructed once: adapter={}, backend={}, instances={}, tick=0",
        renderer.info().adapter_name,
        renderer.info().backend,
        game.loaded().renderables.len()
    );
    let (sender, receiver) = sync_channel::<Result<Request, String>>(8);
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
                    let _ = sender.send(Err(
                        "inspection command exceeds 4096 bytes or could not be read".into(),
                    ));
                    break;
                }
            }
        }
    });
    let mut output = BufWriter::new(std::io::stdout().lock());
    let mut frame_sequence = 0u32;
    let mut command_sequence = 0u32;
    let mut png = Vec::with_capacity(256 * 1024);
    let interval = Duration::from_secs_f64(1.0 / 30.0);
    let mut deadline = Instant::now();
    let result = (|| -> Result<(), Box<dyn Error>> {
        'session: loop {
            for _ in 0..8 {
                match receiver.try_recv() {
                    Ok(Ok(command)) => {
                        let result = (|| -> Result<String, Box<dyn Error>> {
                            let placement = match &command {
                                Request::Tool(command) => {
                                    tools.apply(command, &mut engine, &game, &renderer)?
                                }
                                Request::Normal(command) => {
                                    if tools.dragging() {
                                        return Err(
                                            "Finish or cancel the active gizmo drag first".into()
                                        );
                                    }
                                    if let Command::Classification { id, .. } = command {
                                        let definition = crate::SceneDefinition::load_with(
                                            project,
                                            scene_path,
                                            &G::components(),
                                        )?;
                                        let entity = *game
                                            .loaded()
                                            .entities
                                            .get(id)
                                            .ok_or("Object is not in inspection")?;
                                        engine.world_mut().insert_component(
                                            entity,
                                            definition.inspection_classification(id)?,
                                        )?;
                                    } else if let Command::Material { id, .. } = command {
                                        let definition = crate::SceneDefinition::load_with(
                                            project,
                                            scene_path,
                                            &G::components(),
                                        )?;
                                        let entity = *game.loaded().entities.get(id).ok_or(
                                            "Material instance is not in the inspection scene",
                                        )?;
                                        engine.world().get(entity)?;
                                        let material =
                                            definition.inspection_material(id)?.unwrap_or(
                                                crate::MaterialDefinition::default().constant()?,
                                            );
                                        geometry.replace_material(
                                            entity,
                                            &material,
                                            &game,
                                            &mut renderer,
                                        )?;
                                        engine.world_mut().insert_component(entity, material)?;
                                    } else if let Command::Physics { id, .. } = command {
                                        // shortcut: reuse full authored validation; narrow asset parsing if edit latency warrants it.
                                        let definition = crate::SceneDefinition::load_with(
                                            project,
                                            scene_path,
                                            &G::components(),
                                        )?;
                                        definition.apply_inspection_physics(
                                            engine.world_mut(),
                                            game.loaded(),
                                            id,
                                        )?;
                                        let entity = game.loaded().entities[id];
                                        geometry.add_collider_overlays(
                                            &mut renderer,
                                            &game,
                                            engine.world(),
                                            Some(entity),
                                        )?;
                                    } else {
                                        apply(command, &mut engine, &game, &mut renderer)?;
                                    }
                                    None
                                }
                            };
                            let mut result = tools.result(&engine, &game, &renderer)?;
                            result.placement = placement.map(Into::into);
                            Ok(toml::to_string(&result)?)
                        })();
                        acknowledge(&mut output, command.seq(), &result)?;
                        if result.is_ok() {
                            command_sequence = command.seq();
                        }
                        if matches!(command, Request::Normal(Command::Stop { .. })) {
                            break 'session;
                        }
                    }
                    Ok(Err(message)) => {
                        acknowledge(&mut output, 0, &Err(message.into()))?;
                    }
                    Err(TryRecvError::Disconnected) => break 'session,
                    Err(TryRecvError::Empty) => break,
                }
            }
            if engine.tick() != 0 {
                return Err("inspection advanced gameplay unexpectedly".into());
            }
            geometry.extract(&engine, &game, &mut renderer)?;
            let render_start = Instant::now();
            geometry.extract_particles(&mut renderer, engine.world(), true)?;
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
                .ok_or("inspection frame sequence exhausted")?;
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
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_camera_and_packet_contract() {
        assert!(inspection_eye(0.66, 0.414, 12.45, [0.; 3])
            .unwrap()
            .is_finite());
        assert!(inspection_eye(0., 2., 12., [0.; 3]).is_err());
        assert!(inspection_eye(0., 0.4, 200., [0.; 3]).is_ok());
        assert!(inspection_eye(0., 0.4, 30001., [0.; 3]).is_err());
        assert!(inspection_eye(f32::NAN, 0., 12., [0.; 3]).is_err());
        let mut bytes = Vec::new();
        packet(&mut bytes, 1, &[b"test"]).unwrap();
        assert_eq!(&bytes[..4], b"EMI1");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 4);
        assert!(toml::from_str::<Envelope>(
            "request={op='view',seq=1,yaw=0.0,pitch=0.0,distance=12.0,target=[0.0,0.0,0.0],projection='perspective',field_of_view_degrees=45.0,view_size=12.0}"
        )
        .is_ok());
        assert!(toml::from_str::<Envelope>("request={op='probe',seq=2,x=0.5,y=0.5}").is_ok());
        assert!(toml::from_str::<Envelope>("request={op='unknown',seq=1}").is_err());
    }
}
