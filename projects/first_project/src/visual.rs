use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use emergent::{
    Color, Engine, EngineConfig, EntityId, Geometry, GeometryId, GeometryInstance,
    GeometryRenderer, LightInstance, LightSource, Mat4, Primitive, RenderOutcome, ShadingMode,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::game::FirstProjectGame;
use crate::generated::main_scene::MainScene;

const SMOKE_RESIZE: PhysicalSize<u32> = PhysicalSize::new(980, 640);

#[derive(Clone, Copy)]
struct ProjectGeometry {
    ground: GeometryId,
    cube: GeometryId,
    pyramid: GeometryId,
    authored_cube: GeometryId,
}

struct FirstProjectApp {
    engine: Engine,
    game: FirstProjectGame,
    window: Option<Arc<Window>>,
    renderer: Option<GeometryRenderer>,
    geometry: Option<ProjectGeometry>,
    ambient_light: LightSource,
    previous_frame: Instant,
    running: bool,
    occluded: bool,
    failure: Option<String>,
    presented_frames: u64,
    smoke: bool,
    smoke_resize_requested: bool,
    smoke_resize_observed: bool,
}

impl FirstProjectApp {
    fn new(leader_speed: f32, exit_after_ticks: Option<u64>) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            engine: Engine::new(EngineConfig::default())?,
            game: FirstProjectGame::new(leader_speed, exit_after_ticks),
            window: None,
            renderer: None,
            geometry: None,
            ambient_light: LightSource::ambient(Color::WHITE, 0.18)?,
            previous_frame: Instant::now(),
            running: false,
            occluded: false,
            failure: None,
            presented_frames: 0,
            smoke: exit_after_ticks.is_some(),
            smoke_resize_requested: false,
            smoke_resize_observed: false,
        })
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.failure = Some(error.to_string());
        self.stop();
        event_loop.exit();
    }

    fn stop(&mut self) {
        if !self.running {
            return;
        }
        self.engine.stop(&mut self.game);
        self.running = false;
        println!(
            "Emergent First Project stopped after {} ticks and {} presented frames",
            self.engine.tick(),
            self.presented_frames
        );
        if self.smoke && self.failure.is_none() {
            if self.presented_frames == 0 {
                self.failure = Some("visual smoke presented no frames".into());
            } else if !self.smoke_resize_observed {
                self.failure = Some("visual smoke resize event was not observed".into());
            }
        }
    }

    fn render(&mut self, event_loop: &ActiveEventLoop) {
        let scene = self
            .game
            .scene()
            .expect("a running first-project game has constructed its generated scene");
        let geometry = self
            .geometry
            .expect("a running visual host has uploaded its geometry");
        let matrices = match scene_matrices(&self.engine, scene) {
            Ok(matrices) => matrices,
            Err(error) => return self.fail(event_loop, error),
        };
        let instances = [
            GeometryInstance::new(geometry.ground, matrices[0]),
            GeometryInstance::new(geometry.cube, matrices[1]),
            GeometryInstance::new(geometry.cube, matrices[2]),
            GeometryInstance::new(geometry.cube, matrices[3]),
            GeometryInstance::new(geometry.pyramid, matrices[4]),
            GeometryInstance::new(geometry.authored_cube, matrices[5]),
        ];
        let lights = match scene_lights(&self.engine, scene, self.ambient_light) {
            Ok(lights) => lights,
            Err(error) => return self.fail(event_loop, error),
        };
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        match renderer.render_lit(&instances, &lights) {
            Ok(RenderOutcome::Presented) => self.presented_frames += 1,
            Ok(RenderOutcome::Reconfigured | RenderOutcome::Skipped) => {}
            Err(error) => self.fail(event_loop, error),
        }
    }
}

