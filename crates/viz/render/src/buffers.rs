//! Growable GPU buffers. Every per-frame array grows in place instead of reallocating each
//! frame, so a busy DMX frame never churns GPU memory.

use bytemuck::Pod;
use wgpu::util::DeviceExt;
use wgpu::{Buffer, BufferUsages, Device, Queue};

pub struct DynamicBuffer {
    pub buffer: Buffer,
    capacity: u64,
    usage: BufferUsages,
    label: String,
    pub length: u32,
}

impl DynamicBuffer {
    pub fn new(device: &Device, label: &str, usage: BufferUsages, capacity_bytes: u64) -> Self {
        let capacity = capacity_bytes.max(256);
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: capacity,
                usage: usage | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            usage: usage | BufferUsages::COPY_DST,
            label: label.to_owned(),
            length: 0,
        }
    }

    /// Upload `data`, growing the allocation when needed.
    ///
    /// Returns `true` when the underlying buffer was recreated, which invalidates bind groups.
    pub fn upload<T: Pod>(&mut self, device: &Device, queue: &Queue, data: &[T]) -> bool {
        self.length = data.len() as u32;
        let bytes = bytemuck::cast_slice(data);
        let mut recreated = false;
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two().max(256);
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&self.label),
                size: self.capacity,
                usage: self.usage,
                mapped_at_creation: false,
            });
            recreated = true;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
        recreated
    }

    /// Resize without uploading, used for GPU-written buffers such as the tile light lists.
    pub fn ensure(&mut self, device: &Device, bytes: u64) -> bool {
        if bytes <= self.capacity {
            return false;
        }
        self.capacity = bytes.next_power_of_two().max(256);
        self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&self.label),
            size: self.capacity,
            usage: self.usage,
            mapped_at_creation: false,
        });
        true
    }
}

/// Immutable mesh upload.
pub struct GpuMesh {
    pub vertices: Buffer,
    pub indices: Buffer,
    pub index_count: u32,
}

impl GpuMesh {
    pub fn new(device: &Device, label: &str, mesh: &crate::mesh::MeshData) -> Self {
        Self {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label} vertices")),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label} indices")),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: BufferUsages::INDEX,
            }),
            index_count: mesh.indices.len() as u32,
        }
    }
}
