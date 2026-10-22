use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use emergent::{
    CubeRenderer, Engine, EngineConfig, EngineContext, EntityId, Game, Quat, RenderOutcome,
    Transform, Vec3,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct FirstVisibleGame {
    cube: Option<EntityId>,
    exit_after_ticks: Option<u64>,
}

impl Game for FirstVisibleGame {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        let scene = context.world.spawn("Visible scene");
        let cube = context
            .world
            .spawn_child(scene, "First cube")
            .expect("the scene must accept its cube");
        context
            .world
            .set_local_transform(cube, Transform::from_translation(Vec3::ZERO))
            .expect("the initial cube transform must be valid");
        self.cube = Some(cube);
    }

    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        let angle = context.tick as f32 * context.fixed_delta.as_secs_f32();
        let rotation = Quat::from_rotation_y(angle) * Quat::from_rotation_x(angle * 0.45);
        context
            .world
            .set_local_transform(
                self.cube.expect("the visible game must be started"),
                Transform::new(Vec3::ZERO, rotation, Vec3::splat(0.85)),
            )
            .expect("the generated cube transform must be valid");
        if self
            .exit_after_ticks
            .is_some_and(|limit| context.tick.saturating_add(1) >= limit)
        {
            context.request_exit();
        }
    }
}

struct VisibleApp {
    engine: Engine,
    game: FirstVisibleGame,
    window: Option<Arc<Window>>,
    renderer: Option<CubeRenderer>,
    previous_frame: Instant,
    running: bool,
    occluded: bool,
    failure: Option<String>,
    presented_frames: u64,
    smoke_resize_requested: bool,
    smoke_resize_observed: bool,
}

impl VisibleApp {
    fn new(exit_after_ticks: Option<u64>) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            engine: Engine::new(EngineConfig::default())?,
            game: FirstVisibleGame {
                cube: None,
                exit_after_ticks,
            },
            window: None,
            renderer: None,
            previous_frame: Instant::now(),
            running: false,
            occluded: false,
            failure: None,
            presented_frames: 0,
            smoke_resize_requested: false,
            smoke_resize_observed: false,
        })
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.failure = Some(error.to_string());
        event_loop.exit();
    }

    fn stop(&mut self) {
        if self.running {
            self.engine.stop(&mut self.game);
            self.running = false;
            println!(
                "Emergent graphics stopped cleanly after {} ticks and {} presented frames (resize observed: {})",
                self.engine.tick(),
                self.presented_frames,
                self.smoke_resize_observed
            );
            if self.game.exit_after_ticks.is_some()
                && !self.smoke_resize_observed
                && self.failure.is_none()
            {
                self.failure = Some("graphics smoke resize event was not observed".into());
            }
        }
    }
}

impl ApplicationHandler for VisibleApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("Emergent — First wgpu object")
            .with_inner_size(LogicalSize::new(960, 640));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        let renderer = match pollster::block_on(CubeRenderer::new(
            window.clone(),
            event_loop.owned_display_handle(),
        )) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        if let Err(error) = self.engine.start(&mut self.game) {
            self.fail(event_loop, error);
            return;
        }

        let info = renderer.info();
        println!(
            "Emergent graphics: adapter='{}', backend={}, format={}",
            info.adapter_name, info.backend, info.surface_format
        );
        self.running = true;
        self.previous_frame = Instant::now();
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
        let Some(window) = &self.window else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                self.stop();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if size == PhysicalSize::new(800, 600) {
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
                    self.fail(event_loop, error);
                    return;
                }
                if self.game.exit_after_ticks.is_some()
                    && !self.smoke_resize_requested
                    && self.engine.tick() >= 30
                {
                    let _ = window.request_inner_size(PhysicalSize::new(800, 600));
                    self.smoke_resize_requested = true;
                }
                let cube = self.game.cube.expect("running game has a cube");
                let model = match self.engine.world().global_transform(cube) {
                    Ok(transform) => transform.matrix(),
                    Err(error) => {
                        self.fail(event_loop, error);
                        return;
                    }
                };
                let Some(renderer) = &mut self.renderer else {
                    return;
                };
                match renderer.render(model) {
                    Ok(RenderOutcome::Presented) => self.presented_frames += 1,
                    Ok(RenderOutcome::Reconfigured | RenderOutcome::Skipped) => {}
                    Err(error) => {
                        self.fail(event_loop, error);
                        return;
                    }
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
                let size = window.inner_size();
                if size != PhysicalSize::new(0, 0) {
                    window.request_redraw();
                }
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.stop();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let exit_after_ticks = match std::env::args().nth(1).as_deref() {
        None => None,
        Some("--smoke") => Some(120),
        Some(_) => return Err("usage: first_window [--smoke]".into()),
    };
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = VisibleApp::new(exit_after_ticks)?;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    Ok(())
}
