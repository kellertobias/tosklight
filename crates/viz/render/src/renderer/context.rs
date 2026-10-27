//! The common device and output borrowed by a renderer backend for one frame.
use crate::timing::GpuTimer;

pub(super) struct FrameContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub output: &'a wgpu::TextureView,
    pub timer: Option<&'a GpuTimer>,
}
