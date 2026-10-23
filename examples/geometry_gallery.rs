use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use emergent::{
    Color, Engine, EngineConfig, EngineContext, EntityId, Game, Geometry, GeometryId,
    GeometryInstance, GeometryRenderer, LightInstance, LightSource, Line, Mesh, MeshVertex, Point,
    Polygon, Quat, RenderOutcome, ShadowCasting, Transform, Vec3,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct GeometryGallery {
    entities: Option<[EntityId; 4]>,
    exit_after_ticks: Option<u64>,
}

impl Game for GeometryGallery {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        let scene = context.world.spawn("Geometry gallery");
        let entities = ["Point", "Line", "Polygon", "Mesh"].map(|name| {
            context
                .world
                .spawn_child(scene, name)
                .expect("the gallery scene accepts its children")
        });
        self.entities = Some(entities);
    }

    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        let angle = context.tick as f32 * context.fixed_delta.as_secs_f32() * 0.7;
        for (index, entity) in self
            .entities
            .expect("the geometry gallery has started")
            .into_iter()
            .enumerate()
        {
            let translation = Vec3::new(index as f32 * 2.4 - 3.6, 0.0, 0.0);
            let rotation = Quat::from_rotation_y(angle + index as f32 * 0.35)
                * Quat::from_rotation_x(angle * 0.25);
            context
                .world
                .set_local_transform(entity, Transform::new(translation, rotation, Vec3::ONE))
                .expect("gallery transforms remain valid");
        }
        if self
            .exit_after_ticks
            .is_some_and(|limit| context.tick.saturating_add(1) >= limit)
        {
            context.request_exit();
        }
    }
}

struct GalleryApp {
    engine: Engine,
    game: GeometryGallery,
    window: Option<Arc<Window>>,
    renderer: Option<GeometryRenderer>,
    geometry: Option<[GeometryId; 4]>,
    previous_frame: Instant,
    running: bool,
    occluded: bool,
    failure: Option<String>,
    presented_frames: u64,
    smoke_resize_requested: bool,
    smoke_resize_observed: bool,
}

impl GalleryApp {
    fn new(exit_after_ticks: Option<u64>) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            engine: Engine::new(EngineConfig::default())?,
            game: GeometryGallery {
                entities: None,
                exit_after_ticks,
            },
            window: None,
            renderer: None,
            geometry: None,
            previous_frame: Instant::now(),
            running: false,
            occluded: false,
            failure: None,
            presented_frames: 0,
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
        if self.running {
            self.engine.stop(&mut self.game);
            self.running = false;
            println!(
                "Emergent geometry gallery stopped after {} ticks and {} presented frames (resize observed: {})",
                self.engine.tick(),
                self.presented_frames,
                self.smoke_resize_observed
            );
            if self.game.exit_after_ticks.is_some()
                && !self.smoke_resize_observed
                && self.failure.is_none()
            {
                self.failure = Some("geometry smoke resize event was not observed".into());
            }
        }
    }
}

impl ApplicationHandler for GalleryApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Emergent — Point, line, polygon, mesh")
            .with_inner_size(LogicalSize::new(1100, 680));
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
        let geometry = match gallery_geometry() {
            Ok(geometry) => geometry.map(|geometry| renderer.insert(&geometry)),
            Err(error) => return self.fail(event_loop, error),
        };
        if let Err(error) = self.engine.start(&mut self.game) {
            return self.fail(event_loop, error);
        }
        let info = renderer.info();
        println!(
            "Emergent geometry: adapter='{}', backend={}, format={}, resources={}",
            info.adapter_name,
            info.backend,
            info.surface_format,
            geometry.len()
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
                if size == PhysicalSize::new(900, 600) {
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
                if self.game.exit_after_ticks.is_some()
                    && !self.smoke_resize_requested
                    && self.engine.tick() >= 30
                {
                    let _ = window.request_inner_size(PhysicalSize::new(900, 600));
                    self.smoke_resize_requested = true;
                }
                let entities = self.game.entities.expect("running gallery has entities");
                let handles = self.geometry.expect("running gallery has geometry");
                let mut matrices = [emergent::Mat4::IDENTITY; 4];
                for (index, entity) in entities.into_iter().enumerate() {
                    matrices[index] = match self.engine.world().global_transform(entity) {
                        Ok(transform) => transform.matrix(),
                        Err(error) => return self.fail(event_loop, error),
                    };
                }
                let instances: [GeometryInstance; 4] = std::array::from_fn(|index| {
                    GeometryInstance::new(handles[index], matrices[index])
                });
                let Some(renderer) = &mut self.renderer else {
                    return;
                };
                let lights = [
                    LightInstance::new(
                        LightSource::ambient(Color::WHITE, 0.22)
                            .expect("constant ambient light is valid"),
                        emergent::Mat4::IDENTITY,
                    ),
                    LightInstance::new(
                        LightSource::directional(Color::WHITE, 1.1)
                            .expect("constant directional light is valid")
                            .with_shadow_casting(ShadowCasting::Enabled)
                            .expect("directional lights may cast shadows"),
                        emergent::Mat4::from_quat(Quat::from_rotation_arc(
                            Vec3::NEG_Z,
                            Vec3::new(-0.5, -1.0, -0.35).normalize(),
                        )),
                    ),
                ];
                match renderer.render_lit(&instances, &lights) {
                    Ok(RenderOutcome::Presented) => self.presented_frames += 1,
                    Ok(RenderOutcome::Reconfigured | RenderOutcome::Skipped) => {}
                    Err(error) => return self.fail(event_loop, error),
                }
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

fn gallery_geometry() -> Result<[Geometry; 4], Box<dyn Error>> {
    let point = Point::new(Vec3::ZERO, 0.38, Color::YELLOW)?;
    let line = Line::new(
        Vec3::new(-0.8, -0.45, 0.0),
        Vec3::new(0.8, 0.65, 0.0),
        0.09,
        Color::CYAN,
    )?;
    let polygon = Polygon::new(
        vec![
            Vec3::new(-0.8, -0.6, 0.0),
            Vec3::new(0.65, -0.7, 0.0),
            Vec3::new(0.9, 0.35, 0.0),
            Vec3::new(0.0, 0.9, 0.0),
            Vec3::new(-0.85, 0.35, 0.0),
        ],
        Color::MAGENTA,
    )?;
    let mesh = Mesh::new(
        vec![
            MeshVertex::new(Vec3::new(-0.8, -0.7, 0.8), Color::RED)?,
            MeshVertex::new(Vec3::new(0.8, -0.7, 0.8), Color::GREEN)?,
            MeshVertex::new(Vec3::new(0.8, -0.7, -0.8), Color::BLUE)?,
            MeshVertex::new(Vec3::new(-0.8, -0.7, -0.8), Color::CYAN)?,
            MeshVertex::new(Vec3::new(0.0, 0.9, 0.0), Color::YELLOW)?,
        ],
        vec![0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4, 0, 3, 2, 0, 2, 1],
    )?;
    Ok([point.into(), line.into(), polygon.into(), mesh.into()])
}

fn main() -> Result<(), Box<dyn Error>> {
    let exit_after_ticks = match std::env::args().nth(1).as_deref() {
        None => None,
        Some("--smoke") => Some(120),
        Some(_) => return Err("usage: geometry_gallery [--smoke]".into()),
    };
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = GalleryApp::new(exit_after_ticks)?;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    Ok(())
}
