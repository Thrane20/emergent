//! Explicit blocking diagnostics. Normal frames never map these buffers.
use crate::GraphicsError;

pub(crate) const SHADOW_SLOTS: usize = 1 + crate::graphics::POINT_SHADOW_FACES;
pub(crate) const SLOTS: usize = SHADOW_SLOTS + 1;
const STRIDE: u64 = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;

/// Serial offscreen rendering diagnostics, in microseconds. GPU times exclude
/// readback/PNG; main includes sky, ray terrain and ordinary geometry, not particles.
#[derive(Clone, Copy, Debug)]
pub struct GpuFrameProfile {
    pub main_gpu_us: f64,
    pub shadows_gpu_us: f64,
    pub shadow_passes: u32,
    pub prepare_cpu_us: f64,
    pub encode_cpu_us: f64,
    /// Submission, GPU wait and timestamp mapping; overlaps GPU work.
    pub completion_cpu_us: f64,
    pub total_cpu_us: f64,
    /// Diagnostic colour target and two timestamp buffers only; excludes driver overhead.
    pub diagnostic_bytes: u64,
}

pub(crate) fn timestamp_features(available: wgpu::Features) -> wgpu::Features {
    available & wgpu::Features::TIMESTAMP_QUERY
}

fn elapsed_us(start: u64, end: u64, period_ns: f32) -> Result<f64, GraphicsError> {
    if start == 0 || end == 0 || !period_ns.is_finite() || period_ns <= 0. {
        return Err(GraphicsError::Readback(
            "GPU timestamps are unavailable or have an invalid period".into(),
        ));
    }
    let ticks = end.checked_sub(start).ok_or_else(|| {
        GraphicsError::Readback(format!("GPU timestamp counter reversed ({start} -> {end})"))
    })?;
    Ok(ticks as f64 * f64::from(period_ns) / 1000.)
}

pub(crate) struct GpuProfiler {
    pub queries: wgpu::QuerySet,
    pub target: wgpu::TextureView,
    pub dimensions: (u32, u32),
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
}
impl GpuProfiler {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Frame pass timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: (SLOTS * 2) as u32,
        });
        let buffer = |label, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: STRIDE * SLOTS as u64,
                usage,
                mapped_at_creation: false,
            })
        };
        let target = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Profiling colour target"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Self {
            queries,
            target,
            dimensions: (width, height),
            resolve: buffer(
                "Timestamp resolve",
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            ),
            readback: buffer(
                "Timestamp readback",
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
        }
    }
    pub fn writes(queries: &wgpu::QuerySet, slot: usize) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: queries,
            beginning_of_pass_write_index: Some((slot * 2) as u32),
            end_of_pass_write_index: Some((slot * 2 + 1) as u32),
        }
    }
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder, active: [bool; SLOTS]) {
        for (slot, used) in active.into_iter().enumerate() {
            if used {
                let first = (slot * 2) as u32;
                encoder.resolve_query_set(
                    &self.queries,
                    first..first + 2,
                    &self.resolve,
                    slot as u64 * STRIDE,
                );
            }
        }
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, STRIDE * SLOTS as u64);
    }
    pub fn read(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        submission: wgpu::SubmissionIndex,
        active: [bool; SLOTS],
    ) -> Result<[f64; SLOTS], GraphicsError> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        self.readback
            .map_async(wgpu::MapMode::Read, .., move |result| {
                let _ = sender.send(result);
            });
        let result = (|| {
            let timeout = std::time::Duration::from_secs(10);
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(timeout),
                })
                .map_err(|e| GraphicsError::Readback(e.to_string()))?;
            receiver
                .recv_timeout(timeout)
                .map_err(|e| GraphicsError::Readback(e.to_string()))?
                .map_err(|e| GraphicsError::Readback(e.to_string()))?;
            let mapped = self
                .readback
                .get_mapped_range(..)
                .map_err(|e| GraphicsError::Readback(e.to_string()))?;
            let mut times = [0.; SLOTS];
            for (slot, used) in active.into_iter().enumerate() {
                if used {
                    let offset = slot * STRIDE as usize;
                    let counter = |at| u64::from_le_bytes(std::array::from_fn(|i| mapped[at + i]));
                    times[slot] = elapsed_us(
                        counter(offset),
                        counter(offset + 8),
                        queue.get_timestamp_period(),
                    )?;
                }
            }
            Ok(times)
        })();
        self.readback.unmap();
        result
    }
    pub fn bytes(&self) -> u64 {
        u64::from(self.dimensions.0) * u64::from(self.dimensions.1) * 4 + STRIDE * SLOTS as u64 * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_are_optional_and_use_adapter_period() {
        assert_eq!(
            timestamp_features(wgpu::Features::empty()),
            wgpu::Features::empty()
        );
        assert_eq!(
            timestamp_features(wgpu::Features::all()),
            wgpu::Features::TIMESTAMP_QUERY
        );
        assert_eq!(elapsed_us(100, 300, 2.).unwrap(), 0.4);
        assert_eq!(elapsed_us(100, 100, 1.).unwrap(), 0.);
        assert!(elapsed_us(300, 100, 1.).is_err());
        assert!(elapsed_us(0, 0, 1.).is_err());
        assert!(elapsed_us(100, 200, 0.).is_err());
    }
}
