use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use slotmap::{new_key_type, SlotMap};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

use crate::{Geometry, Line, Mesh, Point, Polygon};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const INITIAL_INSTANCE_CAPACITY: usize = 64;
static NEXT_RENDERER_ID: AtomicU64 = AtomicU64::new(1);

new_key_type! {
    struct GeometryKey;
}

/// Renderer-scoped runtime identity for uploaded geometry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeometryId {
    renderer: u64,
    key: GeometryKey,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectUniform {
    model_view_projection: [[f32; 4]; 4],
}

struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryInstance {
    pub geometry: GeometryId,
    pub model: Mat4,
}

impl GeometryInstance {
    pub fn new(geometry: GeometryId, model: Mat4) -> Self {
        Self { geometry, model }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RendererInfo {
    pub adapter_name: String,
    pub backend: String,
    pub surface_format: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphicsError {
    CreateSurface(String),
    NoAdapter(String),
    RequestDevice(String),
    UnsupportedSurface,
    MissingGeometry(GeometryId),
    NonFiniteModelMatrix { instance: usize },
    TooManyInstances(usize),
    SurfaceLost,
    SurfaceValidation,
}

impl fmt::Display for GraphicsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateSurface(error) => write!(formatter, "could not create surface: {error}"),
            Self::NoAdapter(error) => {
                write!(formatter, "could not select graphics adapter: {error}")
            }
            Self::RequestDevice(error) => write!(formatter, "could not create GPU device: {error}"),
            Self::UnsupportedSurface => {
                formatter.write_str("selected adapter does not support the window surface")
            }
            Self::MissingGeometry(id) => write!(formatter, "geometry handle {id:?} is unavailable"),
            Self::NonFiniteModelMatrix { instance } => write!(
                formatter,
                "geometry instance {instance} has a non-finite model matrix"
            ),
            Self::TooManyInstances(count) => write!(
                formatter,
                "cannot address {count} geometry instances in one frame"
            ),
            Self::SurfaceLost => formatter.write_str("window surface was lost"),
            Self::SurfaceValidation => formatter.write_str("window surface validation failed"),
        }
    }
}

impl Error for GraphicsError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderOutcome {
    Presented,
    Reconfigured,
    Skipped,
}

/// Renderer-owned, depth-tested storage for uploaded geometry.
pub struct GeometryRenderer {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    uniform_stride: u64,
    uniform_capacity: usize,
    uniform_bytes: Vec<u8>,
    renderer_id: u64,
    geometry: SlotMap<GeometryKey, GpuMesh>,
    depth_view: wgpu::TextureView,
    info: RendererInfo,
}

