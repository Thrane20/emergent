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

use crate::{
    CameraProjection, Geometry, LightKind, LightSource, Line, Mesh, Point, Polygon, ShadowCasting,
};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const INITIAL_INSTANCE_CAPACITY: usize = 64;
// Eight authored lights plus the scene ambient light.
const MAX_LIGHTS: usize = 9;
const SHADOW_MAP_SIZE: u32 = 2048;
const POINT_SHADOW_SIZE: u32 = 512;
const MAX_POINT_SHADOWS: usize = 2;
// Keep the frame array length in shaders/geometry.wgsl in sync.
pub(crate) const POINT_SHADOW_FACES: usize = MAX_POINT_SHADOWS * 6;
const SHADOW_DISTANCE: f32 = 22.0;
const SHADOW_EXTENT: f32 = 14.0;
static NEXT_RENDERER_ID: AtomicU64 = AtomicU64::new(1);

new_key_type! {
    struct GeometryKey;
    struct RaymarchedTerrainKey;
}

/// Renderer-scoped generational identity for an implicit terrain resource.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RaymarchedTerrainId {
    renderer: u64,
    key: RaymarchedTerrainKey,
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
    uv: [f32; 2],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x4, 2 => Float32x3, 3 => Float32x2
    ];

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
    material: MaterialUniform,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    albedo: [f32; 4],
    specular: [f32; 4],
    emission: [f32; 4],
    // Matches WGSL: bump strength, unlit, cast shadows, receive shadows.
    params: [f32; 4],
    map_strength: [f32; 4],
    bounds_min: [f32; 4],
    bounds_size: [f32; 4],
    camera: [f32; 4],
    diffuse: [f32; 4],
    texture_tiling: [f32; 4],
    shader_time: [f32; 4],
    shader_parameters: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    light_view_projection: [[f32; 4]; 4],
    shadow_enabled: u32,
    _padding: [u32; 7],
    point_view_projections: [[[f32; 4]; 4]; POINT_SHADOW_FACES],
}

#[derive(Clone, Copy)]
struct ShadowPasses {
    directional: bool,
    point_faces: usize,
}

struct PreparedLights {
    uniform: LightUniform,
    directional_matrix: Mat4,
    point_matrices: [Mat4; POINT_SHADOW_FACES],
    passes: ShadowPasses,
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
    shader_pipeline: Option<wgpu::RenderPipeline>,
    transparent: bool,
    overlay: Option<[f32; 4]>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    material: MaterialUniform,
    material_bind_group: wgpu::BindGroup,
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
    InvalidView,
    TooManyPointShadows,
    InvalidPointShadow { light: usize },
    Material(String),
}

