//! The overlay's own texture atlas and the group that samples it.
//!
//! Every glyph, meter and icon the renderer draws over the picture comes out of one small atlas
//! built at startup, so the status line and Quick Settings cost one pipeline and one draw.

use super::layouts::Layouts;
use wgpu::BufferUsages;

/// The overlay's uniform buffer, atlas and bind group.
pub(super) struct OverlayAtlas {
    pub globals: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl OverlayAtlas {
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layouts: &Layouts,
        icon: Option<&[u8]>,
    ) -> Self {
        let gpu_queue = queue;
        let overlay_globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz overlay globals"),
            size: 16,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas = crate::overlay::build_atlas(icon);
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viz overlay atlas"),
            size: wgpu::Extent3d {
                width: crate::overlay::ATLAS_WIDTH as u32,
                height: crate::overlay::ATLAS_HEIGHT as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu_queue.write_texture(
            atlas_texture.as_image_copy(),
            &atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(crate::overlay::ATLAS_WIDTH as u32 * 4),
                rows_per_image: Some(crate::overlay::ATLAS_HEIGHT as u32),
            },
            wgpu::Extent3d {
                width: crate::overlay::ATLAS_WIDTH as u32,
                height: crate::overlay::ATLAS_HEIGHT as u32,
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viz overlay atlas"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let overlay_layout = &layouts.overlay;
        let overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz overlay"),
            layout: overlay_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: overlay_globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });
        Self {
            globals: overlay_globals,
            bind_group: overlay_bind_group,
        }
    }
}
