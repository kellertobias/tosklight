//! Render targets that follow the surface size and the active quality tier.

use wgpu::{Device, Extent3d, TextureDescriptor, TextureDimension, TextureUsages, TextureView};

pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const TILE_SIZE: u32 = 16;
// Keep typical full concert rigs on the compact path; overflow still evaluates every light.
// At 3200x1800 this uses 23,142,400 bytes, within the renderer's storage-buffer limit.
pub const MAX_LIGHTS_PER_TILE: u32 = 256;

pub struct Targets {
    pub width: u32,
    pub height: u32,
    /// Samples per pixel in the shaded passes. `1` when the adapter offers no multisampling.
    pub samples: u32,
    /// Colour attachment for the shaded passes. Multisampled when `samples > 1`.
    pub hdr: TextureView,
    /// What everything downstream samples: the resolve of [`Self::hdr`], or the same texture
    /// again when there is nothing to resolve.
    pub shaded: TextureView,
    pub depth: TextureView,
    pub bloom_a: TextureView,
    pub bloom_b: TextureView,
    pub tiles_x: u32,
    pub tiles_y: u32,
}

impl Targets {
    pub fn new(device: &Device, width: u32, height: u32, samples: u32) -> Self {
        Self::with_bloom(device, width, height, samples, true)
    }

    /// Draft never renders bloom. Reuse its HDR views for the required, zero-weight post binding.
    /// This allocates no bloom textures and never schedules a pass writing those alias views.
    pub(crate) fn new_draft(device: &Device, width: u32, height: u32) -> Self {
        Self::with_bloom(device, width, height, 1, false)
    }

    fn with_bloom(device: &Device, width: u32, height: u32, samples: u32, bloom: bool) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let samples = samples.max(1);
        let hdr_texture = colour_texture(device, "viz hdr", width, height, HDR_FORMAT, samples);
        let hdr = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // With one sample there is nothing to resolve, so the attachment is also the source.
        let shaded = if samples > 1 {
            colour_texture(device, "viz hdr resolve", width, height, HDR_FORMAT, 1)
                .create_view(&wgpu::TextureViewDescriptor::default())
        } else {
            hdr_texture.create_view(&wgpu::TextureViewDescriptor::default())
        };
        let depth = depth_target(device, width, height, samples);
        let (bloom_a, bloom_b) =
            if let Some((bloom_width, bloom_height)) = bloom_size(width, height, bloom) {
                let a = colour_texture(
                    device,
                    "viz bloom a",
                    bloom_width,
                    bloom_height,
                    HDR_FORMAT,
                    1,
                )
                .create_view(&wgpu::TextureViewDescriptor::default());
                let b = colour_texture(
                    device,
                    "viz bloom b",
                    bloom_width,
                    bloom_height,
                    HDR_FORMAT,
                    1,
                )
                .create_view(&wgpu::TextureViewDescriptor::default());
                (a, b)
            } else {
                (shaded.clone(), shaded.clone())
            };
        Self {
            width,
            height,
            samples,
            hdr,
            shaded,
            depth,
            bloom_a,
            bloom_b,
            tiles_x: width.div_ceil(TILE_SIZE),
            tiles_y: height.div_ceil(TILE_SIZE),
        }
    }

    pub fn tile_count(&self) -> u32 {
        self.tiles_x * self.tiles_y
    }

    /// Where the shaded passes resolve to, or `None` when they are already single-sampled.
    pub fn resolve_target(&self) -> Option<&TextureView> {
        (self.samples > 1).then_some(&self.shaded)
    }
}

fn colour_texture(
    device: &Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    samples: u32,
) -> wgpu::Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn depth_target(device: &Device, width: u32, height: u32, samples: u32) -> TextureView {
    device
        .create_texture(&TextureDescriptor {
            label: Some("viz depth"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn bloom_size(width: u32, height: u32, enabled: bool) -> Option<(u32, u32)> {
    enabled.then_some(((width / 2).max(1), (height / 2).max(1)))
}
#[cfg(test)]
mod tests {
    #[test]
    fn draft_allocates_no_bloom_extent_and_other_tiers_keep_existing_half_targets() {
        assert_eq!(super::bloom_size(1920, 1080, false), None);
        assert_eq!(super::bloom_size(1920, 1080, true), Some((960, 540)));
        assert_eq!(super::bloom_size(513, 289, true), Some((256, 144)));
        assert_eq!(super::bloom_size(1, 1, true), Some((1, 1)));
    }
}
