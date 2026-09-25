use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use wgpu::util::DeviceExt as _;

use crate::config::VideoFormat;
use crate::error::CameraManError;
use crate::frame::{Frame, PixelFormat};
use crate::layout::CompositionLayout;
use crate::render::{Compositor, ScalingFilter};

const WORKGROUP_SIZE: u32 = 256;
const MAP_TIMEOUT: Duration = Duration::from_secs(5);
const EMPTY_CELL_BGRA: [u8; 4] = [32, 32, 32, 255];

const NEAREST_COMPOSITOR_SHADER: &str = r#"
@group(0) @binding(0)
var<storage, read> source_pixels: array<u32>;

@group(0) @binding(1)
var<storage, read_write> destination_pixels: array<u32>;

@group(0) @binding(2)
var<storage, read> params: array<u32>;

@compute @workgroup_size(256)
fn compose_nearest(@builtin(global_invocation_id) id: vec3<u32>) {
    let source_width = params[0];
    let source_height = params[1];
    let destination_width = params[2];
    let destination_height = params[3];
    let fit_x = params[4];
    let fit_y = params[5];
    let fit_width = params[6];
    let fit_height = params[7];
    let background = params[8];
    let index = id.x;
    let pixel_count = destination_width * destination_height;

    if (index >= pixel_count) {
        return;
    }

    let x = index % destination_width;
    let y = index / destination_width;
    if (x < fit_x || x >= fit_x + fit_width || y < fit_y || y >= fit_y + fit_height) {
        destination_pixels[index] = background;
        return;
    }

    let source_x = min((x - fit_x) * source_width / fit_width, source_width - 1u);
    let source_y = min((y - fit_y) * source_height / fit_height, source_height - 1u);
    destination_pixels[index] = source_pixels[source_y * source_width + source_x];
}
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuProbe {
    pub adapter_name: String,
    pub backend: wgpu::Backend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuExperimentError(String);

impl GpuExperimentError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for GpuExperimentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GpuExperimentError {}

/// Feature-gated compute compositor used only for benchmark and parity work.
/// The production compositor remains CPU-based until measurements justify a
/// switch and the multi-source path reaches the same behavioral contract.
pub struct GpuCompositorExperiment {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    probe: GpuProbe,
}

impl GpuCompositorExperiment {
    pub fn new() -> Result<Self, GpuExperimentError> {
        let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        #[cfg(target_os = "macos")]
        {
            instance_descriptor.backends = wgpu::Backends::METAL;
        }
        let instance = wgpu::Instance::new(instance_descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            ..Default::default()
        }))
        .map_err(|error| GpuExperimentError::new(format!("GPU adapter unavailable: {error}")))?;
        let adapter_info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("CameraMan GPU compositor experiment"),
            ..Default::default()
        }))
        .map_err(|error| GpuExperimentError::new(format!("GPU device unavailable: {error}")))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CameraMan nearest compositor shader"),
            source: wgpu::ShaderSource::Wgsl(NEAREST_COMPOSITOR_SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("CameraMan nearest compositor pipeline"),
            layout: None,
            module: &shader,
            entry_point: Some("compose_nearest"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            probe: GpuProbe {
                adapter_name: adapter_info.name,
                backend: adapter_info.backend,
            },
        })
    }

    pub fn probe() -> Result<GpuProbe, GpuExperimentError> {
        Self::new().map(|experiment| experiment.probe)
    }

    pub fn adapter(&self) -> &GpuProbe {
        &self.probe
    }

    /// Composes one BGRA source into the full output cell with the same
    /// nearest-neighbor aspect-fit and background rules as [`Compositor`].
    pub fn compose_single(
        &self,
        input: &Frame,
        format: VideoFormat,
    ) -> Result<Frame, GpuExperimentError> {
        validate_format(input, format)?;
        let byte_len = output_byte_len(format)?;
        let (fit_x, fit_y, fit_width, fit_height) =
            aspect_fit_rect(input.width(), input.height(), format.width, format.height);
        let params = u32s_to_le_bytes(&[
            input.width(),
            input.height(),
            format.width,
            format.height,
            fit_x,
            fit_y,
            fit_width,
            fit_height,
            u32::from_le_bytes(EMPTY_CELL_BGRA),
        ]);

        let source_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("CameraMan GPU source pixels"),
                contents: input.data(),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let destination_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("CameraMan GPU destination pixels"),
            size: byte_len as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("CameraMan GPU compositor parameters"),
                contents: &params,
                usage: wgpu::BufferUsages::STORAGE,
            });
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("CameraMan GPU readback"),
            size: byte_len as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let layout = self.pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CameraMan GPU compositor bindings"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: destination_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("CameraMan GPU compositor commands"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("CameraMan GPU compositor pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let pixel_count = format.width.saturating_mul(format.height);
            pass.dispatch_workgroups(pixel_count.div_ceil(WORKGROUP_SIZE), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&destination_buffer, 0, &staging_buffer, 0, byte_len as u64);
        let submission = self.queue.submit([encoder.finish()]);

        let slice = staging_buffer.slice(..);
        let (sender, receiver) = mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(MAP_TIMEOUT),
            })
            .map_err(|error| GpuExperimentError::new(format!("GPU wait failed: {error}")))?;
        receiver
            .recv_timeout(MAP_TIMEOUT)
            .map_err(|error| GpuExperimentError::new(format!("GPU map timed out: {error}")))?
            .map_err(|error| GpuExperimentError::new(format!("GPU map failed: {error}")))?;

        let mapped = slice
            .get_mapped_range()
            .map_err(|error| GpuExperimentError::new(format!("GPU readback failed: {error}")))?;
        let pixels = mapped[..byte_len].to_vec();
        drop(mapped);
        staging_buffer.unmap();
        Frame::new_checked(format.width, format.height, PixelFormat::Bgra8, pixels)
            .map_err(|error| GpuExperimentError::new(format!("GPU output is invalid: {error}")))
    }
}