impl GeometryRenderer {
    pub async fn new(
        window: Arc<Window>,
        display: OwnedDisplayHandle,
    ) -> Result<Self, GraphicsError> {
        let size = non_zero_size(window.inner_size());
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display)),
        );
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| GraphicsError::CreateSurface(error.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
            .map_err(|error| GraphicsError::NoAdapter(error.to_string()))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Emergent device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| GraphicsError::RequestDevice(error.to_string()))?;
        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .ok_or(GraphicsError::UnsupportedSurface)?;
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Emergent geometry shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/geometry.wgsl").into()),
        });
        let uniform_size = std::mem::size_of::<ObjectUniform>() as u64;
        let uniform_stride = align_up(
            uniform_size,
            device.limits().min_uniform_buffer_offset_alignment as u64,
        );
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Emergent geometry uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(uniform_size),
                },
                count: None,
            }],
        });
        let (uniform_buffer, uniform_bind_group) = create_uniform_resources(
            &device,
            &uniform_layout,
            uniform_stride,
            INITIAL_INSTANCE_CAPACITY,
        );
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Emergent geometry pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Emergent geometry pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::layout())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let depth_view = create_depth_view(&device, size);
        let adapter_info = adapter.get_info();
        let info = RendererInfo {
            adapter_name: adapter_info.name,
            backend: format!("{:?}", adapter_info.backend),
            surface_format: format!("{:?}", config.format),
        };

        Ok(Self {
            instance,
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_layout,
            uniform_buffer,
            uniform_bind_group,
            uniform_stride,
            uniform_capacity: INITIAL_INSTANCE_CAPACITY,
            uniform_bytes: vec![0; uniform_stride as usize * INITIAL_INSTANCE_CAPACITY],
            renderer_id: NEXT_RENDERER_ID.fetch_add(1, Ordering::Relaxed),
            geometry: SlotMap::with_key(),
            depth_view,
            info,
        })
    }

    pub fn info(&self) -> &RendererInfo {
        &self.info
    }

    pub fn insert(&mut self, geometry: &Geometry) -> GeometryId {
        let (vertices, indices) = tessellate(geometry);
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Emergent geometry vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Emergent geometry indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let key = self.geometry.insert(GpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        });
        GeometryId {
            renderer: self.renderer_id,
            key,
        }
    }

    pub fn remove(&mut self, geometry: GeometryId) -> bool {
        geometry.renderer == self.renderer_id && self.geometry.remove(geometry.key).is_some()
    }
    pub fn contains(&self, geometry: GeometryId) -> bool {
        geometry.renderer == self.renderer_id && self.geometry.contains_key(geometry.key)
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, size);
    }

    pub fn render(
        &mut self,
        instances: &[GeometryInstance],
    ) -> Result<RenderOutcome, GraphicsError> {
        self.validate_instances(instances)?;
        self.ensure_uniform_capacity(instances.len())?;
        let aspect = self.config.width as f32 / self.config.height as f32;
        let view_matrix = look_at_rh(Vec3::new(7.0, 5.0, 9.0), Vec3::ZERO, Vec3::Y);
        let projection = perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        for (index, instance) in instances.iter().enumerate() {
            let uniform = ObjectUniform {
                model_view_projection: (projection * view_matrix * instance.model)
                    .to_cols_array_2d(),
            };
            let offset = index * self.uniform_stride as usize;
            let bytes = bytemuck::bytes_of(&uniform);
            self.uniform_bytes[offset..offset + bytes.len()].copy_from_slice(bytes);
        }
        if !instances.is_empty() {
            let byte_count = instances.len() * self.uniform_stride as usize;
            self.queue
                .write_buffer(&self.uniform_buffer, 0, &self.uniform_bytes[..byte_count]);
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(RenderOutcome::Skipped)
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(RenderOutcome::Reconfigured);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .map_err(|_| GraphicsError::SurfaceLost)?;
                self.surface.configure(&self.device, &self.config);
                return Ok(RenderOutcome::Reconfigured);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(GraphicsError::SurfaceValidation)
            }
        };

        let surface_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Emergent geometry frame encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Emergent geometry pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.025,
                            g: 0.035,
                            b: 0.060,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            for (index, instance) in instances.iter().enumerate() {
                let offset = u32::try_from(index as u64 * self.uniform_stride)
                    .map_err(|_| GraphicsError::TooManyInstances(instances.len()))?;
                let mesh = &self.geometry[instance.geometry.key];
                pass.set_bind_group(0, &self.uniform_bind_group, &[offset]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        self.queue.present(frame);
        Ok(RenderOutcome::Presented)
    }

    fn validate_instances(&self, instances: &[GeometryInstance]) -> Result<(), GraphicsError> {
        for (index, instance) in instances.iter().enumerate() {
            if !self.contains(instance.geometry) {
                return Err(GraphicsError::MissingGeometry(instance.geometry));
            }
            if !instance.model.is_finite() {
                return Err(GraphicsError::NonFiniteModelMatrix { instance: index });
            }
        }
        Ok(())
    }

    fn ensure_uniform_capacity(&mut self, required: usize) -> Result<(), GraphicsError> {
        if required <= self.uniform_capacity {
            return Ok(());
        }
        let capacity = required
            .checked_next_power_of_two()
            .ok_or(GraphicsError::TooManyInstances(required))?;
        let maximum = u32::MAX as u64 / self.uniform_stride + 1;
        if capacity as u64 > maximum {
            return Err(GraphicsError::TooManyInstances(required));
        }
        let (buffer, bind_group) = create_uniform_resources(
            &self.device,
            &self.uniform_layout,
            self.uniform_stride,
            capacity,
        );
        self.uniform_buffer = buffer;
        self.uniform_bind_group = bind_group;
        self.uniform_capacity = capacity;
        self.uniform_bytes = vec![0; self.uniform_stride as usize * capacity];
        Ok(())
    }
}

