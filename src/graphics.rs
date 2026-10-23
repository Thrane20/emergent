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

use crate::{Geometry, LightKind, LightSource, Line, Mesh, Point, Polygon, ShadowCasting};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const INITIAL_INSTANCE_CAPACITY: usize = 64;
const MAX_LIGHTS: usize = 8;
const SHADOW_MAP_SIZE: u32 = 2048;
const SHADOW_DISTANCE: f32 = 22.0;
const SHADOW_EXTENT: f32 = 14.0;
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
    normal: [f32; 3],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4, 2 => Float32x3];

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
    model: [[f32; 4]; 4],
    model_view_projection: [[f32; 4]; 4],
    normal_matrix: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    light_view_projection: [[f32; 4]; 4],
    shadow_enabled: u32,
    _padding: [u32; 7],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuLight {
    position_kind: [f32; 4],
    color_intensity: [f32; 4],
    direction_range: [f32; 4],
    spot_and_shadow: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LightUniform {
    lights: [GpuLight; MAX_LIGHTS],
    count: u32,
    _padding: [u32; 7],
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

/// How the renderer derives the normals stored in an uploaded geometry handle.
///
/// `Smooth` retains authored normals and generates shared vertex normals where
/// they are absent. `Flat` expands indexed triangles during upload so every
/// face has one constant normal.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShadingMode {
    #[default]
    Smooth,
    Flat,
}

/// Copied light data for a single render frame.
///
/// Construct it from a validated `LightSource` component and that light
/// entity's propagated transform after simulation. It does not own an entity
/// or persist an identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightInstance {
    source: LightSource,
    model: Mat4,
}

impl LightInstance {
    pub fn new(source: LightSource, model: Mat4) -> Self {
        Self { source, model }
    }

    pub fn source(self) -> LightSource {
        self.source
    }

    pub fn model(self) -> Mat4 {
        self.model
    }
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
    NonFiniteLightMatrix { light: usize },
    InvalidLightDirection { light: usize },
    TooManyInstances(usize),
    TooManyLights(usize),
    SurfaceLost,
    SurfaceValidation,
    InvalidCaptureSize { width: u32, height: u32 },
    WrongRenderTarget,
    Readback(String),
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
            Self::NonFiniteLightMatrix { light } => {
                write!(
                    formatter,
                    "light instance {light} has a non-finite model matrix"
                )
            }
            Self::InvalidLightDirection { light } => {
                write!(
                    formatter,
                    "light instance {light} has no usable transformed direction"
                )
            }
            Self::TooManyInstances(count) => write!(
                formatter,
                "cannot address {count} geometry instances in one frame"
            ),
            Self::TooManyLights(count) => write!(
                formatter,
                "cannot render {count} lights; this renderer accepts at most {MAX_LIGHTS}"
            ),
            Self::SurfaceLost => formatter.write_str("window surface was lost"),
            Self::SurfaceValidation => formatter.write_str("window surface validation failed"),
            Self::InvalidCaptureSize { width, height } => write!(
                formatter,
                "capture size {width}x{height} must be between 1 and 4096 pixels per dimension"
            ),
            Self::WrongRenderTarget => formatter.write_str(
                "this operation requires the matching window or offscreen render target",
            ),
            Self::Readback(error) => write!(formatter, "GPU readback failed: {error}"),
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

/// Tightly packed RGBA8 pixels captured by the Rust renderer for authoring tools.
pub struct RenderedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Renderer-owned, depth-tested storage for uploaded geometry.
pub struct GeometryRenderer {
    instance: wgpu::Instance,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    uniform_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    uniform_stride: u64,
    uniform_capacity: usize,
    uniform_bytes: Vec<u8>,
    frame_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    lighting_bind_group: wgpu::BindGroup,
    shadow_frame_bind_group: wgpu::BindGroup,
    renderer_id: u64,
    geometry: SlotMap<GeometryKey, GpuMesh>,
    depth_view: wgpu::TextureView,
    shadow_view: wgpu::TextureView,
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
        Self::create(instance, Some(window), Some(surface), size).await
    }

    /// Creates a GPU renderer without a window, using the native render passes.
    pub async fn new_offscreen(width: u32, height: u32) -> Result<Self, GraphicsError> {
        capture_layout(width, height)?;
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        Self::create(instance, None, None, PhysicalSize::new(width, height)).await
    }