/// Contract-preserving adapter: the GPU path is opportunistic and every
/// unsupported case or runtime error immediately falls back to the CPU.
pub struct ExperimentalCompositor {
    cpu: Compositor,
    gpu: Option<GpuCompositorExperiment>,
    gpu_attempts: AtomicU64,
    gpu_fallbacks: AtomicU64,
}

impl ExperimentalCompositor {
    pub fn new(format: VideoFormat) -> Self {
        Self {
            cpu: Compositor::new(format),
            gpu: GpuCompositorExperiment::new().ok(),
            gpu_attempts: AtomicU64::new(0),
            gpu_fallbacks: AtomicU64::new(0),
        }
    }

    pub fn cpu_only(format: VideoFormat) -> Self {
        Self {
            cpu: Compositor::new(format),
            gpu: None,
            gpu_attempts: AtomicU64::new(0),
            gpu_fallbacks: AtomicU64::new(0),
        }
    }

    pub fn set_scaling_filter(&mut self, filter: ScalingFilter) {
        self.cpu.set_scaling_filter(filter);
    }

    pub fn route_counters(&self) -> (u64, u64) {
        (
            self.gpu_attempts.load(Ordering::Relaxed),
            self.gpu_fallbacks.load(Ordering::Relaxed),
        )
    }

    pub fn compose(
        &self,
        frames: &[Option<Frame>],
        layout: CompositionLayout,
    ) -> Result<Frame, CameraManError> {
        if self.cpu.scaling_filter() == ScalingFilter::Nearest
            && frames.len() == 1
            && let Some(frame) = frames[0].as_ref()
            && let Some(gpu) = self.gpu.as_ref()
        {
            self.gpu_attempts.fetch_add(1, Ordering::Relaxed);
            match gpu.compose_single(frame, self.cpu.format()) {
                Ok(output) => return Ok(output),
                Err(_) => {
                    self.gpu_fallbacks.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        self.cpu.compose(frames, layout)
    }
}

fn validate_format(input: &Frame, format: VideoFormat) -> Result<(), GpuExperimentError> {
    if input.pixel_format() != PixelFormat::Bgra8 || format.pixel_format != PixelFormat::Bgra8 {
        return Err(GpuExperimentError::new(
            "only tightly packed BGRA8 is supported",
        ));
    }
    if input.width() == 0 || input.height() == 0 || format.width == 0 || format.height == 0 {
        return Err(GpuExperimentError::new("zero-sized frames are unsupported"));
    }
    Ok(())
}

fn output_byte_len(format: VideoFormat) -> Result<usize, GpuExperimentError> {
    u64::from(format.width)
        .checked_mul(u64::from(format.height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| GpuExperimentError::new("output dimensions overflow address space"))
}

fn aspect_fit_rect(
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
) -> (u32, u32, u32, u32) {
    let scale_x = destination_width as f32 / source_width as f32;
    let scale_y = destination_height as f32 / source_height as f32;
    let scale = scale_x.min(scale_y);
    let width = ((source_width as f32 * scale).round() as u32)
        .max(1)
        .min(destination_width);
    let height = ((source_height as f32 * scale).round() as u32)
        .max(1)
        .min(destination_height);
    (
        (destination_width - width) / 2,
        (destination_height - height) / 2,
        width,
        height,
    )
}

fn u32s_to_le_bytes(values: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(width: u32, height: u32) -> VideoFormat {
        VideoFormat {
            width,
            height,
            fps: 30,
            pixel_format: PixelFormat::Bgra8,
        }
    }

    #[test]
    fn cpu_fallback_preserves_multi_source_contract() {
        let compositor = ExperimentalCompositor::cpu_only(format(4, 2));
        let left = Frame::solid_bgra(1, 1, [1, 2, 3, 255]).unwrap();
        let right = Frame::solid_bgra(1, 1, [4, 5, 6, 255]).unwrap();

        let output = compositor
            .compose(&[Some(left), Some(right)], CompositionLayout::Row)
            .unwrap();

        assert_eq!(output.bgra_at(0, 0), Some([1, 2, 3, 255]));
        assert_eq!(output.bgra_at(3, 0), Some([4, 5, 6, 255]));
        assert_eq!(compositor.route_counters(), (0, 0));
    }

    #[test]
    fn aspect_fit_math_matches_cpu_rounding_contract() {
        assert_eq!(aspect_fit_rect(16, 9, 10, 10), (0, 2, 10, 6));
        assert_eq!(aspect_fit_rect(9, 16, 10, 10), (2, 0, 6, 10));
    }

    #[test]
    fn gpu_output_matches_cpu_when_an_adapter_is_available() {
        let Ok(gpu) = GpuCompositorExperiment::new() else {
            return;
        };
        let output_format = format(8, 6);
        let input = Frame::new_checked(
            2,
            2,
            PixelFormat::Bgra8,
            vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
        )
        .unwrap();
        let expected = Compositor::new(output_format)
            .compose(&[Some(input.clone())], CompositionLayout::Grid)
            .unwrap();

        let actual = gpu.compose_single(&input, output_format).unwrap();

        assert_eq!(actual, expected);
    }
}
