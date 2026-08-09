//! Turning resident clips into the textures an output composites.
//!
//! This is the seam between playback and rendering: a session says which frame, the cache holds
//! that frame still compressed, and this uploads it. Nothing here decides *what* to play — that is
//! the reducer's and the session's business.

use std::collections::HashMap;

use media_codec::ClipCache;
use media_domain::AssetId;
use media_domain::geometry::Size;
use media_render::{Gpu, SourceTexture};

/// A layer's uploaded frame, kept so an unchanged frame is not uploaded twice.
struct Uploaded {
    asset: AssetId,
    frame: usize,
    texture: SourceTexture,
}

/// One output's per-layer textures.
///
/// A frame is uploaded when the layer moves to a different one. Holding on a paused or completed
/// layer therefore costs nothing per frame, which is the common case on a running show.
pub struct LayerSources {
    gpu: Gpu,
    size: Size,
    uploaded: HashMap<usize, Uploaded>,
}

impl LayerSources {
    pub fn new(gpu: &Gpu, size: Size) -> Self {
        Self {
            gpu: gpu.clone(),
            size,
            uploaded: HashMap::new(),
        }
    }

    /// Uploads the frame a layer wants, if it is not already on the GPU.
    ///
    /// Returns whether a texture is available for that layer. A frame that is not resident yields
    /// nothing rather than blocking the render thread on storage — the layer simply keeps showing
    /// what it had, and the loader is what makes it resident.
    pub fn prepare(
        &mut self,
        layer: usize,
        asset: AssetId,
        frame: usize,
        cache: &mut ClipCache,
    ) -> bool {
        if self
            .uploaded
            .get(&layer)
            .is_some_and(|held| held.asset == asset && held.frame == frame)
        {
            return true;
        }

        let Some(payload) = cache.frame(asset, frame) else {
            return self.uploaded.contains_key(&layer);
        };
        let Ok(blocks) =
            media_codec::decode_blocks(self.size.width, self.size.height, payload.as_ref())
        else {
            tracing::warn!(%asset, frame, "a resident frame did not decompress");
            return false;
        };

        let texture = if self.gpu.samples_block_compression() {
            SourceTexture::from_bc3_blocks(&self.gpu, self.size, &blocks)
        } else {
            // An adapter that cannot sample BC gets the blocks expanded on the way in. Slower, and
            // still correct, so no platform loses the feature.
            match media_codec::hap::expand_to_rgba(self.size.width, self.size.height, &blocks) {
                Ok(rgba) => SourceTexture::from_rgba8(&self.gpu, self.size, &rgba),
                Err(error) => {
                    tracing::warn!(%asset, frame, %error, "a frame could not be expanded");
                    return false;
                }
            }
        };

        match texture {
            Ok(texture) => {
                self.uploaded.insert(
                    layer,
                    Uploaded {
                        asset,
                        frame,
                        texture,
                    },
                );
                true
            }
            Err(error) => {
                tracing::warn!(%asset, frame, %error, "a frame could not be uploaded");
                false
            }
        }
    }

    /// The texture a layer is showing, if it has one.
    pub fn texture(&self, layer: usize) -> Option<&SourceTexture> {
        self.uploaded.get(&layer).map(|held| &held.texture)
    }

    /// Drops a layer's texture, for a layer that has stopped drawing.
    pub fn release(&mut self, layer: usize) {
        self.uploaded.remove(&layer);
    }

    /// Rebuilds for a new source resolution. Existing uploads are dropped rather than reused at
    /// the wrong size.
    pub fn resize(&mut self, size: Size) {
        if size != self.size {
            self.size = size;
            self.uploaded.clear();
        }
    }
}
