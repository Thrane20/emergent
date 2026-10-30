use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Runtime diagnostics; these options never change an authored terrain recipe.
#[derive(Clone, Copy, Debug)]
pub struct RaymarchedRenderOptions {
    /// Restrict draws to conservative projected bounds. Disable for comparisons.
    pub bounds_culling: bool,
    /// Colour rays by iteration count, with exhausted rays shown in magenta.
    pub step_heatmap: bool,
}
impl Default for RaymarchedRenderOptions {
    fn default() -> Self {
        Self {
            bounds_culling: true,
            step_heatmap: false,
        }
    }
}

/// Terrain submission counts from the most recently prepared frame.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RaymarchedFrameStats {
    pub regions: u32,
    pub submitted: u32,
    /// Sum of submitted scissor areas; overlapping regions count separately.
    pub scissor_pixels: u64,
}

// Clip before dividing by W. Near-plane intersections deliberately use the full
// viewport: projecting only the original corners would under-bound the surface.
fn projected_bounds(
    vp: crate::Mat4,
    lower: crate::Vec3,
    upper: crate::Vec3,
    width: u32,
    height: u32,
) -> Option<[u32; 4]> {
    let corners: [_; 8] = std::array::from_fn(|i| {
        vp * crate::Vec3::new(
            if i & 1 == 0 { lower.x } else { upper.x },
            if i & 2 == 0 { lower.y } else { upper.y },
            if i & 4 == 0 { lower.z } else { upper.z },
        )
        .extend(1.)
    });
    if corners.iter().any(|p| !p.is_finite()) {
        return Some([0, 0, width, height]);
    }
    for plane in 0..6 {
        if corners.iter().all(|p| {
            let distance = match plane {
                0 => p.x + p.w,
                1 => p.w - p.x,
                2 => p.y + p.w,
                3 => p.w - p.y,
                4 => p.z,
                _ => p.w - p.z,
            };
            distance < -1e-5 * p.w.abs().max(1.)
        }) {
            return None;
        }
    }
    if corners
        .iter()
        .any(|p| p.w <= 1e-5 || p.z <= 1e-5 * p.w.abs().max(1.))
    {
        return Some([0, 0, width, height]);
    }
    let mut min = crate::Vec2::ONE;
    let mut max = -crate::Vec2::ONE;
    for p in corners {
        let ndc = crate::Vec2::new(p.x / p.w, -p.y / p.w);
        min = min.min(ndc);
        max = max.max(ndc);
    }
    let extent = crate::Vec2::new(width as f32, height as f32);
    let lo = (((min * 0.5 + crate::Vec2::splat(0.5)) * extent).floor() - crate::Vec2::ONE)
        .clamp(crate::Vec2::ZERO, extent);
    let hi = (((max * 0.5 + crate::Vec2::splat(0.5)) * extent).ceil() + crate::Vec2::ONE)
        .clamp(crate::Vec2::ZERO, extent);
    let (x, y, right, bottom) = (lo.x as u32, lo.y as u32, hi.x as u32, hi.y as u32);
    (right > x && bottom > y).then_some([x, y, right - x, bottom - y])
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    inverse_vp: [[f32; 4]; 4],
    vp: [[f32; 4]; 4],
    normal_model: [[f32; 4]; 4],
    params: [f32; 4],
    detail: [f32; 4],
    recipe: [u32; 4],
}
pub(crate) struct RaymarchedTerrainPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    field: crate::RaymarchedTerrain,
    scissor: Option<[u32; 4]>,
}
impl RaymarchedTerrainPass {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        field: crate::RaymarchedTerrain,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Implicit terrain layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Implicit terrain uniform"),
            contents: bytemuck::bytes_of(&Uniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Implicit terrain bindings"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Implicit terrain shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/raymarched_terrain.wgsl").into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Implicit terrain pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Implicit terrain pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            bind_group,
            uniform,
            field,
            scissor: None,
        }
    }
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        vp: crate::Mat4,
        model: crate::Mat4,
        viewport: [u32; 2],
        options: RaymarchedRenderOptions,
    ) {
        let local_vp = vp * model;
        let (lower, upper) = self.field.bounds();
        self.scissor = if options.bounds_culling {
            projected_bounds(local_vp, lower, upper, viewport[0], viewport[1])
        } else {
            Some([0, 0, viewport[0], viewport[1]])
        };
        if self.scissor.is_none() {
            return;
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Uniform {
                inverse_vp: local_vp.inverse().to_cols_array_2d(),
                vp: local_vp.to_cols_array_2d(),
                normal_model: model.inverse().transpose().to_cols_array_2d(),
                params: [
                    self.field.size,
                    self.field.phase(),
                    self.field.relief,
                    self.field.biome_scale,
                ],
                detail: [
                    self.field.overhang,
                    self.field.cave_radius,
                    self.field.density_bound(),
                    0.,
                ],
                recipe: [
                    self.field.version,
                    self.field.seed,
                    self.field.biome as u32,
                    u32::from(options.step_heatmap),
                ],
            }),
        );
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some([x, y, width, height]) = self.scissor else {
            return;
        };
        pass.set_scissor_rect(x, y, width, height);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    pub fn scissor(&self) -> Option<[u32; 4]> {
        self.scissor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mat4, Vec3};
    #[test]
    fn projected_bounds_cull_clip_planes_and_cover_intersections() {
        let lower = Vec3::new(-0.25, -0.25, 0.25);
        let upper = Vec3::new(0.25, 0.25, 0.75);
        assert_eq!(
            projected_bounds(Mat4::IDENTITY, lower, upper, 100, 100),
            Some([36, 36, 28, 28])
        );
        for offset in [
            Vec3::X * 3.,
            -Vec3::X * 3.,
            Vec3::Y * 3.,
            -Vec3::Y * 3.,
            Vec3::Z * 3.,
            -Vec3::Z * 3.,
        ] {
            assert_eq!(
                projected_bounds(Mat4::from_translation(offset), lower, upper, 100, 100),
                None
            );
        }
        assert_eq!(
            projected_bounds(Mat4::IDENTITY, Vec3::splat(-1.), Vec3::ONE, 100, 100),
            Some([0, 0, 100, 100])
        );
        let perspective = crate::graphics::perspective_rh(1., 1., 0.1, 100.);
        assert_eq!(
            projected_bounds(perspective, Vec3::splat(-1.), Vec3::ONE, 100, 100),
            Some([0, 0, 100, 100])
        );
        assert_eq!(
            projected_bounds(perspective, lower, upper, 100, 100),
            None,
            "behind camera"
        );
    }
    #[test]
    fn transformed_bounds_contain_projected_samples() {
        let projection = crate::graphics::perspective_rh(1., 1.7, 0.1, 100.);
        for perspective in [
            projection,
            crate::graphics::orthographic_rh(-5., 5., -3., 3., 0.1, 100.),
        ] {
            let model = Mat4::from_translation(Vec3::new(1., 0., -8.))
                * Mat4::from_rotation_y(0.7)
                * Mat4::from_scale(Vec3::new(-2., 0.5, 1.3));
            let vp = perspective * model;
            let [x, y, w, h] = projected_bounds(vp, -Vec3::ONE, Vec3::ONE, 960, 540).unwrap();
            for ix in -4..=4 {
                for iy in -4..=4 {
                    for iz in -4..=4 {
                        let p = vp
                            * Vec3::new(ix as f32 / 4., iy as f32 / 4., iz as f32 / 4.).extend(1.);
                        let px = (p.x / p.w * 0.5 + 0.5) * 960.;
                        let py = (-p.y / p.w * 0.5 + 0.5) * 540.;
                        assert!(
                            px >= x as f32
                                && px <= (x + w) as f32
                                && py >= y as f32
                                && py <= (y + h) as f32
                        );
                    }
                }
            }
        }
    }
}
