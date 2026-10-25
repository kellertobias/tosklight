//! One bounded scratch record per uploaded light, reused by all screen tiles.
use crate::buffers::DynamicBuffer;
use wgpu::{BindGroup, BindGroupLayout, BufferUsages, Device};
const BOUNDS_STRIDE: u64 = 32;
pub(super) struct ScreenBounds {
    buffer: DynamicBuffer,
    layout: BindGroupLayout,
    pub group: BindGroup,
}
impl ScreenBounds {
    pub fn new(device: &Device, layout: &BindGroupLayout) -> Self {
        let buffer = DynamicBuffer::new(
            device,
            "viz per-light screen bounds",
            BufferUsages::STORAGE,
            256,
        );
        let group = group(device, layout, &buffer);
        Self {
            buffer,
            layout: layout.clone(),
            group,
        }
    }
    pub fn ensure(&mut self, device: &Device, light_count: u32) {
        // The already uploaded GpuLight array is larger per light than this scratch. The same
        // accepted light count therefore fits the device storage binding/allocation limits.
        if self
            .buffer
            .ensure(device, u64::from(light_count) * BOUNDS_STRIDE)
        {
            self.group = group(device, &self.layout, &self.buffer);
        }
    }
}
pub(super) fn layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viz per-light bounds layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(BOUNDS_STRIDE),
            },
            count: None,
        }],
    })
}
fn group(device: &Device, layout: &BindGroupLayout, buffer: &DynamicBuffer) -> BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viz per-light bounds group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.buffer.as_entire_binding(),
        }],
    })
}
#[cfg(test)]
mod tests;