    async fn create(
        instance: wgpu::Instance,
        window: Option<Arc<Window>>,
        surface: Option<wgpu::Surface<'static>>,
        size: PhysicalSize<u32>,
    ) -> Result<Self, GraphicsError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: surface.as_ref(),
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
        let config = if let Some(surface) = &surface {
            let config = surface
                .get_default_config(&adapter, size.width, size.height)
                .ok_or(GraphicsError::UnsupportedSurface)?;
            surface.configure(&device, &config);
            config
        } else {
            wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            }
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Emergent geometry shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/geometry.wgsl").into()),
        });
        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Emergent directional shadow shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
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
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Emergent lighting frame uniform"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Emergent light buffer"),
            size: std::mem::size_of::<LightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_view = create_shadow_view(&device);
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Emergent shadow comparison sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let lighting_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Emergent lighting layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(
                            std::mem::size_of::<FrameUniform>() as u64
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(
                            std::mem::size_of::<LightUniform>() as u64
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Emergent lighting bind group"),
            layout: &lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });
        let shadow_frame_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Emergent shadow frame layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(
                            std::mem::size_of::<FrameUniform>() as u64
                        ),
                    },
                    count: None,
                }],
            });
        let shadow_frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Emergent shadow frame bind group"),
            layout: &shadow_frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Emergent geometry pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&lighting_layout)],
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
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Emergent directional shadow pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Emergent directional shadow pipeline layout"),
                    bind_group_layouts: &[Some(&uniform_layout), Some(&shadow_frame_layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: Some("shadow_vertex_main"),
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
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            fragment: None,
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
            shadow_pipeline,
            uniform_layout,
            uniform_buffer,
            uniform_bind_group,
            uniform_stride,
            uniform_capacity: INITIAL_INSTANCE_CAPACITY,
            uniform_bytes: vec![0; uniform_stride as usize * INITIAL_INSTANCE_CAPACITY],
            frame_buffer,
            light_buffer,
            lighting_bind_group,
            shadow_frame_bind_group,
            renderer_id: NEXT_RENDERER_ID.fetch_add(1, Ordering::Relaxed),
            geometry: SlotMap::with_key(),
            depth_view,
            shadow_view,
            info,
        })
    }

    pub fn info(&self) -> &RendererInfo {
        &self.info
    }

    pub fn insert(&mut self, geometry: &Geometry) -> GeometryId {
        self.insert_with_shading(geometry, ShadingMode::Smooth)
    }

    /// Uploads geometry with a normal-generation policy fixed for the handle's
    /// lifetime. Reinsert geometry to change its shading mode.
    pub fn insert_with_shading(&mut self, geometry: &Geometry, shading: ShadingMode) -> GeometryId {
        let (mut vertices, mut indices) = tessellate(geometry);
        if shading == ShadingMode::Flat {
            (vertices, indices) = flatten_triangles(&vertices, &indices);
        }
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
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        self.depth_view = create_depth_view(&self.device, size);
    }

    pub fn render(
        &mut self,
        instances: &[GeometryInstance],
    ) -> Result<RenderOutcome, GraphicsError> {
        self.render_internal(instances, &[])
    }

    /// Renders geometry with bounded Lambert lighting and an optional first
    /// shadow-casting directional light.
    pub fn render_lit(
        &mut self,
        instances: &[GeometryInstance],
        lights: &[LightInstance],
    ) -> Result<RenderOutcome, GraphicsError> {
        self.render_internal(instances, lights)
    }

    fn prepare_frame(
        &mut self,
        instances: &[GeometryInstance],
        lights: &[LightInstance],
    ) -> Result<bool, GraphicsError> {
        self.validate_instances(instances)?;
        let (light_uniform, light_view_projection, shadow_enabled) = self.prepare_lights(lights)?;
        self.ensure_uniform_capacity(instances.len())?;
        let aspect = self.config.width as f32 / self.config.height as f32;
        let view_matrix = look_at_rh(Vec3::new(7.0, 5.0, 9.0), Vec3::ZERO, Vec3::Y);
        let projection = perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        for (index, instance) in instances.iter().enumerate() {
            let uniform = ObjectUniform {
                model: instance.model.to_cols_array_2d(),
                model_view_projection: (projection * view_matrix * instance.model)
                    .to_cols_array_2d(),
                normal_matrix: normal_matrix(instance.model).to_cols_array_2d(),
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
        self.queue.write_buffer(
            &self.frame_buffer,
            0,
            bytemuck::bytes_of(&FrameUniform {
                light_view_projection: light_view_projection.to_cols_array_2d(),
                shadow_enabled: u32::from(shadow_enabled),
                _padding: [0; 7],
            }),
        );
        self.queue
            .write_buffer(&self.light_buffer, 0, bytemuck::bytes_of(&light_uniform));

        Ok(shadow_enabled)
    }

    fn encode_frame(
        &self,
        instances: &[GeometryInstance],
        surface_view: &wgpu::TextureView,
        shadow_enabled: bool,
    ) -> Result<wgpu::CommandEncoder, GraphicsError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Emergent geometry frame encoder"),
            });
        if shadow_enabled {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Emergent directional shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
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
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(1, &self.shadow_frame_bind_group, &[]);
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
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Emergent geometry pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
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
            pass.set_bind_group(1, &self.lighting_bind_group, &[]);
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
        Ok(encoder)
    }

    fn render_internal(
        &mut self,
        instances: &[GeometryInstance],
        lights: &[LightInstance],
    ) -> Result<RenderOutcome, GraphicsError> {
        let shadow_enabled = self.prepare_frame(instances, lights)?;
        let surface = self
            .surface
            .as_ref()
            .ok_or(GraphicsError::WrongRenderTarget)?;
        let window = self
            .window
            .as_ref()
            .ok_or(GraphicsError::WrongRenderTarget)?;
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(RenderOutcome::Skipped)
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                surface.configure(&self.device, &self.config);
                return Ok(RenderOutcome::Reconfigured);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                let surface = self
                    .instance
                    .create_surface(window.clone())
                    .map_err(|_| GraphicsError::SurfaceLost)?;
                surface.configure(&self.device, &self.config);
                self.surface = Some(surface);
                return Ok(RenderOutcome::Reconfigured);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(GraphicsError::SurfaceValidation)
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self.encode_frame(instances, &view, shadow_enabled)?;
        self.queue.submit(Some(encoder.finish()));
        window.pre_present_notify();
        self.queue.present(frame);
        Ok(RenderOutcome::Presented)
    }

    /// Captures one offscreen frame. This blocks for bounded GPU readback and
    /// allocates pixel buffers; call during authoring, not a steady-state game loop.
    pub fn render_lit_rgba(
        &mut self,
        instances: &[GeometryInstance],
        lights: &[LightInstance],
    ) -> Result<RenderedFrame, GraphicsError> {
        if self.surface.is_some() {
            return Err(GraphicsError::WrongRenderTarget);
        }
        let width = self.config.width;
        let height = self.config.height;
        let (row_bytes, padded_row_bytes) = capture_layout(width, height)?;
        let shadow_enabled = self.prepare_frame(instances, lights)?;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Emergent offscreen color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Emergent capture readback"),
            size: u64::from(padded_row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.encode_frame(instances, &view, shadow_enabled)?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(height),
                },
            },
            size,
        );
        let submission = self.queue.submit(Some(encoder.finish()));
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        let timeout = std::time::Duration::from_secs(10);
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(timeout),
            })
            .map_err(|error| GraphicsError::Readback(error.to_string()))?;
        receiver
            .recv_timeout(timeout)
            .map_err(|error| GraphicsError::Readback(error.to_string()))?
            .map_err(|error| GraphicsError::Readback(error.to_string()))?;
        let mapped = buffer
            .get_mapped_range(..)
            .map_err(|error| GraphicsError::Readback(error.to_string()))?;
        let mut rgba = Vec::with_capacity(row_bytes as usize * height as usize);
        for row in mapped.chunks_exact(padded_row_bytes as usize) {
            rgba.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(RenderedFrame {
            width,
            height,
            rgba,
        })
    }

    fn prepare_lights(
        &self,
        lights: &[LightInstance],
    ) -> Result<(LightUniform, Mat4, bool), GraphicsError> {
        if lights.len() > MAX_LIGHTS {
            return Err(GraphicsError::TooManyLights(lights.len()));
        }
        let mut result = LightUniform::zeroed();
        let mut shadow_view_projection = Mat4::IDENTITY;
        let mut shadow_selected = false;
        for (index, light) in lights.iter().copied().enumerate() {
            if !light.model.is_finite() {
                return Err(GraphicsError::NonFiniteLightMatrix { light: index });
            }
            let source = light.source;
            let kind = match source.kind() {
                LightKind::Ambient => 0.0,
                LightKind::Directional => 1.0,
                LightKind::Point => 2.0,
                LightKind::Spot => 3.0,
            };
            let raw_direction = light.model.transform_vector3(Vec3::NEG_Z);
            let needs_direction = matches!(source.kind(), LightKind::Directional | LightKind::Spot);
            if needs_direction
                && (!raw_direction.is_finite() || raw_direction.length_squared() < 1.0e-10)
            {
                return Err(GraphicsError::InvalidLightDirection { light: index });
            }
            let direction = if raw_direction.length_squared() > 1.0e-10 {
                raw_direction.normalize()
            } else {
                Vec3::NEG_Z
            };
            let [inner, outer] = source.spot_half_angles().unwrap_or([0.0, 0.0]);
            let casts_shadow = source.kind() == LightKind::Directional
                && source.shadow_casting() == ShadowCasting::Enabled
                && !shadow_selected;
            if casts_shadow {
                shadow_view_projection = directional_shadow_matrix(direction);
                shadow_selected = true;
            }
            let color = source.color().components();
            let position = light.model.transform_point3(Vec3::ZERO);
            result.lights[index] = GpuLight {
                position_kind: [position.x, position.y, position.z, kind],
                color_intensity: [color[0], color[1], color[2], source.intensity()],
                direction_range: [
                    direction.x,
                    direction.y,
                    direction.z,
                    source.range().unwrap_or(0.0),
                ],
                spot_and_shadow: [
                    inner.cos(),
                    outer.cos(),
                    0.0,
                    if casts_shadow { 1.0 } else { 0.0 },
                ],
            };
        }
        result.count = lights.len() as u32;
        Ok((result, shadow_view_projection, shadow_selected))
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

fn capture_layout(width: u32, height: u32) -> Result<(u32, u32), GraphicsError> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(GraphicsError::InvalidCaptureSize { width, height });
    }
    let row_bytes = width * 4;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    Ok((row_bytes, row_bytes.div_ceil(alignment) * alignment))
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
    let (mut vertices, indices) = match geometry {
        Geometry::Point(point) => tessellate_point(*point),
        Geometry::Line(line) => tessellate_line(*line),
        Geometry::Polygon(polygon) => tessellate_polygon(polygon),
        Geometry::Mesh(mesh) => tessellate_mesh(mesh),
        Geometry::Primitive(primitive) => {
            let mesh = primitive
                .to_mesh()
                .expect("validated primitive must produce a valid mesh");
            tessellate_mesh(&mesh)
        }
    };
    complete_normals(&mut vertices, &indices);
    (vertices, indices)
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
            normal: Vec3::ZERO.to_array(),
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
            normal: Vec3::ZERO.to_array(),
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
            normal: Vec3::ZERO.to_array(),
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
            normal: vertex.normal().unwrap_or(Vec3::ZERO).to_array(),
        })
        .collect();
    (vertices, mesh.indices().to_vec())
}

