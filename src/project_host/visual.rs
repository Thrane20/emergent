use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use crate::{
    Color, Engine, EngineConfig, EntityId, GeometryId, GeometryInstance, GeometryRenderer,
    LightInstance, LightSource, Mat4, RenderOutcome, ShadingMode,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{CursorGrabMode, Window, WindowId};

use super::{ProjectGame, TickLimit};

const SMOKE_RESIZE: PhysicalSize<u32> = PhysicalSize::new(980, 640);

pub(crate) struct ProjectGeometry {
    raymarched: Vec<(EntityId, crate::RaymarchedTerrainId)>,
    handles: Vec<(EntityId, GeometryId)>,
    collider_handles: Vec<(EntityId, GeometryId)>,
    ui_handles: Vec<(EntityId, GeometryId)>,
    particle_entities: Vec<(EntityId, Mat4)>,
    preview_particles: crate::ParticleStore,
    pub(crate) instances: Vec<GeometryInstance>,
    pub(crate) lights: Vec<LightInstance>,
    ambient_light: LightSource,
}

impl ProjectGeometry {
    pub(crate) fn upload(
        renderer: &mut GeometryRenderer,
        game: &impl ProjectGame,
        world: &crate::World,
        authoring: bool,
    ) -> Result<Self, Box<dyn Error>> {
        renderer.set_environment(&game.loaded().environment);
        let mut raymarched = Vec::new();
        for &id in game.loaded().entities.values() {
            if let Some(field) = world.component::<crate::RaymarchedTerrain>(id)? {
                let handle = renderer.insert_raymarched_terrain(*field)?;
                renderer
                    .transform_raymarched_terrain(handle, world.global_transform(id)?.matrix())?;
                raymarched.push((id, handle));
            }
        }
        let mut handles = Vec::with_capacity(game.loaded().renderables.len());
        for (entity, geometry) in &game.loaded().renderables {
            let handle = if let Some(material) = world.component::<crate::Material>(*entity)? {
                renderer.insert_with_material(geometry, material)?
            } else {
                renderer.insert_with_shading(geometry, ShadingMode::Flat)
            };
            handles.push((*entity, handle));
        }
        let mut ui_entities: Vec<_> = game.loaded().entities.iter().collect();
        ui_entities.sort_by(|a, b| a.0.cmp(b.0));
        let mut ui_handles = Vec::new();
        for (_, &id) in ui_entities {
            if let Some(surface) = world.component::<crate::UiSurface>(id)? {
                ui_handles.push((id, renderer.insert_ui_surface(surface)?));
            }
        }
        let mut particle_entities = Vec::new();
        for &id in game.loaded().entities.values() {
            if let Some(effect) = world.component::<crate::ParticleEffect>(id)? {
                renderer.prepare_particle_effect(effect);
                if authoring {
                    particle_entities.push((id, world.global_transform(id)?.matrix()));
                }
            }
        }
        let preview_particles = if authoring {
            frozen_particles(world, &particle_entities)?
        } else {
            crate::ParticleStore::default()
        };
        let mut min = crate::Vec3::splat(f32::INFINITY);
        let mut max = crate::Vec3::splat(f32::NEG_INFINITY);
        for &(id, _) in &raymarched {
            let field = world
                .component::<crate::RaymarchedTerrain>(id)?
                .ok_or("Missing implicit terrain")?;
            let matrix = world.global_transform(id)?.matrix();
            let (lower, upper) = field.bounds();
            for x in [lower.x, upper.x] {
                for y in [lower.y, upper.y] {
                    for z in [lower.z, upper.z] {
                        let point = matrix.transform_point3(crate::Vec3::new(x, y, z));
                        min = min.min(point);
                        max = max.max(point);
                    }
                }
            }
        }
        for (entity, geometry) in &game.loaded().renderables {
            if let crate::Geometry::Mesh(mesh) = geometry {
                let matrix = world.global_transform(*entity)?.matrix();
                for vertex in mesh.vertices() {
                    let point = matrix.transform_point3(vertex.position());
                    min = min.min(point);
                    max = max.max(point);
                }
            }
        }
        preview_particles.visit(|e, _, model, particles| {
            for p in particles {
                let point = if e.local_space {
                    model.transform_point3(p.position)
                } else {
                    p.position
                };
                let radius = crate::Vec3::splat(e.size[0].max(e.size[1]) * 0.5 * p.scale);
                min = min.min(point - radius);
                max = max.max(point + radius);
            }
        });
        if min.is_finite() && max.is_finite() {
            let target = (min + max) * 0.5;
            let distance = ((max - min).length() * 1.3).max(2.);
            renderer.set_view(
                target + crate::Vec3::new(7., 5., 9.).normalize() * distance,
                target,
                crate::Vec3::Y,
            )?;
        }
        sync_camera(renderer, world, game.loaded().active_camera)?;
        Ok(Self {
            raymarched,
            instances: Vec::with_capacity(handles.len()),
            lights: Vec::with_capacity(9),
            ambient_light: game.loaded().environment.settings.ambient_light()?,
            handles,
            collider_handles: Vec::new(),
            ui_handles,
            particle_entities,
            preview_particles,
        })
    }

    /// Freeze VFX at two seconds in authoring, without ticking gameplay or physics.
    pub(crate) fn extract_particles(
        &mut self,
        renderer: &mut GeometryRenderer,
        world: &crate::World,
        authoring: bool,
    ) -> Result<(), Box<dyn Error>> {
        let mut index = 0;
        while index < self.raymarched.len() {
            let (entity, handle) = self.raymarched[index];
            if world
                .component::<crate::RaymarchedTerrain>(entity)
                .ok()
                .flatten()
                .is_none()
            {
                renderer.remove_raymarched_terrain(handle);
                self.raymarched.swap_remove(index);
            } else {
                renderer.transform_raymarched_terrain(
                    handle,
                    world.global_transform(entity)?.matrix(),
                )?;
                index += 1;
            }
        }
        if !authoring {
            renderer.set_particles(world.particles());
            return Ok(());
        }
        let mut changed = false;
        for (entity, previous) in &mut self.particle_entities {
            let current = world.global_transform(*entity)?.matrix();
            changed |= current != *previous;
            *previous = current;
        }
        if changed {
            self.preview_particles = frozen_particles(world, &self.particle_entities)?;
        }
        renderer.set_particles(&self.preview_particles);
        Ok(())
    }

    /// Retained authoring overlays; refresh only the edited entity, never part of Run geometry.
    pub(crate) fn add_collider_overlays(
        &mut self,
        renderer: &mut GeometryRenderer,
        game: &impl ProjectGame,
        world: &crate::World,
        only: Option<EntityId>,
    ) -> Result<(), Box<dyn Error>> {
        self.collider_handles.retain(|&(id, handle)| {
            if only.is_none_or(|selected| selected == id) {
                renderer.remove(handle);
                false
            } else {
                true
            }
        });
        let overlay_material = crate::MaterialDefinition {
            shading: crate::MaterialShading::Unlit,
            cast_shadows: false,
            receive_shadows: false,
            ..Default::default()
        }
        .constant()?;
        for &id in game.loaded().entities.values() {
            if only.is_some_and(|selected| selected != id) {
                continue;
            }
            let Some(body) = world.component::<crate::PhysicsBody>(id)? else {
                continue;
            };
            let Some(collider) = body.collider.as_box() else {
                continue;
            };
            let center = crate::Vec3::from_array(collider.center);
            let half = crate::Vec3::from_array(collider.half_extents);
            for corner in 0..8u32 {
                let point = |bits: u32| {
                    center
                        + half
                            * crate::Vec3::new(
                                if bits & 1 == 0 { -1.0 } else { 1.0 },
                                if bits & 2 == 0 { -1.0 } else { 1.0 },
                                if bits & 4 == 0 { -1.0 } else { 1.0 },
                            )
                };
                for axis in [1, 2, 4] {
                    if corner & axis != 0 {
                        continue;
                    }
                    let line = crate::Line::new(
                        point(corner),
                        point(corner | axis),
                        0.012,
                        Color::YELLOW,
                    )?;
                    self.collider_handles.push((
                        id,
                        renderer.insert_with_material(
                            &crate::Geometry::from(line),
                            &overlay_material,
                        )?,
                    ));
                }
            }
        }
        self.instances
            .reserve(self.handles.len().saturating_sub(self.instances.capacity()));
        Ok(())
    }

    pub(crate) fn replace_material(
        &self,
        entity: EntityId,
        material: &crate::Material,
        game: &impl ProjectGame,
        renderer: &mut GeometryRenderer,
    ) -> Result<(), Box<dyn Error>> {
        let (_, handle) = self
            .handles
            .iter()
            .find(|(id, _)| *id == entity)
            .ok_or("Selected entity has no uploaded mesh")?;
        let (_, geometry) = game
            .loaded()
            .renderables
            .iter()
            .find(|(id, _)| *id == entity)
            .ok_or("Selected entity has no source geometry")?;
        renderer.replace_material(*handle, geometry, material)?;
        Ok(())
    }
    pub(crate) fn extract(
        &mut self,
        engine: &Engine,
        game: &impl ProjectGame,
        renderer: &mut GeometryRenderer,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(time) = engine.world().resource::<crate::SimulationTime>() {
            renderer.set_shader_time(time.tick as f32 * time.fixed_delta.as_secs_f32());
        }
        self.instances.clear();
        for &(entity, geometry) in self.handles.iter().chain(&self.collider_handles) {
            if engine.world().get(entity).is_err() {
                continue;
            }
            self.instances.push(GeometryInstance::new(
                geometry,
                engine.world().global_transform(entity)?.matrix(),
            ));
        }
        let mut index = 0;
        while index < self.ui_handles.len() {
            let (entity, geometry) = self.ui_handles[index];
            if engine
                .world()
                .component::<crate::UiSurface>(entity)
                .ok()
                .flatten()
                .is_none()
            {
                renderer.remove(geometry);
                self.ui_handles.remove(index);
                continue;
            }
            self.instances.push(GeometryInstance::new(
                geometry,
                engine.world().global_transform(entity)?.matrix(),
            ));
            index += 1;
        }
        self.lights.clear();
        for &entity in &game.loaded().lights {
            let source = *engine
                .world()
                .component::<LightSource>(entity)?
                .ok_or("authored light component is missing")?;
            self.lights.push(LightInstance::new(
                source,
                engine.world().global_transform(entity)?.matrix(),
            ));
        }
        self.lights
            .push(LightInstance::new(self.ambient_light, Mat4::IDENTITY));
        Ok(())
    }
}

fn frozen_particles(
    world: &crate::World,
    entities: &[(EntityId, Mat4)],
) -> Result<crate::ParticleStore, Box<dyn Error>> {
    let mut store = crate::ParticleStore::default();
    for &(id, model) in entities {
        if let Some(effect) = world.component::<crate::ParticleEffect>(id)? {
            store.insert(effect.clone(), model)?;
        }
    }
    // shortcut: fixed authoring sample; add an independent VFX timeline for short bursts and scrubbing.
    for _ in 0..120 {
        store.step(crate::PARTICLE_STEP)?;
    }
    Ok(store)
}

pub(crate) fn sync_camera(
    renderer: &mut GeometryRenderer,
    world: &crate::World,
    active: Option<EntityId>,
) -> Result<(), Box<dyn Error>> {
    if let Some(entity) = active {
        let camera = *world
            .component::<crate::Camera>(entity)?
            .ok_or("active camera component is missing")?;
        let matrix = world.global_transform(entity)?.matrix();
        let eye = matrix.transform_point3(crate::Vec3::ZERO);
        let forward = matrix.transform_vector3(-crate::Vec3::Z).normalize();
        let up = matrix.transform_vector3(crate::Vec3::Y).normalize();
        renderer.set_camera(eye, eye + forward, up, camera.projection())?;
    }
    Ok(())
}

struct ProjectApp<G: ProjectGame> {
    project_name: String,
    engine: Engine,
    game: TickLimit<G>,
    window: Option<Arc<Window>>,
    renderer: Option<GeometryRenderer>,
    geometry: Option<ProjectGeometry>,
    previous_frame: Instant,
    running: bool,
    occluded: bool,
    failure: Option<String>,
    presented_frames: u64,
    smoke: bool,
    smoke_resize_requested: bool,
    smoke_resize_observed: bool,
    keys: crate::KeySet,
    mouse_captured: bool,
    #[cfg(feature = "audio")]
    _audio: Option<crate::audio::output::AudioOutput>,
}

impl<G: ProjectGame> ProjectApp<G> {
    fn new(
        project: &crate::Project,
        exit_after_ticks: Option<u64>,
        scene_path: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let mut engine = Engine::new(EngineConfig::default())?;
        // A missing or busy device leaves the game running silently.
        #[cfg(feature = "audio")]
        let audio = match crate::audio::output::open(crate::AudioConfig::default()) {
            Ok((audio, output)) => {
                println!(
                    "Audio output: {} Hz, {} channels",
                    output.sample_rate, output.channels
                );
                engine.set_audio(audio);
                Some(output)
            }
            Err(error) => {
                eprintln!("Audio disabled: {error}");
                None
            }
        };
        let game = TickLimit::new(G::load(&mut engine, project, scene_path)?, exit_after_ticks);
        Ok(Self {
            project_name: project.manifest().name.clone(),
            #[cfg(feature = "audio")]
            _audio: audio,
            engine,
            game,
            window: None,
            renderer: None,
            geometry: None,
            previous_frame: Instant::now(),
            running: false,
            occluded: false,
            failure: None,
            presented_frames: 0,
            smoke: exit_after_ticks.is_some(),
            smoke_resize_requested: false,
            smoke_resize_observed: false,
            keys: crate::KeySet::EMPTY,
            mouse_captured: false,
        })
    }

    fn uses_mouse_look(&self) -> bool {
        self.game.inner.loaded().active_camera.is_some_and(|id| {
            self.engine
                .world()
                .component::<crate::MouseLook>(id)
                .ok()
                .flatten()
                .is_some()
        })
    }
    fn release_mouse(&mut self) {
        self.mouse_captured = false;
        self.keys = crate::KeySet::EMPTY;
        self.engine.clear_input();
        if let Some(window) = &self.window {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
            if self.uses_mouse_look() {
                window.set_title(&format!(
                    "Emergent — {} — click to capture mouse; WASD + Space; Escape releases",
                    self.project_name
                ));
            }
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.failure = Some(error.to_string());
        self.stop();
        event_loop.exit();
    }

    fn stop(&mut self) {
        self.release_mouse();
        if !self.running {
            return;
        }
        self.engine.stop(&mut self.game);
        self.running = false;
        println!(
            "{} stopped after {} ticks and {} presented frames",
            self.project_name,
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
        let Some(geometry) = &mut self.geometry else {
            return;
        };
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        if let Err(error) = geometry.extract(&self.engine, &self.game.inner, renderer) {
            return self.fail(event_loop, error);
        }
        if let Err(error) = sync_camera(
            renderer,
            self.engine.world(),
            self.game.inner.loaded().active_camera,
        ) {
            return self.fail(event_loop, error);
        }
        if let Err(error) = geometry.extract_particles(renderer, self.engine.world(), false) {
            return self.fail(event_loop, error);
        }
        match renderer.render_lit(&geometry.instances, &geometry.lights) {
            Ok(RenderOutcome::Presented) => self.presented_frames += 1,
            Ok(RenderOutcome::Reconfigured | RenderOutcome::Skipped) => {}
            Err(error) => self.fail(event_loop, error),
        }
    }
}

impl<G: ProjectGame> ApplicationHandler for ProjectApp<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(format!("Emergent — {}", self.project_name))
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
        let geometry = match ProjectGeometry::upload(
            &mut renderer,
            &self.game.inner,
            self.engine.world(),
            false,
        ) {
            Ok(geometry) => geometry,
            Err(error) => return self.fail(event_loop, error),
        };
        if let Err(error) = self.engine.start(&mut self.game) {
            return self.fail(event_loop, error);
        }
        let info = renderer.info();
        println!(
            "{} graphics: adapter='{}', backend={}, format={}, resources={}",
            self.project_name,
            info.adapter_name,
            info.backend,
            info.surface_format,
            geometry.handles.len()
        );
        self.running = true;
        self.previous_frame = Instant::now();
        self.geometry = Some(geometry);
        self.renderer = Some(renderer);
        if self.uses_mouse_look() {
            window.set_title(&format!(
                "Emergent — {} — click to capture mouse; WASD + Space; Escape releases",
                self.project_name
            ));
        }
        self.window = Some(window.clone());
        window.request_redraw();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.mouse_captured {
            if let DeviceEvent::MouseMotion { delta } = event {
                self.engine
                    .add_mouse_motion(crate::Vec2::new(delta.0 as f32, delta.1 as f32));
            }
        }
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
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } if self.uses_mouse_look() && !self.mouse_captured && window.has_focus() => {
                match window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                {
                    Ok(()) => {
                        window.set_cursor_visible(false);
                        window.set_title(&format!(
                            "Emergent — {} — mouse captured; WASD + Space; Escape releases",
                            self.project_name
                        ));
                        self.engine.clear_input();
                        self.keys = crate::KeySet::EMPTY;
                        self.mouse_captured = true;
                    }
                    Err(error) => window.set_title(&format!(
                        "Emergent — {} — mouse capture unavailable: {error}",
                        self.project_name
                    )),
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.physical_key
                    == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape)
                    && event.state.is_pressed()
                {
                    self.release_mouse();
                    return;
                }
                if self.uses_mouse_look() && !self.mouse_captured {
                    return;
                }
                if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                    if let Some(key) = crate::Key::from_winit(code) {
                        if event.state.is_pressed() {
                            self.keys.insert(key);
                        } else {
                            self.keys.remove(key);
                        }
                        self.engine.set_keys(self.keys);
                    }
                }
            }
            // Releases are lost while unfocused, so drop held keys rather than leave them stuck.
            WindowEvent::Focused(false) => {
                self.release_mouse();
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if !occluded {
                    self.previous_frame = Instant::now();
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested if self.running && !self.occluded => {
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
        if self.running && !self.occluded {
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

pub fn run<G: ProjectGame>(
    project: &crate::Project,
    exit_after_ticks: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    run_scene::<G>(
        project,
        exit_after_ticks,
        project
            .manifest()
            .entry_scene
            .to_str()
            .ok_or("non-UTF8 scene path")?,
    )
}

pub fn run_scene<G: ProjectGame>(
    project: &crate::Project,
    exit_after_ticks: Option<u64>,
    scene_path: &str,
) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ProjectApp::<G>::new(project, exit_after_ticks, scene_path)?;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    Ok(())
}

/// Constructs the project's registered scene at tick zero and captures its real
/// renderer output without opening a window or advancing gameplay.
pub fn preview<G: ProjectGame>(
    project: &crate::Project,
    scene_path: &str,
    output: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let mut engine = Engine::new(EngineConfig::default())?;
    let mut game = G::load(&mut engine, project, scene_path)?;
    let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(1280, 720))?;
    let mut geometry = ProjectGeometry::upload(&mut renderer, &game, engine.world(), true)?;
    engine.start(&mut game)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        // Upload synced the camera before start propagated world transforms; resync from the committed pose.
        sync_camera(&mut renderer, engine.world(), game.loaded().active_camera)?;
        geometry.extract(&engine, &game, &mut renderer)?;
        geometry.extract_particles(&mut renderer, engine.world(), true)?;
        let frame = renderer.render_lit_rgba(&geometry.instances, &geometry.lights)?;
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

#[cfg(test)]
mod camera_tests {
    use super::*;
    use crate::{Camera, CameraProjection, Quat, Transform, Vec3, World};

    #[test]
    fn camera_dolly_and_orthographic_size_change_native_pixels() {
        let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(96, 96)).unwrap();
        let cube = crate::Primitive::cube(1., Color::WHITE).unwrap();
        let geometry = renderer.insert(&cube.into());
        let instances = [GeometryInstance::new(geometry, Mat4::IDENTITY)];
        let perspective = CameraProjection::perspective(45., 0.1, 100.).unwrap();
        renderer
            .set_camera(Vec3::new(0., 0., 10.), Vec3::ZERO, Vec3::Y, perspective)
            .unwrap();
        let distant = renderer.render_lit_rgba(&instances, &[]).unwrap().rgba;
        renderer
            .set_camera(Vec3::new(0., 0., 5.), Vec3::ZERO, Vec3::Y, perspective)
            .unwrap();
        assert_ne!(
            distant,
            renderer.render_lit_rgba(&instances, &[]).unwrap().rgba
        );
        renderer.set_projection(CameraProjection::isometric(12., 0.1, 100.).unwrap());
        let wide = renderer.render_lit_rgba(&instances, &[]).unwrap().rgba;
        renderer
            .set_view(Vec3::new(0., 0., 10.), Vec3::ZERO, Vec3::Y)
            .unwrap();
        assert_eq!(
            wide,
            renderer.render_lit_rgba(&instances, &[]).unwrap().rgba
        );
        renderer.set_projection(CameraProjection::isometric(6., 0.1, 100.).unwrap());
        assert_ne!(
            wide,
            renderer.render_lit_rgba(&instances, &[]).unwrap().rgba
        );
    }

    #[test]
    fn preview_renders_from_the_started_active_camera() {
        // Regression: syncing before start read unpropagated transforms, leaving the camera inside geometry.
        // The standard template is the fixture, so engine changes that break new projects fail here.
        struct Template(crate::LoadedScene);
        impl crate::Game for Template {
            fn fixed_update(&mut self, _: &mut crate::EngineContext<'_>) {}
        }
        impl ProjectGame for Template {
            fn load(
                engine: &mut Engine,
                project: &crate::Project,
                scene: &str,
            ) -> Result<Self, Box<dyn Error>> {
                Ok(Self(
                    crate::SceneDefinition::load_with(project, scene, &Self::components())?
                        .instantiate(engine.world_mut())?,
                ))
            }
            fn loaded(&self) -> &crate::LoadedScene {
                &self.0
            }
        }
        let project =
            crate::Project::load(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/standard"))
                .unwrap();
        let directory =
            std::env::temp_dir().join(format!("emergent-preview-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let output = directory.join("frame.png");
        let _ = std::fs::remove_file(&output);
        preview::<Template>(&project, "assets/scenes/main.scene.toml", &output).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(&output).unwrap(),
        ));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        let colours: std::collections::HashSet<_> = pixels[..info.buffer_size()]
            .chunks(4)
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        std::fs::remove_dir_all(&directory).unwrap();
        assert!(
            colours.len() > 16,
            "preview looks empty: {} distinct colours",
            colours.len()
        );
    }

    #[test]
    fn active_camera_tracks_parent_motion_projection_changes_and_stale_handles() {
        let mut renderer = pollster::block_on(GeometryRenderer::new_offscreen(64, 64)).unwrap();
        let mut world = World::new();
        let parent = world.spawn("camera rig");
        let camera = world.spawn_child(parent, "camera").unwrap();
        world
            .insert_component(
                camera,
                Camera::new(CameraProjection::perspective(60., 0.1, 1000.).unwrap()),
            )
            .unwrap();
        world.propagate_transforms();
        sync_camera(&mut renderer, &world, Some(camera)).unwrap();
        let initial = renderer.view_projection();
        world
            .set_local_transform(
                parent,
                Transform::new(Vec3::new(3., 2., 5.), Quat::from_rotation_y(0.5), Vec3::ONE),
            )
            .unwrap();
        world.propagate_transforms();
        sync_camera(&mut renderer, &world, Some(camera)).unwrap();
        let moved = renderer.view_projection();
        assert_ne!(initial, moved);
        let matrix = world.global_transform(camera).unwrap().matrix();
        let eye = matrix.transform_point3(Vec3::ZERO);
        let forward = matrix.transform_vector3(-Vec3::Z).normalize();
        let center = moved.project_point3(eye + forward * 5.);
        assert!(center.x.abs() < 0.0001 && center.y.abs() < 0.0001);
        world
            .insert_component(
                camera,
                Camera::new(CameraProjection::isometric(20., 0.1, 1000.).unwrap()),
            )
            .unwrap();
        sync_camera(&mut renderer, &world, Some(camera)).unwrap();
        assert_ne!(moved, renderer.view_projection());
        let orthographic = renderer.view_projection();
        sync_camera(&mut renderer, &world, None).unwrap();
        assert_eq!(orthographic, renderer.view_projection());
        world.remove_subtree(parent).unwrap();
        assert!(sync_camera(&mut renderer, &world, Some(camera)).is_err());
        assert_eq!(orthographic, renderer.view_projection());
    }
}
