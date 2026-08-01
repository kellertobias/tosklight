//! The rig's gobo artwork, as one GPU array the lighting passes sample.
//!
//! Every piece of glass in the scene is the same size by the time it gets here — the projection
//! layer resamples them — so the whole library fits one array texture and a beam picks its slot
//! with a layer index rather than a bind-group change per fixture. A scene with no artwork still
//! gets an array, because a shader cannot have a binding that is sometimes absent; it gets one
//! opaque layer that nothing points at.

use viz_scene::GoboArtwork;
use wgpu::{Device, Extent3d, Queue, TextureView};

/// The artwork uploaded for one scene.
pub struct GoboAtlas {
    pub view: TextureView,
    /// Scene revision this was built for, so a rig that has not changed is not re-uploaded.
    pub revision: u64,
    pub layers: u32,
}

impl GoboAtlas {
    pub fn new(device: &Device, queue: &Queue, artwork: &[GoboArtwork], revision: u64) -> Self {
        // One fully open layer stands in for an empty library: nothing samples it, and it keeps
        // the binding valid.
        let edge = artwork.first().map_or(4, |first| first.edge.max(1));
        let layers = artwork.len().max(1) as u32;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viz gobo artwork"),
            size: Extent3d {
                width: edge,
                height: edge,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let open = vec![255_u8; (edge * edge) as usize];
        for layer in 0..layers {
            let mask = artwork
                .get(layer as usize)
                .map_or(open.as_slice(), |image| image.mask.as_slice());
            // A layer whose mask is the wrong length would sample as noise, so it is drawn as
            // open glass instead: a slot that projects nothing is better than one that projects
            // a mistake.
            let mask = if mask.len() == (edge * edge) as usize {
                mask
            } else {
                open.as_slice()
            };
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                mask,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(edge),
                    rows_per_image: Some(edge),
                },
                Extent3d {
                    width: edge,
                    height: edge,
                    depth_or_array_layers: 1,
                },
            );
        }
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            revision,
            layers,
        }
    }

    /// Whether this atlas still describes `scene`.
    pub fn matches(&self, artwork: &[GoboArtwork], revision: u64) -> bool {
        self.revision == revision && self.layers == artwork.len().max(1) as u32
    }
}