fn complete_normals(vertices: &mut [Vertex], indices: &[u32]) {
    let mut generated = vec![Vec3::ZERO; vertices.len()];
    let (triangles, remainder) = indices.as_chunks::<3>();
    debug_assert!(remainder.is_empty());
    for triangle in triangles {
        let positions = [
            Vec3::from_array(vertices[triangle[0] as usize].position),
            Vec3::from_array(vertices[triangle[1] as usize].position),
            Vec3::from_array(vertices[triangle[2] as usize].position),
        ];
        let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
        if normal.length_squared() > 1.0e-12 {
            for index in triangle {
                if Vec3::from_array(vertices[*index as usize].normal).length_squared() <= 1.0e-12 {
                    generated[*index as usize] += normal;
                }
            }
        }
    }
    for (index, vertex) in vertices.iter_mut().enumerate() {
        let normal = Vec3::from_array(vertex.normal);
        let completed = if normal.length_squared() > 1.0e-12 {
            normal
        } else {
            generated[index]
        };
        vertex.normal = if completed.length_squared() > 1.0e-12 {
            completed.normalize().to_array()
        } else {
            Vec3::Y.to_array()
        };
    }
}

fn flatten_triangles(vertices: &[Vertex], indices: &[u32]) -> (Vec<Vertex>, Vec<u32>) {
    let (triangles, remainder) = indices.as_chunks::<3>();
    debug_assert!(remainder.is_empty());
    let mut flattened_vertices = Vec::with_capacity(indices.len());
    let mut flattened_indices = Vec::with_capacity(indices.len());
    for triangle in triangles {
        let first = flattened_vertices.len() as u32;
        let positions = [
            Vec3::from_array(vertices[triangle[0] as usize].position),
            Vec3::from_array(vertices[triangle[1] as usize].position),
            Vec3::from_array(vertices[triangle[2] as usize].position),
        ];
        let normal = (positions[1] - positions[0])
            .cross(positions[2] - positions[0])
            .normalize();
        for index in triangle {
            let mut vertex = vertices[*index as usize];
            vertex.normal = normal.to_array();
            flattened_vertices.push(vertex);
        }
        flattened_indices.extend_from_slice(&[first, first + 1, first + 2]);
    }
    (flattened_vertices, flattened_indices)
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

fn orthographic_rh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(2.0 / (right - left), 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / (top - bottom), 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0 / (near - far), 0.0),
        Vec4::new(
            (left + right) / (left - right),
            (top + bottom) / (bottom - top),
            near / (near - far),
            1.0,
        ),
    )
}