/// Compatibility wrapper for the original one-cube example.
pub struct CubeRenderer {
    renderer: GeometryRenderer,
    cube: GeometryId,
}

impl CubeRenderer {
    pub async fn new(
        window: Arc<Window>,
        display: OwnedDisplayHandle,
    ) -> Result<Self, GraphicsError> {
        let mut renderer = GeometryRenderer::new(window, display).await?;
        let cube = renderer.insert(&Geometry::Mesh(
            Mesh::cube(1.0).expect("the built-in cube must be valid"),
        ));
        Ok(Self { renderer, cube })
    }
    pub fn info(&self) -> &RendererInfo {
        self.renderer.info()
    }
    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.renderer.resize(size);
    }
    pub fn render(&mut self, model: Mat4) -> Result<RenderOutcome, GraphicsError> {
        self.renderer
            .render(&[GeometryInstance::new(self.cube, model)])
    }
}

fn create_uniform_resources(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    stride: u64,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Emergent geometry instance uniforms"),
        size: stride * capacity as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Emergent geometry uniform bind group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: NonZeroU64::new(std::mem::size_of::<ObjectUniform>() as u64),
            }),
        }],
    });
    (buffer, bind_group)
}

fn tessellate(geometry: &Geometry) -> (Vec<Vertex>, Vec<u32>) {
    match geometry {
        Geometry::Point(point) => tessellate_point(*point),
        Geometry::Line(line) => tessellate_line(*line),
        Geometry::Polygon(polygon) => tessellate_polygon(polygon),
        Geometry::Mesh(mesh) => tessellate_mesh(mesh),
    }
}

fn tessellate_point(point: Point) -> (Vec<Vertex>, Vec<u32>) {
    let center = point.position();
    let radius = point.radius();
    let positions = [
        center + Vec3::X * radius,
        center - Vec3::X * radius,
        center + Vec3::Y * radius,
        center - Vec3::Y * radius,
        center + Vec3::Z * radius,
        center - Vec3::Z * radius,
    ];
    let color = point.color().components();
    let vertices = positions
        .into_iter()
        .map(|position| Vertex {
            position: position.to_array(),
            color,
        })
        .collect();
    let indices = vec![
        0, 2, 4, 4, 2, 1, 1, 2, 5, 5, 2, 0, 0, 4, 3, 4, 1, 3, 1, 5, 3, 5, 0, 3,
    ];
    (vertices, indices)
}

fn tessellate_line(line: Line) -> (Vec<Vertex>, Vec<u32>) {
    let direction = (line.end() - line.start()).normalize();
    let helper = if direction.dot(Vec3::Y).abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let side = direction.cross(helper).normalize() * line.radius();
    let up = direction.cross(side).normalize() * line.radius();
    let positions = [
        line.start() - side - up,
        line.start() + side - up,
        line.start() + side + up,
        line.start() - side + up,
        line.end() - side - up,
        line.end() + side - up,
        line.end() + side + up,
        line.end() - side + up,
    ];
    let color = line.color().components();
    let vertices = positions
        .into_iter()
        .map(|position| Vertex {
            position: position.to_array(),
            color,
        })
        .collect();
    let indices = vec![
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6,
        3, 0, 4, 3, 4, 7,
    ];
    (vertices, indices)
}

fn tessellate_polygon(polygon: &Polygon) -> (Vec<Vertex>, Vec<u32>) {
    let color = polygon.color().components();
    let vertices = polygon
        .vertices()
        .iter()
        .map(|position| Vertex {
            position: position.to_array(),
            color,
        })
        .collect();
    let mut indices = Vec::with_capacity((polygon.vertices().len() - 2) * 3);
    for index in 1..polygon.vertices().len() - 1 {
        indices.extend_from_slice(&[0, index as u32, index as u32 + 1]);
    }
    (vertices, indices)
}