impl fmt::Display for GraphicsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyPointShadows => write!(formatter, "at most {MAX_POINT_SHADOWS} point lights can cast shadows"),
            Self::InvalidPointShadow { light } => write!(formatter, "point shadow light {light} requires range 0.01..1000000 and finite world coordinates within +/-1000000"),
            Self::Material(error) => write!(formatter, "material error: {error}"),
            Self::InvalidView => formatter
                .write_str("camera eye, target and up must define a finite, non-degenerate view"),
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
    gpu_profiler: Option<crate::gpu_profile::GpuProfiler>,
    raymarched_options: crate::RaymarchedRenderOptions,
    raymarched_regions: SlotMap<
        RaymarchedTerrainKey,
        (
            crate::raymarched_terrain_graphics::RaymarchedTerrainPass,
            Mat4,
        ),
    >,
    raymarched_terrain: Option<crate::raymarched_terrain_graphics::RaymarchedTerrainPass>,
    particles: crate::particle_graphics::ParticlePass,
    skybox: Option<crate::skybox_graphics::SkyboxPass>,
    background: [f32; 3],
    view_matrix: Mat4,
    projection: CameraProjection,
    instance: wgpu::Instance,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    surface_layout: wgpu::PipelineLayout,
    shader_time: f32,
    transparent_order: Vec<usize>,
    overlay_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    uniform_layout: wgpu::BindGroupLayout,
    material_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    uniform_stride: u64,
    uniform_capacity: usize,
    uniform_bytes: Vec<u8>,
    frame_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    lighting_bind_group: wgpu::BindGroup,
    shadow_frame_bind_group: wgpu::BindGroup,
    shadow_matrix_buffer: wgpu::Buffer,
    shadow_matrix_stride: u64,
    shadow_matrix_bytes: Vec<u8>,
    point_shadow_views: [wgpu::TextureView; POINT_SHADOW_FACES],
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
                required_features: crate::gpu_profile::timestamp_features(adapter.features()),
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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
        let (point_shadow_array, point_shadow_views) = create_point_shadow_views(&device);
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&point_shadow_array),
                },
            ],
        });
        let shadow_matrix_stride = align_up(
            64,
            device.limits().min_uniform_buffer_offset_alignment as u64,
        );
        let shadow_matrix_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Emergent shadow face matrices"),
            size: shadow_matrix_stride * (1 + POINT_SHADOW_FACES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_frame_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Emergent shadow matrix layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(64),
                    },
                    count: None,
                }],
            });
        let shadow_frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Emergent shadow matrices"),
            layout: &shadow_frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &shadow_matrix_buffer,
                    offset: 0,
                    size: NonZeroU64::new(64),
                }),
            }],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Emergent packed material noise layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Emergent geometry pipeline layout"),
            bind_group_layouts: &[
                Some(&uniform_layout),
                Some(&lighting_layout),
                Some(&material_layout),
            ],
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
        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Emergent UI overlay pipeline"),
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
            depth_stencil: None,
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

        let particles =
            crate::particle_graphics::ParticlePass::new(&device, config.format, &depth_view);
        Ok(Self {
            gpu_profiler: None,
            raymarched_regions: SlotMap::with_key(),
            raymarched_terrain: None,
            raymarched_options: crate::RaymarchedRenderOptions::default(),
            particles,
            skybox: None,
            background: crate::EnvironmentDefinition::default().background_color,
            projection: CameraProjection::perspective(45.0, 0.1, 100.0)
                .expect("default camera projection is valid"),
            view_matrix: look_at_rh(Vec3::new(7.0, 5.0, 9.0), Vec3::ZERO, Vec3::Y),
            instance,
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            surface_layout: pipeline_layout,
            shader_time: 0.,
            transparent_order: Vec::new(),
            overlay_pipeline,
            shadow_pipeline,
            uniform_layout,
            material_layout,
            uniform_buffer,
            uniform_bind_group,
            uniform_stride,
            uniform_capacity: INITIAL_INSTANCE_CAPACITY,
            uniform_bytes: vec![0; uniform_stride as usize * INITIAL_INSTANCE_CAPACITY],
            frame_buffer,
            light_buffer,
            lighting_bind_group,
            shadow_frame_bind_group,
            shadow_matrix_buffer,
            shadow_matrix_stride,
            shadow_matrix_bytes: vec![0; shadow_matrix_stride as usize * (1 + POINT_SHADOW_FACES)],
            point_shadow_views,
            renderer_id: NEXT_RENDERER_ID.fetch_add(1, Ordering::Relaxed),
            geometry: SlotMap::with_key(),
            depth_view,
            shadow_view,
            info,
        })
    }

    /// Upload shared sprites during scene preparation, outside the steady-state frame loop.
    pub fn prepare_particle_effect(&mut self, effect: &crate::ParticleEffect) {
        self.particles
            .prepare_effect(&self.device, &self.queue, effect);
    }
    /// Extract and upload the current particle frame. Call after setting the camera.
    pub fn set_particles(&mut self, store: &crate::ParticleStore) {
        self.particles.prepare(
            &self.device,
            &self.queue,
            store,
            self.view_matrix,
            self.view_projection(),
        );
    }
    pub fn clear_particles(&mut self) {
        self.particles.clear();
    }

    /// Uploads a resolved scene environment outside the frame loop.
    pub fn set_environment(&mut self, environment: &crate::SceneEnvironment) {
        self.background = environment.settings.background_color;
        self.skybox = environment.skybox().map(|image| {
            crate::skybox_graphics::SkyboxPass::new(
                &self.device,
                &self.queue,
                self.config.format,
                image,
                environment.settings.skybox_rotation,
                environment.settings.skybox_intensity,
            )
        });
    }

    /// Experimental single world-origin field. Prepare outside the frame loop.
    /// This visual prototype has no physics collider or scene-asset contract yet.
    pub fn set_raymarched_terrain(
        &mut self,
        field: Option<crate::RaymarchedTerrain>,
    ) -> Result<(), &'static str> {
        if let Some(field) = field {
            field.validate()?;
        }
        self.raymarched_terrain = field.map(|field| {
            crate::raymarched_terrain_graphics::RaymarchedTerrainPass::new(
                &self.device,
                self.config.format,
                field,
            )
        });
        Ok(())
    }

    /// Prepare a scene field outside the frame loop. At most eight regions are supported.
    pub fn insert_raymarched_terrain(
        &mut self,
        field: crate::RaymarchedTerrain,
    ) -> Result<RaymarchedTerrainId, &'static str> {
        field.validate()?;
        if self.raymarched_regions.len() >= 8 {
            return Err("Scene supports at most eight ray-marched terrain regions.");
        }
        let pass = crate::raymarched_terrain_graphics::RaymarchedTerrainPass::new(
            &self.device,
            self.config.format,
            field,
        );
        Ok(RaymarchedTerrainId {
            renderer: self.renderer_id,
            key: self.raymarched_regions.insert((pass, Mat4::IDENTITY)),
        })
    }
    pub fn transform_raymarched_terrain(
        &mut self,
        id: RaymarchedTerrainId,
        model: Mat4,
    ) -> Result<(), &'static str> {
        if id.renderer != self.renderer_id {
            return Err("Ray-marched terrain belongs to another renderer.");
        }
        if !model.is_finite() || !model.inverse().is_finite() || model.row(3) != Vec4::W {
            return Err("Ray-marched terrain requires an invertible finite transform.");
        }
        let entry = self
            .raymarched_regions
            .get_mut(id.key)
            .ok_or("Ray-marched terrain handle is stale.")?;
        entry.1 = model;
        Ok(())
    }
    pub fn remove_raymarched_terrain(&mut self, id: RaymarchedTerrainId) -> bool {
        id.renderer == self.renderer_id && self.raymarched_regions.remove(id.key).is_some()
    }

    /// Sets terrain-only diagnostics without mutating stored field recipes.
    pub fn set_raymarched_render_options(&mut self, options: crate::RaymarchedRenderOptions) {
        self.raymarched_options = options;
    }

    /// Returns last-prepared terrain submission counts, not GPU fragment counts.
    pub fn raymarched_frame_stats(&self) -> crate::RaymarchedFrameStats {
        let mut stats = crate::RaymarchedFrameStats::default();
        for field in self
            .raymarched_terrain
            .iter()
            .chain(self.raymarched_regions.values().map(|(field, _)| field))
        {
            stats.regions += 1;
            if let Some([_, _, width, height]) = field.scissor() {
                stats.submitted += 1;
                stats.scissor_pixels += u64::from(width) * u64::from(height);
            }
        }
        stats
    }

    pub fn info(&self) -> &RendererInfo {
        &self.info
    }

    /// Current camera pose in world space.
    pub fn view_transform(&self) -> Mat4 {
        self.view_matrix.inverse()
    }

    pub fn projection(&self) -> CameraProjection {
        self.projection
    }

    /// The exact WebGPU projection used by rendering, for inspection tools.
    pub fn view_projection(&self) -> Mat4 {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let projection = match self.projection {
            CameraProjection::Perspective {
                vertical_fov_radians,
                near,
                far,
            } => perspective_rh(vertical_fov_radians, aspect, near, far),
            CameraProjection::Isometric {
                vertical_size,
                near,
                far,
            } => {
                let half_height = vertical_size * 0.5;
                let half_width = half_height * aspect;
                orthographic_rh(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    near,
                    far,
                )
            }
        };
        projection * self.view_matrix
    }

    /// Changes the view transform without rebuilding geometry or world state.
    pub fn set_view(&mut self, eye: Vec3, target: Vec3, up: Vec3) -> Result<(), GraphicsError> {
        self.view_matrix = validated_view(eye, target, up)?;
        self.projection = self
            .projection
            .with_far_at_least(((eye - target).length() * 8.).clamp(100., 1_000_000.));
        Ok(())
    }

    pub fn set_projection(&mut self, projection: CameraProjection) {
        self.projection = projection;
    }

    pub fn set_camera(
        &mut self,
        eye: Vec3,
        target: Vec3,
        up: Vec3,
        projection: CameraProjection,
    ) -> Result<(), GraphicsError> {
        self.view_matrix = validated_view(eye, target, up)?;
        self.projection = projection;
        Ok(())
    }

    pub fn insert(&mut self, geometry: &Geometry) -> GeometryId {
        self.insert_with_shading(geometry, ShadingMode::Smooth)
    }

    /// Uploads geometry with a normal-generation policy fixed for the handle's
    /// lifetime. Reinsert geometry to change its shading mode.
    pub fn insert_with_shading(&mut self, geometry: &Geometry, shading: ShadingMode) -> GeometryId {
        self.upload_material(geometry, shading, &crate::Material::default())
            .expect("constant default material is valid")
    }

    /// Uploads a resolved material and its bounded noise maps once.
    pub fn insert_with_material(
        &mut self,
        geometry: &Geometry,
        material: &crate::Material,
    ) -> Result<GeometryId, GraphicsError> {
        let shading = if material.terrain.is_none()
            && material.definition().shading == crate::MaterialShading::Flat
        {
            ShadingMode::Flat
        } else {
            ShadingMode::Smooth
        };
        self.upload_material(geometry, shading, material)
    }

    fn upload_material(
        &mut self,
        geometry: &Geometry,
        shading: ShadingMode,
        material: &crate::Material,
    ) -> Result<GeometryId, GraphicsError> {
        let layers: Vec<_> = material
            .terrain
            .as_ref()
            .map_or_else(|| vec![material], |t| t.layers.iter().collect());
        let volumes = layers
            .iter()
            .map(|layer| layer.bake())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| GraphicsError::Material(e.to_string()))?;
        let side = volumes.iter().map(|v| v.0).max().unwrap_or(1);
        let mut pixels = Vec::with_capacity((side as usize).pow(3) * 8 * layers.len());
        for (source_side, source) in &volumes {
            for z in 0..side {
                for y in 0..side {
                    for x in 0..side {
                        let coordinate = |n: u32| {
                            if side == 1 {
                                0
                            } else {
                                n * (source_side - 1) / (side - 1)
                            }
                        };
                        let index = ((coordinate(z) * source_side * source_side
                            + coordinate(y) * source_side
                            + coordinate(x))
                            * 8) as usize;
                        pixels.extend_from_slice(&source[index..index + 8]);
                    }
                }
            }
        }
        let shader_pipeline = material
            .shader()
            .map(|graph| self.compile_surface(graph))
            .transpose()?;
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
        let (material_uniform, material_bind_group) =
            self.upload_noise(material, &layers, side, &pixels, &vertices);
        let key = self.geometry.insert(GpuMesh {
            shader_pipeline,
            transparent: material.shader().is_some_and(|g| g.transparent),
            overlay: None,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            material: material_uniform,
            material_bind_group,
        });
        Ok(GeometryId {
            renderer: self.renderer_id,
            key,
        })
    }

    /// Prepares a static UI texture/quad. No raster work occurs in rendering.
    pub fn insert_ui_surface(
        &mut self,
        surface: &crate::UiSurface,
    ) -> Result<GeometryId, GraphicsError> {
        let (geometry, material) = surface.prepare().map_err(GraphicsError::Material)?;
        let id = self.insert_with_material(&geometry, &material)?;
        if surface.screen {
            self.geometry[id.key].overlay =
                Some([surface.width, surface.height, surface.x, surface.y]);
        }
        Ok(id)
    }

    fn upload_noise(
        &self,
        material: &crate::Material,
        layers: &[&crate::Material],
        side: u32,
        pixels: &[u8],
        vertices: &[Vertex],
    ) -> (MaterialUniform, wgpu::BindGroup) {
        let count = layers.len() as u32;
        let size = wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: side * count,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Material noise atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(side * 8),
                rows_per_image: Some(side),
            },
            size,
        );
        let view = texture.create_view(&Default::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let terrain = material.terrain.as_ref();
        let image = material.diffuse_image();
        // shortcut: terrain images share a 1024-pixel array budget; add streaming textures when larger terrain workloads require it.
        let (width, height) = if terrain.is_some() {
            let side = layers
                .iter()
                .filter_map(|layer| layer.diffuse_image())
                .map(|image| image.width().max(image.height()))
                .max()
                .unwrap_or(1)
                .min(1024);
            (side, side)
        } else {
            image.map_or((1, 1), |image| (image.width(), image.height()))
        };
        let mips = width.max(height).ilog2() + 1;
        let diffuse = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Material diffuse array"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: count,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, material) in layers.iter().enumerate() {
            if let Some(image) = material.diffuse_image() {
                let image = if image.width() == width && image.height() == height {
                    image.clone()
                } else {
                    image.resized(width, height)
                };
                for (level, image) in image.mip_levels().iter().enumerate() {
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &diffuse,
                            mip_level: level as u32,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: layer as u32,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        image.rgba(),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(image.width() * 4),
                            rows_per_image: Some(image.height()),
                        },
                        wgpu::Extent3d {
                            width: image.width(),
                            height: image.height(),
                            depth_or_array_layers: 1,
                        },
                    );
                }
            } else {
                for level in 0..mips {
                    let w = (width >> level).max(1);
                    let h = (height >> level).max(1);
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &diffuse,
                            mip_level: level,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: layer as u32,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &vec![255; (w * h * 4) as usize],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(w * 4),
                            rows_per_image: Some(h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }
        }
        let diffuse_view = diffuse.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let diffuse_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Diffuse trilinear repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let resolution = terrain.map_or(1, |t| t.paint.resolution);
        let weights = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Terrain material weights"),
            size: wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &weights,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            terrain.map_or(&[255, 0, 0, 0][..], |t| t.paint.weights.as_slice()),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(resolution * 4),
                rows_per_image: Some(resolution),
            },
            wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: 1,
            },
        );
        let weights_view = weights.create_view(&Default::default());
        let mut uniforms = [MaterialUniform::zeroed(); 4];
        for (index, layer) in layers.iter().enumerate() {
            uniforms[index] = Self::material_uniform(layer, vertices);
            uniforms[index].bounds_min[3] = f32::from(
                terrain.is_some() && layer.definition().shading == crate::MaterialShading::Flat,
            );
        }
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Terrain material settings"),
                contents: bytemuck::cast_slice(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Surface material"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&diffuse_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&weights_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: buffer.as_entire_binding(),
                },
            ],
        });
        let mut uniform = Self::material_uniform(material, vertices);
        uniform.diffuse[2] = count as f32;
        uniform.diffuse[3] = side as f32;
        if terrain.is_some() {
            uniform.params[2] = f32::from(layers.iter().any(|m| m.definition().cast_shadows));
            uniform.params[3] = 1.;
        }
        (uniform, bind_group)
    }

    fn material_uniform(material: &crate::Material, vertices: &[Vertex]) -> MaterialUniform {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in vertices {
            let p = Vec3::from_array(vertex.position);
            min = min.min(p);
            max = max.max(p);
        }
        let d = material.definition();
        let albedo = d.albedo.unwrap_or([1.0; 3]);
        MaterialUniform {
            albedo: [
                albedo[0],
                albedo[1],
                albedo[2],
                f32::from(d.albedo.is_some()),
            ],
            specular: [d.specular[0], d.specular[1], d.specular[2], d.shininess],
            emission: [
                d.emission[0],
                d.emission[1],
                d.emission[2],
                d.emission_intensity,
            ],
            params: [
                d.bump_strength,
                f32::from(d.shading == crate::MaterialShading::Unlit),
                f32::from(d.cast_shadows),
                f32::from(d.receive_shadows),
            ],
            map_strength: d.maps().map(|m| m.map_or(0.0, |m| m.strength)),
            bounds_min: min.extend(0.0).to_array(),
            bounds_size: (max - min).max(Vec3::splat(0.0001)).extend(0.0).to_array(),
            camera: [0.0; 4],
            diffuse: [
                // 0: no image, 1: triplanar projection, 2: mesh texture coordinates.
                match (material.diffuse_image().is_some(), material.uv_mapped()) {
                    (false, _) => 0.,
                    (true, false) => 1.,
                    (true, true) => 2.,
                },
                d.active_diffuse_map().map_or(1., |map| map.scale),
                0.,
                0.,
            ],
            texture_tiling: [d.texture_tiling[0], d.texture_tiling[1], 0., 0.],
            shader_time: [0.; 4],
            shader_parameters: material.shader().map_or([[0.; 4]; 4], |g| {
                g.parameter_values(&d.shader_parameters)
                    .expect("resolved shader values")
            }),
        }
    }

    /// Prepares a replacement off the draw path, retaining the existing geometry handle on success.
    pub fn replace_material(
        &mut self,
        id: GeometryId,
        geometry: &Geometry,
        material: &crate::Material,
    ) -> Result<(), GraphicsError> {
        if !self.contains(id) {
            return Err(GraphicsError::MissingGeometry(id));
        }
        let prepared = self.insert_with_material(geometry, material)?;
        let mesh = self
            .geometry
            .remove(prepared.key)
            .expect("new geometry handle");
        self.geometry[id.key] = mesh;
        Ok(())
    }
    /// Explicit animation time; invalid host input leaves the last valid time intact.
    pub fn set_shader_time(&mut self, seconds: f32) -> bool {
        if !seconds.is_finite() || !(0.0..=1_000_000.0).contains(&seconds) {
            return false;
        }
        self.shader_time = seconds;
        true
    }
    fn compile_surface(
        &self,
        graph: &crate::ShaderDefinition,
    ) -> Result<wgpu::RenderPipeline, GraphicsError> {
        let graph_code = graph
            .wgsl()
            .map_err(|e| GraphicsError::Material(e.to_string()))?;
        let base = include_str!("shaders/geometry.wgsl");
        let begin = base.find("// SURFACE_GRAPH_BEGIN").expect("shader marker");
        let end = base.find("// SURFACE_GRAPH_END").expect("shader marker");
        let source=format!("{}{}{}",&base[..begin],graph_code,&base[end..])
            .replace("let position = input.position; // SURFACE_VERTEX", "let displaced=surface_graph(input.position,input.uv,1.0);let position=input.position+input.normal*displaced.displacement;")
            .replace("// SURFACE_FRAGMENT", "let graph=surface_graph(input.local_position,input.uv,dot(normalize(input.normal),normalize(object.material.camera.xyz-input.world_position)));albedo=graph.color;emission=graph.color*graph.emission;alpha*=graph.opacity;if alpha<0.001 {discard;}");
        let displaced = graph
            .nodes
            .iter()
            .find(|n| n.id == graph.outputs.displacement)
            .is_some_and(|n| n.op != crate::ShaderOp::Constant || n.value != 0.);
        let source = if displaced {
            source.replace("var surface_normal = normalize(input.normal);", "var surface_normal = normalize(cross(dpdx(input.world_position),dpdy(input.world_position)));if dot(surface_normal,input.normal)<0.0 {surface_normal = -surface_normal;}")
        } else {
            source
        };
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Surface graph"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Emergent geometry pipeline"),
                layout: Some(&self.surface_layout),
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
                    depth_write_enabled: Some(!graph.transparent),
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
                        format: self.config.format,
                        blend: graph
                            .transparent
                            .then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        if let Some(error) = pollster::block_on(scope.pop()) {
            return Err(GraphicsError::Material(error.to_string()));
        }
        Ok(pipeline)
    }

    pub fn remove(&mut self, geometry: GeometryId) -> bool {
        geometry.renderer == self.renderer_id && self.geometry.remove(geometry.key).is_some()
    }
    pub fn contains(&self, geometry: GeometryId) -> bool {
        geometry.renderer == self.renderer_id && self.geometry.contains_key(geometry.key)
    }

    /// Uploaded vertex/index bytes for a live, renderer-scoped handle. Excludes
    /// material textures, uniforms, transient staging and driver allocation overhead.
    pub fn geometry_buffer_bytes(&self, geometry: GeometryId) -> Option<u64> {
        if geometry.renderer != self.renderer_id {
            return None;
        }
        self.geometry
            .get(geometry.key)
            .map(|mesh| mesh.vertex_buffer.size() + mesh.index_buffer.size())
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.gpu_profiler = None;
        self.config.width = size.width;
        self.config.height = size.height;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        self.depth_view = create_depth_view(&self.device, size);
        self.particles.resize_depth(&self.device, &self.depth_view);
    }

    pub fn render(
        &mut self,
        instances: &[GeometryInstance],
    ) -> Result<RenderOutcome, GraphicsError> {
        self.render_internal(instances, &[])
    }

    /// Renders geometry with bounded Lambert lighting and an optional first
    /// shadow-casting directional light plus up to two shadow-casting point lights.
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
    ) -> Result<ShadowPasses, GraphicsError> {
        self.validate_instances(instances)?;
        let prepared = self.prepare_lights(lights)?;
        self.ensure_uniform_capacity(instances.len())?;
        self.transparent_order.clear();
        self.transparent_order
            .extend(instances.iter().enumerate().filter_map(|(i, instance)| {
                self.geometry[instance.geometry.key]
                    .transparent
                    .then_some(i)
            }));
        let eye = self.view_matrix.inverse().transform_point3(Vec3::ZERO);
        self.transparent_order.sort_unstable_by(|&a, &b| {
            let distance = |i: usize| {
                instances[i]
                    .model
                    .transform_point3(Vec3::ZERO)
                    .distance_squared(eye)
            };
            distance(b).total_cmp(&distance(a))
        });
        let view_projection = self.view_projection();
        let viewport = [self.config.width, self.config.height];
        if let Some(field) = &mut self.raymarched_terrain {
            field.prepare(
                &self.queue,
                view_projection,
                Mat4::IDENTITY,
                viewport,
                self.raymarched_options,
            );
        }
        for (field, model) in self.raymarched_regions.values_mut() {
            field.prepare(
                &self.queue,
                view_projection,
                *model,
                viewport,
                self.raymarched_options,
            );
        }
        if let Some(skybox) = &self.skybox {
            skybox.prepare(&self.queue, view_projection);
        }
        let camera = self
            .view_matrix
            .inverse()
            .transform_point3(Vec3::ZERO)
            .extend(1.0)
            .to_array();
        for (index, instance) in instances.iter().enumerate() {
            let overlay = self.geometry[instance.geometry.key].overlay;
            let model = if let Some([w, h, x, y]) = overlay {
                Mat4::from_scale_rotation_translation(
                    Vec3::new(
                        if w > 0.0 {
                            w
                        } else {
                            (self.config.width as f32 - x).max(0.001)
                        },
                        if h > 0.0 {
                            h
                        } else {
                            (self.config.height as f32 - y).max(0.001)
                        },
                        1.0,
                    ),
                    crate::Quat::IDENTITY,
                    Vec3::new(x, y, 0.0),
                )
            } else {
                instance.model
            };
            let projection = if overlay.is_some() {
                orthographic_rh(
                    0.0,
                    self.config.width as f32,
                    self.config.height as f32,
                    0.0,
                    -1.0,
                    1.0,
                )
            } else {
                view_projection
            };
            let mut material = self.geometry[instance.geometry.key].material;
            material.camera = camera;
            material.shader_time[0] = self.shader_time;
            let uniform = ObjectUniform {
                model: model.to_cols_array_2d(),
                model_view_projection: (projection * model).to_cols_array_2d(),
                normal_matrix: normal_matrix(model).to_cols_array_2d(),
                material,
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
                light_view_projection: prepared.directional_matrix.to_cols_array_2d(),
                shadow_enabled: u32::from(prepared.passes.directional),
                _padding: [0; 7],
                point_view_projections: prepared.point_matrices.map(|m| m.to_cols_array_2d()),
            }),
        );
        self.queue
            .write_buffer(&self.light_buffer, 0, bytemuck::bytes_of(&prepared.uniform));
        for (index, matrix) in std::iter::once(prepared.directional_matrix)
            .chain(prepared.point_matrices)
            .enumerate()
        {
            let offset = index * self.shadow_matrix_stride as usize;
            self.shadow_matrix_bytes[offset..offset + 64]
                .copy_from_slice(bytemuck::bytes_of(&matrix.to_cols_array_2d()));
        }
        self.queue
            .write_buffer(&self.shadow_matrix_buffer, 0, &self.shadow_matrix_bytes);
        Ok(prepared.passes)
    }

    fn encode_frame(
        &self,
        instances: &[GeometryInstance],
        surface_view: &wgpu::TextureView,
        passes: ShadowPasses,
        queries: Option<&wgpu::QuerySet>,
    ) -> Result<wgpu::CommandEncoder, GraphicsError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Emergent geometry frame encoder"),
            });
        for face in 0..=passes.point_faces {
            if face == 0 && !passes.directional {
                continue;
            }
            let target = if face == 0 {
                &self.shadow_view
            } else {
                &self.point_shadow_views[face - 1]
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Emergent shadow depth pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: queries.map(|q| crate::gpu_profile::GpuProfiler::writes(q, face)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(
                1,
                &self.shadow_frame_bind_group,
                &[(face as u64 * self.shadow_matrix_stride) as u32],
            );
            for (index, instance) in instances.iter().enumerate() {
                let offset = u32::try_from(index as u64 * self.uniform_stride)
                    .map_err(|_| GraphicsError::TooManyInstances(instances.len()))?;
                let mesh = &self.geometry[instance.geometry.key];
                if mesh.overlay.is_some() {
                    continue;
                }
                if mesh.material.params[2] < 0.5 {
                    continue;
                }
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
                            r: self.background[0] as f64,
                            g: self.background[1] as f64,
                            b: self.background[2] as f64,
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
                timestamp_writes: queries.map(|q| {
                    crate::gpu_profile::GpuProfiler::writes(q, crate::gpu_profile::SHADOW_SLOTS)
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(skybox) = &self.skybox {
                skybox.draw(&mut pass);
            }
            if let Some(field) = &self.raymarched_terrain {
                field.draw(&mut pass);
            }
            for (field, _) in self.raymarched_regions.values() {
                field.draw(&mut pass);
            }
            pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(1, &self.lighting_bind_group, &[]);
            for index in (0..instances.len())
                .filter(|&i| !self.geometry[instances[i].geometry.key].transparent)
                .chain(self.transparent_order.iter().copied())
            {
                let instance = &instances[index];
                let offset = u32::try_from(index as u64 * self.uniform_stride)
                    .map_err(|_| GraphicsError::TooManyInstances(instances.len()))?;
                let mesh = &self.geometry[instance.geometry.key];
                if mesh.overlay.is_some() {
                    continue;
                }
                pass.set_pipeline(mesh.shader_pipeline.as_ref().unwrap_or(&self.pipeline));
                pass.set_bind_group(0, &self.uniform_bind_group, &[offset]);
                pass.set_bind_group(2, &mesh.material_bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        self.particles
            .encode(&mut encoder, surface_view, &self.depth_view);
        if instances
            .iter()
            .any(|i| self.geometry[i.geometry.key].overlay.is_some())
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI overlays"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.overlay_pipeline);
            pass.set_bind_group(1, &self.lighting_bind_group, &[]);
            for (index, instance) in instances.iter().enumerate() {
                let mesh = &self.geometry[instance.geometry.key];
                if mesh.overlay.is_none() {
                    continue;
                }
                let offset = u32::try_from(index as u64 * self.uniform_stride)
                    .map_err(|_| GraphicsError::TooManyInstances(instances.len()))?;
                pass.set_bind_group(0, &self.uniform_bind_group, &[offset]);
                pass.set_bind_group(2, &mesh.material_bind_group, &[]);
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
        let passes = self.prepare_frame(instances, lights)?;
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
        let encoder = self.encode_frame(instances, &view, passes, None)?;
        self.queue.submit(Some(encoder.finish()));
        window.pre_present_notify();
        self.queue.present(frame);
        Ok(RenderOutcome::Presented)
    }

    /// Whether this device supports optional render-pass timestamp queries.
    pub fn gpu_timestamps_supported(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    /// Profiles a serial offscreen frame without reading colour pixels. Returns
    /// None on devices without timestamps. Blocks for completion; use for diagnostics,
    /// not the game's presentation loop. Profiling resources are retained until resize.
    pub fn profile_lit_frame(
        &mut self,
        instances: &[GeometryInstance],
        lights: &[LightInstance],
    ) -> Result<Option<crate::GpuFrameProfile>, GraphicsError> {
        if self.surface.is_some() {
            return Err(GraphicsError::WrongRenderTarget);
        }
        if !self.gpu_timestamps_supported() {
            return Ok(None);
        }
        let dimensions = (self.config.width, self.config.height);
        if self
            .gpu_profiler
            .as_ref()
            .is_none_or(|p| p.dimensions != dimensions)
        {
            self.gpu_profiler = Some(crate::gpu_profile::GpuProfiler::new(
                &self.device,
                self.config.format,
                dimensions.0,
                dimensions.1,
            ));
        }
        let start = std::time::Instant::now();
        let passes = self.prepare_frame(instances, lights)?;
        let prepare_cpu_us = start.elapsed().as_secs_f64() * 1e6;
        let encode = std::time::Instant::now();
        let profiler = self.gpu_profiler.as_ref().expect("profiler created above");
        let mut active = [false; crate::gpu_profile::SLOTS];
        active[0] = passes.directional;
        for used in &mut active[1..1 + passes.point_faces] {
            *used = true;
        }
        active[crate::gpu_profile::SHADOW_SLOTS] = true;
        let encoder =
            self.encode_frame(instances, &profiler.target, passes, Some(&profiler.queries))?;
        let commands = encoder.finish();
        let encode_cpu_us = encode.elapsed().as_secs_f64() * 1e6;
        let completion = std::time::Instant::now();
        let submission = self.queue.submit(Some(commands));
        // Metal counter resolves need host-observed sampling completion (wgpu #9414).
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .map_err(|e| GraphicsError::Readback(e.to_string()))?;
        let mut resolve = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Completed pass timestamp resolve"),
            });
        profiler.resolve(&mut resolve, active);
        let submission = self.queue.submit(Some(resolve.finish()));
        let times = profiler.read(&self.device, &self.queue, submission, active)?;
        Ok(Some(crate::GpuFrameProfile {
            main_gpu_us: times[crate::gpu_profile::SHADOW_SLOTS],
            shadows_gpu_us: times[..crate::gpu_profile::SHADOW_SLOTS].iter().sum(),
            shadow_passes: active[..crate::gpu_profile::SHADOW_SLOTS]
                .iter()
                .filter(|used| **used)
                .count() as u32,
            prepare_cpu_us,
            encode_cpu_us,
            completion_cpu_us: completion.elapsed().as_secs_f64() * 1e6,
            total_cpu_us: start.elapsed().as_secs_f64() * 1e6,
            diagnostic_bytes: profiler.bytes(),
        }))
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
        let passes = self.prepare_frame(instances, lights)?;
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
        let mut encoder = self.encode_frame(instances, &view, passes, None)?;
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

    fn prepare_lights(&self, lights: &[LightInstance]) -> Result<PreparedLights, GraphicsError> {
        if lights.len() > MAX_LIGHTS {
            return Err(GraphicsError::TooManyLights(lights.len()));
        }
        let mut result = LightUniform::zeroed();
        let mut shadow_view_projection = Mat4::IDENTITY;
        let mut shadow_selected = false;
        let mut point_matrices = [Mat4::IDENTITY; POINT_SHADOW_FACES];
        let mut point_count = 0;
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
            let point_shadow = source.kind() == LightKind::Point
                && source.shadow_casting() == ShadowCasting::Enabled;
            let mut point_slot = 0;
            if point_shadow {
                if point_count == MAX_POINT_SHADOWS {
                    return Err(GraphicsError::TooManyPointShadows);
                }
                let matrices = point_shadow_matrices(position, source.range().unwrap_or(0.0))
                    .ok_or(GraphicsError::InvalidPointShadow { light: index })?;
                point_slot = point_count;
                point_matrices[point_count * 6..point_count * 6 + 6].copy_from_slice(&matrices);
                point_count += 1;
            }
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
                    point_slot as f32,
                    if casts_shadow || point_shadow {
                        1.0
                    } else {
                        0.0
                    },
                ],
            };
        }
        result.count = lights.len() as u32;
        Ok(PreparedLights {
            uniform: result,
            directional_matrix: shadow_view_projection,
            point_matrices,
            passes: ShadowPasses {
                directional: shadow_selected,
                point_faces: point_count * 6,
            },
        })
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
            uv: [0.0; 2],
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
            uv: [0.0; 2],
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
            uv: [0.0; 2],
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
            uv: vertex.uv().unwrap_or_default().to_array(),
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

fn validated_view(eye: Vec3, target: Vec3, up: Vec3) -> Result<Mat4, GraphicsError> {
    let forward = target - eye;
    let cross = forward.cross(up);
    if !eye.is_finite()
        || !target.is_finite()
        || !up.is_finite()
        || !forward.length_squared().is_finite()
        || !cross.length_squared().is_finite()
        || forward.length_squared() < 1e-8
        || cross.length_squared() < 1e-8
    {
        return Err(GraphicsError::InvalidView);
    }
    let view = look_at_rh(eye, target, up);
    if !view.is_finite() {
        return Err(GraphicsError::InvalidView);
    }
    Ok(view)
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

pub(crate) fn perspective_rh(vertical_fov: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let focal_length = 1.0 / (vertical_fov * 0.5).tan();
    let depth = far / (near - far);
    Mat4::from_cols(
        Vec4::new(focal_length / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, focal_length, 0.0, 0.0),
        Vec4::new(0.0, 0.0, depth, -1.0),
        Vec4::new(0.0, 0.0, near * depth, 0.0),
    )
}

pub(crate) fn orthographic_rh(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> Mat4 {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn point_shadow_matrices(position: Vec3, range: f32) -> Option<[Mat4; 6]> {
    if !position.is_finite()
        || position.abs().max_element() > 1_000_000.0
        || !range.is_finite()
        || !(0.01..=1_000_000.0).contains(&range)
    {
        return None;
    }
    let projection = perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, range * 0.001, range);
    let axes = [
        (Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::NEG_Y),
        (Vec3::Y, Vec3::Z),
        (Vec3::NEG_Y, Vec3::NEG_Z),
        (Vec3::Z, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_Y),
    ];
    let matrices =
        axes.map(|(direction, up)| projection * look_at_rh(position, position + direction, up));
    matrices.iter().all(Mat4::is_finite).then_some(matrices)
}

fn create_point_shadow_views(
    device: &wgpu::Device,
) -> (wgpu::TextureView, [wgpu::TextureView; POINT_SHADOW_FACES]) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Emergent point shadow faces"),
        size: wgpu::Extent3d {
            width: POINT_SHADOW_SIZE,
            height: POINT_SHADOW_SIZE,
            depth_or_array_layers: POINT_SHADOW_FACES as u32,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let array = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let faces = std::array::from_fn(|i| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: i as u32,
            array_layer_count: Some(1),
            ..Default::default()
        })
    });
    (array, faces)
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

impl crate::PickingMesh {
    /// Builds an optional inspection cache from the renderer's own tessellation.
    pub fn from_geometry(geometry: &Geometry) -> Self {
        let (vertices, indices) = tessellate(geometry);
        Self::new(
            vertices
                .into_iter()
                .map(|v| Vec3::from_array(v.position))
                .collect(),
            indices,
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn point_shadow_faces_project_axes_and_depth_consistently() {
        let position = Vec3::new(3.0, 2.0, 1.0);
        let range = 20.0;
        let matrices = point_shadow_matrices(position, range).unwrap();
        for (matrix, axis) in matrices.iter().zip([
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ]) {
            let center = matrix.project_point3(position + axis * 2.0);
            assert!(center.x.abs() < 0.0001 && center.y.abs() < 0.0001);
            assert!((0.0..1.0).contains(&center.z));
            let near = matrix.project_point3(position + axis * (range * 0.001));
            let far = matrix.project_point3(position + axis * range);
            assert!(near.z.abs() < 0.0001, "{near:?}");
            assert!((far.z - 1.0).abs() < 0.0001);
        }
        for range in [0.0, 0.001, 1_000_001.0, f32::NAN] {
            assert!(point_shadow_matrices(position, range).is_none());
        }
        assert!(point_shadow_matrices(Vec3::splat(f32::INFINITY), 20.0).is_none());
        assert!(point_shadow_matrices(Vec3::X * 1_000_001.0, 20.0).is_none());
        assert_eq!(
            std::mem::offset_of!(FrameUniform, point_view_projections),
            96
        );
        assert_eq!(
            std::mem::size_of::<FrameUniform>(),
            96 + POINT_SHADOW_FACES * 64
        );
    }

    #[test]
    fn inspection_view_rejects_degenerate_camera_inputs() {
        use super::*;
        assert!(validated_view(Vec3::new(7., 5., 9.), Vec3::ZERO, Vec3::Y)
            .unwrap()
            .is_finite());
        assert!(validated_view(Vec3::ZERO, Vec3::ZERO, Vec3::Y).is_err());
        assert!(validated_view(Vec3::Y, Vec3::ZERO, Vec3::Y).is_err());
        assert!(validated_view(Vec3::splat(f32::NAN), Vec3::ZERO, Vec3::Y).is_err());
    }
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
    fn sphere_normals_face_outward_for_smooth_and_flat_shading() {
        for (sectors, stacks) in [(3, 2), (12, 6), (24, 12)] {
            let sphere =
                Geometry::from(Primitive::sphere(1.0, sectors, stacks, Color::WHITE).unwrap());
            let (vertices, indices) = tessellate(&sphere);
            let (flat, _) = flatten_triangles(&vertices, &indices);
            for vertex in vertices.iter().chain(&flat) {
                let normal = Vec3::from_array(vertex.normal);
                assert!(normal.dot(Vec3::from_array(vertex.position)) > 0.0);
                assert!((normal.length() - 1.0).abs() < 1e-5);
            }
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
