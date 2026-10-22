use std::error::Error;
use std::fmt;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

const CUBE_VERTICES: [Vertex; 8] = [
    Vertex {
        position: [-1.0, -1.0, 1.0],
        color: [0.95, 0.25, 0.20],
    },
    Vertex {
        position: [1.0, -1.0, 1.0],
        color: [0.20, 0.80, 0.35],
    },
    Vertex {
        position: [1.0, 1.0, 1.0],
        color: [0.20, 0.45, 1.00],
    },
    Vertex {
        position: [-1.0, 1.0, 1.0],
        color: [0.95, 0.75, 0.20],
    },
    Vertex {
        position: [-1.0, -1.0, -1.0],
        color: [0.70, 0.25, 0.90],
    },
    Vertex {
        position: [1.0, -1.0, -1.0],
        color: [0.20, 0.85, 0.90],
    },
    Vertex {
        position: [1.0, 1.0, -1.0],
        color: [0.95, 0.45, 0.65],
    },
    Vertex {
        position: [-1.0, 1.0, -1.0],
        color: [0.75, 0.85, 0.95],
    },
];

const CUBE_INDICES: [u16; 36] = [
    0, 1, 2, 0, 2, 3, 1, 5, 6, 1, 6, 2, 5, 4, 7, 5, 7, 6, 4, 0, 3, 4, 3, 7, 3, 2, 6, 3, 6, 7, 4, 5,
    1, 4, 1, 0,
];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectUniform {
    model_view_projection: [[f32; 4]; 4],
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

/// Minimal renderer for the first Emergent 3D object.
pub struct CubeRenderer {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    info: RendererInfo,
}

impl CubeRenderer {
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
            label: Some("Emergent cube shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/cube.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Emergent object uniform"),
            contents: bytemuck::bytes_of(&ObjectUniform {
                model_view_projection: Mat4::IDENTITY.to_cols_array_2d(),
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Emergent object uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Emergent object uniform bind group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Emergent cube pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Emergent cube pipeline"),
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
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Emergent cube vertices"),
            contents: bytemuck::cast_slice(&CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Emergent cube indices"),
            contents: bytemuck::cast_slice(&CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
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
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            uniform_bind_group,
            depth_view,
            info,
        })
    }

    pub fn info(&self) -> &RendererInfo {
        &self.info
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

    pub fn render(&mut self, model: Mat4) -> Result<RenderOutcome, GraphicsError> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(RenderOutcome::Skipped);
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
                return Err(GraphicsError::SurfaceValidation);
            }
        };

        let aspect = self.config.width as f32 / self.config.height as f32;
        let view = look_at_rh(Vec3::new(4.0, 3.0, 6.0), Vec3::ZERO, Vec3::Y);
        let projection = perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        let uniform = ObjectUniform {
            model_view_projection: (projection * view * model).to_cols_array_2d(),
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Emergent frame encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Emergent cube pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        self.queue.present(frame);
        Ok(RenderOutcome::Presented)
    }
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
}