fn tessellate_mesh(mesh: &Mesh) -> (Vec<Vertex>, Vec<u32>) {
    let vertices = mesh
        .vertices()
        .iter()
        .map(|vertex| Vertex {
            position: vertex.position().to_array(),
            color: vertex.color().components(),
        })
        .collect();
    (vertices, mesh.indices().to_vec())
}

fn align_up(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}
fn non_zero_size(size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    PhysicalSize::new(size.width.max(1), size.height.max(1))
}

fn look_at_rh(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let forward = (target - eye).normalize();
    let right = forward.cross(up).normalize();
    let camera_up = right.cross(forward);
    Mat4::from_cols(
        Vec4::new(right.x, camera_up.x, -forward.x, 0.0),
        Vec4::new(right.y, camera_up.y, -forward.y, 0.0),
        Vec4::new(right.z, camera_up.z, -forward.z, 0.0),
        Vec4::new(-right.dot(eye), -camera_up.dot(eye), forward.dot(eye), 1.0),
    )
}

fn perspective_rh(vertical_fov: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let focal_length = 1.0 / (vertical_fov * 0.5).tan();
    let depth = far / (near - far);
    Mat4::from_cols(
        Vec4::new(focal_length / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, focal_length, 0.0, 0.0),
        Vec4::new(0.0, 0.0, depth, -1.0),
        Vec4::new(0.0, 0.0, near * depth, 0.0),
    )
}

fn create_depth_view(device: &wgpu::Device, size: PhysicalSize<u32>) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Emergent depth texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, MeshVertex};

    #[test]
    fn clamps_zero_surface_dimensions_for_initial_configuration() {
        assert_eq!(
            non_zero_size(PhysicalSize::new(0, 0)),
            PhysicalSize::new(1, 1)
        );
    }

    #[test]
    fn projection_maps_near_and_far_depth_to_webgpu_range() {
        let projection = perspective_rh(45.0_f32.to_radians(), 1.0, 0.1, 100.0);
        let near = projection * Vec4::new(0.0, 0.0, -0.1, 1.0);
        let far = projection * Vec4::new(0.0, 0.0, -100.0, 1.0);
        assert!((near.z / near.w).abs() < 1.0e-5);
        assert!((far.z / far.w - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn tessellates_every_geometry_family_to_valid_triangles() {
        let point = Geometry::from(Point::new(Vec3::ZERO, 0.2, Color::RED).unwrap());
        let line = Geometry::from(
            Line::new(Vec3::ZERO, Vec3::new(2.0, 1.0, -1.0), 0.1, Color::GREEN).unwrap(),
        );
        let polygon = Geometry::from(
            Polygon::new(
                vec![Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y],
                Color::BLUE,
            )
            .unwrap(),
        );
        let mesh = Geometry::from(
            Mesh::new(
                vec![
                    MeshVertex::new(Vec3::ZERO, Color::RED).unwrap(),
                    MeshVertex::new(Vec3::X, Color::GREEN).unwrap(),
                    MeshVertex::new(Vec3::Y, Color::BLUE).unwrap(),
                ],
                vec![0, 1, 2],
            )
            .unwrap(),
        );
        for geometry in [&point, &line, &polygon, &mesh] {
            let (vertices, indices) = tessellate(geometry);
            assert!(!vertices.is_empty());
            assert!(!indices.is_empty());
            assert!(indices.len().is_multiple_of(3));
            assert!(indices
                .iter()
                .all(|index| (*index as usize) < vertices.len()));
        }
    }

    #[test]
    fn slotmap_geometry_handles_become_stale_without_aliasing() {
        let mut storage = SlotMap::<GeometryKey, ()>::with_key();
        let removed_key = storage.insert(());
        let removed = GeometryId {
            renderer: 7,
            key: removed_key,
        };
        assert_eq!(storage.remove(removed.key), Some(()));
        let replacement = GeometryId {
            renderer: 7,
            key: storage.insert(()),
        };
        assert_ne!(removed, replacement);
        assert!(!storage.contains_key(removed.key));
    }

    #[test]
    fn aligns_dynamic_uniform_offsets() {
        assert_eq!(align_up(64, 256), 256);
        assert_eq!(align_up(256, 256), 256);
        assert_eq!(align_up(257, 256), 512);
    }
}
