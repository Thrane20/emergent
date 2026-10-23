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
        ];
        let key_light = self
            .engine
            .world()
            .component::<LightSource>(scene.key_light)
            .expect("generated key-light entity remains valid")
            .copied()
            .expect("generated key-light entity retains its light component");
        let key_light_matrix = match self.engine.world().global_transform(scene.key_light) {
            Ok(transform) => transform.matrix(),
            Err(error) => return self.fail(event_loop, error),
        };
        let lights = [
            LightInstance::new(self.ambient_light, Mat4::IDENTITY),
            LightInstance::new(key_light, key_light_matrix),
        ];
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
            Ok([ground, cube, pyramid]) => ProjectGeometry {
                ground: renderer.insert_with_shading(&ground, ShadingMode::Flat),
                cube: renderer.insert_with_shading(&cube, ShadingMode::Flat),
                pyramid: renderer.insert_with_shading(&pyramid, ShadingMode::Flat),
            },
            Err(error) => return self.fail(event_loop, error),
        };
        if let Err(error) = self.engine.start(&mut self.game) {
            return self.fail(event_loop, error);
        }
        let info = renderer.info();
        println!(
            "First Project graphics: adapter='{}', backend={}, format={}, resources=3",
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

fn project_geometry() -> Result<[Geometry; 3], Box<dyn Error>> {
    let ground_color = Color::new(0.12, 0.22, 0.16, 1.0)?;
    Ok([
        Primitive::plane(6.0, 6.0, ground_color)?.into(),
        Primitive::cube(0.45, Color::CYAN)?.into(),
        Primitive::pyramid(0.8, 1.8, Color::YELLOW)?.into(),
    ])
}

fn scene_matrices(engine: &Engine, scene: MainScene) -> Result<[Mat4; 5], Box<dyn Error>> {
    Ok([
        world_matrix(engine, scene.ground)?,
        world_matrix(engine, scene.leader)?,
        world_matrix(engine, scene.follower)?,
        world_matrix(engine, scene.tail)?,
        world_matrix(engine, scene.pyramid)?,
    ])
}

fn world_matrix(engine: &Engine, entity: EntityId) -> Result<Mat4, Box<dyn Error>> {
    Ok(engine.world().global_transform(entity)?.matrix())
}