impl ApplicationHandler for FirstProjectApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Emergent — First Project")
            .with_inner_size(LogicalSize::new(1180, 720));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => return self.fail(event_loop, error),
        };
        let mut renderer = match pollster::block_on(GeometryRenderer::new(
            window.clone(),
            event_loop.owned_display_handle(),
        )) {
            Ok(renderer) => renderer,
            Err(error) => return self.fail(event_loop, error),
        };
        let geometry = match project_geometry() {
            Ok([ground, cube, pyramid, authored_cube]) => ProjectGeometry {
                ground: renderer.insert_with_shading(&ground, ShadingMode::Flat),
                cube: renderer.insert_with_shading(&cube, ShadingMode::Flat),
                pyramid: renderer.insert_with_shading(&pyramid, ShadingMode::Flat),
                authored_cube: renderer.insert_with_shading(&authored_cube, ShadingMode::Flat),
            },
            Err(error) => return self.fail(event_loop, error),
        };
        if let Err(error) = self.engine.start(&mut self.game) {
            return self.fail(event_loop, error);
        }
        let info = renderer.info();
        println!(
            "First Project graphics: adapter='{}', backend={}, format={}, resources=4",
            info.adapter_name, info.backend, info.surface_format
        );
        self.running = true;
        self.previous_frame = Instant::now();
        self.geometry = Some(geometry);
        self.renderer = Some(renderer);
        self.window = Some(window.clone());
        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = &self.window else { return };
        if window.id() != window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.stop();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if size == SMOKE_RESIZE {
                    self.smoke_resize_observed = true;
                }
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                }
                if size.width > 0 && size.height > 0 {
                    window.request_redraw();
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if !occluded {
                    self.previous_frame = Instant::now();
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested if !self.occluded => {
                let now = Instant::now();
                let elapsed = now.duration_since(self.previous_frame);
                self.previous_frame = now;
                if let Err(error) = self.engine.advance_frame(&mut self.game, elapsed) {
                    return self.fail(event_loop, error);
                }
                if self.smoke && !self.smoke_resize_requested && self.engine.tick() >= 30 {
                    let _ = window.request_inner_size(SMOKE_RESIZE);
                    self.smoke_resize_requested = true;
                }
                self.render(event_loop);
                if self.engine.exit_requested() {
                    self.stop();
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.occluded {
            if let Some(window) = &self.window {
                if window.inner_size() != PhysicalSize::new(0, 0) {
                    window.request_redraw();
                }
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.stop();
    }
}

pub fn run(leader_speed: f32, exit_after_ticks: Option<u64>) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = FirstProjectApp::new(leader_speed, exit_after_ticks)?;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    Ok(())
}

fn project_geometry() -> Result<[Geometry; 4], Box<dyn Error>> {
    let ground_color = Color::new(0.12, 0.22, 0.16, 1.0)?;
    Ok([
        Primitive::plane(6.0, 6.0, ground_color)?.into(),
        Primitive::cube(0.45, Color::CYAN)?.into(),
        Primitive::pyramid(0.8, 1.8, Color::YELLOW)?.into(),
        Primitive::cube(0.5, Color::WHITE)?.into(),
    ])
}

/// Constructs the project's registered scene at tick zero and captures its real
/// renderer output without opening a window or advancing gameplay.
pub fn preview(
    project: &emergent::Project,
    speed: f32,
    scene_path: &str,
    output: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let definition: toml::Value = toml::from_str(&project.read_text_asset(scene_path)?)?;
    validate_preview_builder(&definition)?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(1280, 720))?;
    let [ground, cube, pyramid, authored_cube] = project_geometry()?;
    let ground = renderer.insert_with_shading(&ground, ShadingMode::Flat);
    let cube = renderer.insert_with_shading(&cube, ShadingMode::Flat);
    let pyramid = renderer.insert_with_shading(&pyramid, ShadingMode::Flat);
    let authored_cube = renderer.insert_with_shading(&authored_cube, ShadingMode::Flat);
    let mut engine = Engine::new(EngineConfig::default())?;
    let mut game = FirstProjectGame::new(speed, None);
    engine.start(&mut game)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        let scene = game.scene().ok_or("project scene was not constructed")?;
        let matrices = scene_matrices(&engine, scene)?;
        let instances = [
            GeometryInstance::new(ground, matrices[0]),
            GeometryInstance::new(cube, matrices[1]),
            GeometryInstance::new(cube, matrices[2]),
            GeometryInstance::new(cube, matrices[3]),
            GeometryInstance::new(pyramid, matrices[4]),
            GeometryInstance::new(authored_cube, matrices[5]),
        ];
        let lights = scene_lights(&engine, scene, LightSource::ambient(Color::WHITE, 0.18)?)?;
        let frame = renderer.render_lit_rgba(&instances, &lights)?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        let mut encoder =
            png::Encoder::new(std::io::BufWriter::new(file), frame.width, frame.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&frame.rgba)?;
        writer.finish()?;
        println!(
            "Rust scene preview: scene={scene_path}, tick={}, adapter={}, backend={}, size={}x{}",
            engine.tick(),
            renderer.info().adapter_name,
            renderer.info().backend,
            frame.width,
            frame.height
        );
        Ok(())
    })();
    engine.stop(&mut game);
    result
}

fn validate_preview_builder(definition: &toml::Value) -> Result<(), Box<dyn Error>> {
    if definition
        .get("schema_version")
        .and_then(toml::Value::as_integer)
        != Some(1)
    {
        return Err("scene preview requires schema_version = 1".into());
    }
    if definition.get("builder").and_then(toml::Value::as_str)
        != Some("emergent.first-project.generated.main_scene")
    {
        return Err("this scene has no registered Rust preview builder; first_project currently supports emergent.first-project.generated.main_scene".into());
    }
    Ok(())
}

fn scene_lights(
    engine: &Engine,
    scene: MainScene,
    ambient: LightSource,
) -> Result<[LightInstance; 2], Box<dyn Error>> {
    let key_light = engine
        .world()
        .component::<LightSource>(scene.key_light)?
        .copied()
        .ok_or("key-light component is missing")?;
    Ok([
        LightInstance::new(ambient, Mat4::IDENTITY),
        LightInstance::new(
            key_light,
            engine.world().global_transform(scene.key_light)?.matrix(),
        ),
    ])
}

fn scene_matrices(engine: &Engine, scene: MainScene) -> Result<[Mat4; 6], Box<dyn Error>> {
    Ok([
        world_matrix(engine, scene.ground)?,
        world_matrix(engine, scene.leader)?,
        world_matrix(engine, scene.follower)?,
        world_matrix(engine, scene.tail)?,
        world_matrix(engine, scene.pyramid)?,
        world_matrix(engine, scene.cube)?,
    ])
}

fn world_matrix(engine: &Engine, entity: EntityId) -> Result<Mat4, Box<dyn Error>> {
    Ok(engine.world().global_transform(entity)?.matrix())
}

#[cfg(test)]
mod preview_tests {
    use super::validate_preview_builder;

    #[test]
    fn only_registered_rust_builders_are_rendered() {
        let valid = toml::from_str(
            "schema_version = 1\nbuilder = \"emergent.first-project.generated.main_scene\"\n",
        )
        .unwrap();
        assert!(validate_preview_builder(&valid).is_ok());
        let unsupported =
            toml::from_str("schema_version = 1\nbuilder = \"another.scene\"\n").unwrap();
        assert!(validate_preview_builder(&unsupported).is_err());
    }
}
