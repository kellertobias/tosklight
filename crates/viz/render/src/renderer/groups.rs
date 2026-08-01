//! The bind groups every pass reads its resources through.
//!
//! One place where the renderer's buffers, atlases and render targets are wired to the layouts
//! the shaders were compiled against. Rebuilt whenever a buffer outgrows itself, which is why the
//! wiring is worth having in one piece rather than spread down a constructor.

use super::buffers::SceneBuffers;
use super::build_haze_volume;
use super::layouts::ShadowResources;
use super::layouts::{Layouts, build_scene_group, build_shadow_group, post_group};
use crate::gobos::GoboAtlas;
use crate::targets::Targets;

/// Every bind group a frame binds, and the two samplers they are built with.
pub(super) struct Groups {
    pub scene: wgpu::BindGroup,
    pub cull: wgpu::BindGroup,
    pub depth: wgpu::BindGroup,
    pub shadow: wgpu::BindGroup,
    pub shadow_draw: wgpu::BindGroup,
    pub bloom_extract: wgpu::BindGroup,
    pub bloom_blur_a: wgpu::BindGroup,
    pub bloom_blur_b: wgpu::BindGroup,
    pub composite_source: wgpu::BindGroup,
    pub composite_bloom: wgpu::BindGroup,
    pub gobo_atlas: GoboAtlas,
    pub gobo_sampler: wgpu::Sampler,
    pub haze_view: wgpu::TextureView,
    pub haze_sampler: wgpu::Sampler,
}

impl Groups {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layouts: &Layouts,
        buffers: &SceneBuffers,
        shadow: &ShadowResources,
        targets: &Targets,
        sampler: &wgpu::Sampler,
    ) -> Self {
        // An empty rig still needs the binding, so the atlas starts with its one open layer.
        let gobo_atlas = crate::gobos::GoboAtlas::new(device, queue, &[], u64::MAX);
        let gobo_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viz gobo"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_bind_group = build_shadow_group(
            device,
            &layouts.shadow,
            &shadow.atlas,
            &shadow.sampler,
            &shadow.matrices,
            &gobo_atlas.view,
            &gobo_sampler,
        );
        let shadow_draw_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz shadow draw"),
            layout: &layouts.shadow_draw,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow.matrices.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &shadow.draws,
                        offset: 0,
                        size: std::num::NonZeroU64::new(16),
                    }),
                },
            ],
        });

        let scene_bind_group = build_scene_group(
            device,
            &layouts.scene,
            &buffers.globals,
            &buffers.lights,
            &buffers.tile_counts,
            &buffers.tile_lights,
        );
        let cull_bind_group = build_scene_group(
            device,
            &layouts.cull,
            &buffers.globals,
            &buffers.lights,
            &buffers.tile_counts,
            &buffers.tile_lights,
        );
        let (haze_view, haze_sampler) = build_haze_volume(device, queue);
        let depth_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz depth"),
            layout: &layouts.depth,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&targets.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&haze_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&haze_sampler),
                },
            ],
        });
        let bloom_extract_group = post_group(
            device,
            &layouts.post,
            &targets.shaded,
            sampler,
            &buffers.post_settings,
        );
        let bloom_blur_a_group = post_group(
            device,
            &layouts.post,
            &targets.bloom_a,
            sampler,
            &buffers.post_settings,
        );
        let bloom_blur_b_group = post_group(
            device,
            &layouts.post,
            &targets.bloom_b,
            sampler,
            &buffers.post_settings,
        );
        let composite_source_group = post_group(
            device,
            &layouts.post,
            &targets.shaded,
            sampler,
            &buffers.post_settings,
        );
        let composite_bloom_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz composite bloom"),
            layout: &layouts.composite,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&targets.bloom_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        Self {
            scene: scene_bind_group,
            cull: cull_bind_group,
            depth: depth_bind_group,
            shadow: shadow_bind_group,
            shadow_draw: shadow_draw_group,
            bloom_extract: bloom_extract_group,
            bloom_blur_a: bloom_blur_a_group,
            bloom_blur_b: bloom_blur_b_group,
            composite_source: composite_source_group,
            composite_bloom: composite_bloom_group,
            gobo_atlas,
            gobo_sampler,
            haze_view,
            haze_sampler,
        }
    }
}