fn directional_shadow_matrix(direction: Vec3) -> Mat4 {
    let up = if direction.dot(Vec3::Y).abs() > 0.95 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let view = look_at_rh(-direction * SHADOW_DISTANCE, Vec3::ZERO, up);
    orthographic_rh(
        -SHADOW_EXTENT,
        SHADOW_EXTENT,
        -SHADOW_EXTENT,
        SHADOW_EXTENT,
        0.1,
        SHADOW_DISTANCE * 2.5,
    ) * view
}

fn normal_matrix(model: Mat4) -> Mat4 {
    let determinant = glam::Mat3::from_mat4(model).determinant();
    if determinant.is_finite() && determinant.abs() > 1.0e-8 {
        model.inverse().transpose()
    } else {
        Mat4::IDENTITY
    }
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

fn create_shadow_view(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Emergent directional shadow texture"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

#[cfg(test)]
mod tests {
    #[test]
    fn capture_layout_is_bounded_and_gpu_aligned() {
        assert_eq!(super::capture_layout(1, 1).unwrap(), (4, 256));
        assert_eq!(super::capture_layout(65, 2).unwrap(), (260, 512));
        assert_eq!(super::capture_layout(1280, 720).unwrap(), (5120, 5120));
        assert!(super::capture_layout(0, 1).is_err());
        assert!(super::capture_layout(1, 4097).is_err());
    }
    use super::*;
    use crate::{Color, MeshVertex, Primitive};

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
    fn orthographic_shadow_projection_maps_near_and_far_depth_to_webgpu_range() {
        let projection = orthographic_rh(-2.0, 2.0, -2.0, 2.0, 0.1, 40.0);
        let near = projection * Vec4::new(0.0, 0.0, -0.1, 1.0);
        let far = projection * Vec4::new(0.0, 0.0, -40.0, 1.0);
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
        let primitive = Geometry::from(Primitive::sphere(1.0, 12, 6, Color::WHITE).unwrap());
        for geometry in [&point, &line, &polygon, &mesh, &primitive] {
            let (vertices, indices) = tessellate(geometry);
            assert!(!vertices.is_empty());
            assert!(!indices.is_empty());
            assert!(indices.len().is_multiple_of(3));
            assert!(indices
                .iter()
                .all(|index| (*index as usize) < vertices.len()));
            assert!(vertices.iter().all(|vertex| {
                let normal = Vec3::from_array(vertex.normal);
                normal.is_finite() && (normal.length() - 1.0).abs() < 1.0e-4
            }));
        }
    }

    #[test]
    fn flat_shading_expands_triangles_and_uses_one_normal_per_face() {
        let mesh = Mesh::cube(1.0).unwrap();
        let (vertices, indices) = tessellate(&Geometry::Mesh(mesh));
        let (flat_vertices, flat_indices) = flatten_triangles(&vertices, &indices);

        assert_eq!(flat_vertices.len(), indices.len());
        assert_eq!(flat_indices.len(), indices.len());
        assert_eq!(flat_indices, (0..indices.len() as u32).collect::<Vec<_>>());
        let (faces, remainder) = flat_vertices.as_chunks::<3>();
        assert!(remainder.is_empty());
        for face in faces {
            let normal = face[0].normal;
            assert_eq!(face[1].normal, normal);
            assert_eq!(face[2].normal, normal);
            assert!((Vec3::from_array(normal).length() - 1.0).abs() < 1.0e-5);
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
