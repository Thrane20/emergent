//! Renderer-owned billboard pass. Resources and staging buffers are reused between frames.
use crate::{Mat4, ParticleBlend, ParticleStore, TextureImage};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use wgpu::util::DeviceExt;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Frame {
    vp: [[f32; 4]; 4],
    inverse: [[f32; 4]; 4],
    right: [f32; 4],
    up: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    position_size: [f32; 4],
    color: [f32; 4],
    uv: [f32; 4],
    params: [f32; 4],
}
struct Sprite {
    bind: wgpu::BindGroup,
    _source: Option<TextureImage>,
    generation: u64,
}
struct Sortable {
    instance: Instance,
    key: usize,
    depth: f32,
    blend: ParticleBlend,
}
pub(crate) struct ParticlePass {
    alpha: wgpu::RenderPipeline,
    screen: wgpu::RenderPipeline,
    multiply: wgpu::RenderPipeline,
    frame: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    frame_layout: wgpu::BindGroupLayout,
    sprites: HashMap<usize, Sprite>,
    generation: u64,
    frame_bind: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    capacity: usize,
    sorted: Vec<Sortable>,
    instances: Vec<Instance>,
}
impl ParticlePass {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth: &wgpu::TextureView,
    ) -> Self {
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle frame"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle sprite"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particles.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle layout"),
            bind_group_layouts: &[Some(&frame_layout), Some(&layout)],
            immediate_size: 0,
        });
        let make = |blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{label:Some("particle billboard pipeline"),layout:Some(&pipeline_layout),vertex:wgpu::VertexState{module:&shader,entry_point:Some("vertex_main"),compilation_options:Default::default(),buffers:&[Some(wgpu::VertexBufferLayout{array_stride:64,step_mode:wgpu::VertexStepMode::Instance,attributes:&wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x4,2=>Float32x4,3=>Float32x4]})]},primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:Some(false),depth_compare:Some(wgpu::CompareFunction::LessEqual),stencil:Default::default(),bias:Default::default()}),multisample:Default::default(),fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fragment_main"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format,blend:Some(blend),write_mask:wgpu::ColorWrites::ALL})]}),multiview_mask:None,cache:None})
        };
        let alpha = make(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let screen = make(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrc,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        });
        let multiply = make(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        });
        let frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle camera"),
            size: std::mem::size_of::<Frame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind = Self::frame_binding(device, &frame_layout, &frame, depth);
        Self {
            alpha,
            screen,
            multiply,
            frame,
            layout,
            frame_layout,
            sprites: HashMap::new(),
            generation: 0,
            frame_bind,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("particle instances"),
                size: 64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity: 1,
            sorted: Vec::new(),
            instances: Vec::new(),
        }
    }
    pub fn prepare_effect(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        effect: &crate::ParticleEffect,
    ) {
        for image in effect.textures.iter() {
            let image = image.as_ref();
            let key = image.map_or(0, |i| i.rgba().as_ptr() as usize);
            self.sprites.entry(key).or_insert_with(|| {
                upload_sprite(device, queue, &self.layout, image, self.generation)
            });
        }
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        store: &ParticleStore,
        view: Mat4,
        vp: Mat4,
    ) {
        self.sorted.clear();
        self.instances.clear();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let inverse_view = view.inverse();
        queue.write_buffer(
            &self.frame,
            0,
            bytemuck::bytes_of(&Frame {
                vp: vp.to_cols_array_2d(),
                inverse: vp.inverse().to_cols_array_2d(),
                right: inverse_view.x_axis.to_array(),
                up: inverse_view.y_axis.to_array(),
            }),
        );
        let mut has_emitters = false;
        // Texture identity is pinned by the retained image clone, so allocator reuse cannot alias another sprite.
        store.visit(|e, image, model, particles| {
            has_emitters = true;
            let key = image.map_or(0, |i| i.rgba().as_ptr() as usize);
            let sprite = self
                .sprites
                .entry(key)
                .or_insert_with(|| upload_sprite(device, queue, &self.layout, image, generation));
            sprite.generation = generation;
            let texel = image.map_or([1. / 64., 1. / 64.], |i| {
                [1. / i.width() as f32, 1. / i.height() as f32]
            });
            let scale = if e.local_space {
                model
                    .x_axis
                    .truncate()
                    .length()
                    .max(model.y_axis.truncate().length())
                    .max(model.z_axis.truncate().length())
            } else {
                1.
            };
            for p in particles {
                let t = (p.age / p.lifetime).clamp(0., 1.);
                let position = if e.local_space {
                    model.transform_point3(p.position)
                } else {
                    p.position
                };
                let frame = crate::particles::sprite_frame_at(
                    (p.frame + p.age * e.fps).floor() as u32,
                    e.frames,
                    e.ping_pong,
                );
                let size = (e.size[0] * (1. - t) + e.size[1] * t) * scale * p.scale;
                let color =
                    std::array::from_fn(|i| e.color_start[i] * (1. - t) + e.color_end[i] * t);
                self.sorted.push(Sortable {
                    instance: Instance {
                        position_size: position.extend(size).to_array(),
                        color,
                        uv: [
                            (frame % e.columns) as f32 / e.columns as f32,
                            (frame / e.columns) as f32 / e.rows as f32,
                            1. / e.columns as f32,
                            1. / e.rows as f32,
                        ],
                        params: [
                            p.rotation,
                            if e.blend == ParticleBlend::Additive {
                                -e.soft_distance - 1.
                            } else {
                                e.soft_distance
                            },
                            texel[0],
                            texel[1],
                        ],
                    },
                    key,
                    depth: -view.transform_point3(position).z,
                    blend: e.blend,
                });
            }
        });
        if has_emitters {
            self.sprites
                .retain(|_, sprite| sprite.generation == generation);
        }
        self.sorted
            .sort_unstable_by(|a, b| b.depth.total_cmp(&a.depth));
        self.instances
            .extend(self.sorted.iter().map(|p| p.instance));
        if self.instances.len() > self.capacity {
            self.capacity = self.instances.len().next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("particle instances"),
                size: self.capacity as u64 * 64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !self.instances.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.instances));
        }
    }
    fn frame_binding(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        frame: &wgpu::Buffer,
        depth: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particle frame"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
            ],
        })
    }
    pub fn resize_depth(&mut self, device: &wgpu::Device, depth: &wgpu::TextureView) {
        self.frame_bind = Self::frame_binding(device, &self.frame_layout, &self.frame, depth);
    }
    pub fn clear(&mut self) {
        self.sorted.clear();
        self.instances.clear();
        self.sprites.clear();
    }
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) {
        if self.instances.is_empty() {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("particles"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.frame_bind, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        let mut start = 0;
        while start < self.sorted.len() {
            let p = &self.sorted[start];
            let mut end = start + 1;
            while end < self.sorted.len()
                && self.sorted[end].key == p.key
                && self.sorted[end].blend == p.blend
            {
                end += 1;
            }
            pass.set_pipeline(match p.blend {
                ParticleBlend::Alpha | ParticleBlend::Additive => &self.alpha,
                ParticleBlend::Screen => &self.screen,
                ParticleBlend::Multiply => &self.multiply,
            });
            pass.set_bind_group(1, &self.sprites[&p.key].bind, &[]);
            pass.draw(0..6, start as u32..end as u32);
            start = end;
        }
    }
}

fn premultiply_srgb(pixels: &mut [u8]) {
    let linear: [f32; 256] = std::array::from_fn(|i| {
        let c = i as f32 / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = pixel[3] as f32 / 255.;
        for channel in &mut pixel[..3] {
            let c = linear[*channel as usize] * alpha;
            let srgb = if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1. / 2.4) - 0.055
            };
            *channel = (srgb * 255.).round().clamp(0., 255.) as u8;
        }
    }
}
fn upload_sprite(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    image: Option<&TextureImage>,
    generation: u64,
) -> Sprite {
    let (w, h, mut pixels) = match image {
        Some(i) => (i.width(), i.height(), i.rgba().to_vec()),
        None => {
            let mut p = Vec::with_capacity(64 * 64 * 4);
            for y in 0..64 {
                for x in 0..64 {
                    let r = ((x as f32 + 0.5) / 32. - 1.).powi(2)
                        + ((y as f32 + 0.5) / 32. - 1.).powi(2);
                    let a = (1. - r).max(0.).powi(2);
                    p.extend_from_slice(&[255, 255, 255, (255. * a) as u8]);
                }
            }
            (64, 64, p)
        }
    };
    premultiply_srgb(&mut pixels);
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("particle sprite"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            // shortcut: base mip avoids atlas frame bleeding; add per-cell mips if distant sprites alias.
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &pixels,
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("particle sprite"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    Sprite {
        bind,
        _source: image.cloned(),
        generation,
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn sprites_premultiply_in_linear_light() {
        let mut pixels = [255, 0, 0, 128, 255, 128, 64, 0];
        super::premultiply_srgb(&mut pixels);
        assert!((pixels[0] as i32 - 188).abs() <= 1);
        assert_eq!(pixels[3], 128);
        assert_eq!(&pixels[4..], &[0, 0, 0, 0]);
    }
}
