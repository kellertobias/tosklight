//! Turning resident clips into the textures an output composites.
//!
//! This is the seam between playback and rendering: a session says which frame, the cache holds
//! that frame still compressed, and this uploads it. Nothing here decides *what* to play — that is
//! the reducer's and the session's business.

use std::collections::{HashMap, HashSet};

use media_domain::AssetId;
use media_domain::SourceFailure;
use media_domain::geometry::Size;
#[cfg(test)]
use media_playback::ClipLoader;
use media_render::{Gpu, SourceTexture};

/// A layer's uploaded frame, kept so an unchanged frame is not uploaded twice.
struct Uploaded {
    asset: AssetId,
    frame: usize,
    size: Size,
    texture: SourceTexture,
}

/// One output's per-layer textures.
///
/// A frame is uploaded when the layer moves to a different one. Holding on a paused or completed
/// layer therefore costs nothing per frame, which is the common case on a running show.
pub struct LayerSources {
    gpu: Gpu,
    uploaded: HashMap<usize, Uploaded>,
    consumers: HashMap<usize, (AssetId, u64)>,
    warned_decode: HashSet<(AssetId, u32, u32)>,
}

impl LayerSources {
    pub fn new(gpu: &Gpu, _output_size: Size) -> Self {
        Self {
            gpu: gpu.clone(),
            uploaded: HashMap::new(),
            consumers: HashMap::new(),
            warned_decode: HashSet::new(),
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
        source_size: Size,
        loader: &mut impl media_playback::MediaLoader,
    ) -> Result<bool, SourceFailure> {
        if !self
            .gpu
            .supports_resolution(source_size.width, source_size.height)
        {
            return Err(SourceFailure::GpuUploadFailed);
        }
        static NEXT_CONSUMER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        if self
            .consumers
            .get(&layer)
            .is_some_and(|(selected, _)| *selected != asset)
        {
            self.release(layer, loader);
        }
        let consumer = self
            .consumers
            .entry(layer)
            .or_insert_with(|| {
                (
                    asset,
                    NEXT_CONSUMER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                )
            })
            .1;
        if self.uploaded.get(&layer).is_some_and(|held| {
            held.asset == asset && held.frame == frame && held.size == source_size
        }) {
            return Ok(true);
        }

        let Some((frame, payload)) = loader.request_frame(asset, frame, consumer) else {
            if let Some(failure) = loader.failure(asset) {
                return Err(failure);
            }
            return Ok(self
                .uploaded
                .get(&layer)
                .is_some_and(|held| held.asset == asset));
        };
        if self.uploaded.get(&layer).is_some_and(|held| {
            held.asset == asset && held.frame == frame && held.size == source_size
        }) {
            return Ok(true);
        }
        let blocks = match media_codec::decode_blocks(
            source_size.width,
            source_size.height,
            payload.as_ref(),
        ) {
            Ok(blocks) => blocks,
            Err(error) => {
                if self
                    .warned_decode
                    .insert((asset, source_size.width, source_size.height))
                {
                    tracing::warn!(%asset, frame, width = source_size.width, height = source_size.height, %error, "a resident frame did not decode");
                }
                return Err(SourceFailure::DecodeFailed);
            }
        };

        // WGPU can only create a BC3 texture when both physical dimensions end on a complete
        // 4x4 block. Clips themselves may have any dimensions, so preserve an odd-sized clip's
        // exact logical size by expanding its edge blocks before upload.
        let block_aligned =
            source_size.width.is_multiple_of(4) && source_size.height.is_multiple_of(4);
        let texture = if self.gpu.samples_block_compression() && block_aligned {
            SourceTexture::from_bc3_blocks(&self.gpu, source_size, &blocks)
        } else {
            // An adapter that cannot sample BC, or a source with partial edge blocks, gets the
            // blocks expanded on the way in. Slower, and still correct, so no platform or source
            // dimension loses the feature.
            match media_codec::hap::expand_to_rgba(source_size.width, source_size.height, &blocks) {
                Ok(rgba) => SourceTexture::from_rgba8(&self.gpu, source_size, &rgba),
                Err(error) => {
                    tracing::warn!(%asset, frame, %error, "a frame could not be expanded");
                    return Err(SourceFailure::DecodeFailed);
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
                        size: source_size,
                        texture,
                    },
                );
                Ok(true)
            }
            Err(error) => {
                tracing::warn!(%asset, frame, %error, "a frame could not be uploaded");
                Err(SourceFailure::GpuUploadFailed)
            }
        }
    }

    /// The texture a layer is showing, if it has one.
    pub fn texture(&self, layer: usize) -> Option<&SourceTexture> {
        self.uploaded.get(&layer).map(|held| &held.texture)
    }

    /// Drops a layer's texture, for a layer that has stopped drawing.
    pub fn release(&mut self, layer: usize, loader: &mut impl media_playback::MediaLoader) {
        if let Some((_, consumer)) = self.consumers.remove(&layer) {
            loader.release_consumer(consumer);
        }
        self.uploaded.remove(&layer);
    }

    /// Rebuilds for a new source resolution. Existing uploads are dropped rather than reused at
    /// the wrong size.
    pub fn resize(&mut self, _output_size: Size) {
        // Source textures keep their intrinsic dimensions. Only the compositor target changes.
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use media_codec::ResidentClip;
    use media_domain::{
        LayerState, MasterState, OutputId, PresentationMode, ScalingMode, Timestamp,
    };
    use media_render::{LayerDraw, OutputRenderer};

    use super::*;

    fn resident(size: Size, colour: [u8; 4]) -> ResidentClip {
        let rgba = colour.repeat(size.width as usize * size.height as usize);
        let frame = media_codec::encode(size.width, size.height, &rgba).expect("encodes");
        ResidentClip::new(vec![Arc::from(frame.into_boxed_slice())])
    }

    #[test]
    fn mixed_source_sizes_decode_independently_of_the_output() {
        let gpu = Gpu::off_screen().expect("an adapter is available");
        let mut sources = LayerSources::new(&gpu, Size::new(1280, 720));
        let mut cache = ClipLoader::new(16 * 1024 * 1024);
        let portrait = Size::new(13, 29);
        let landscape = Size::new(64, 36);
        let portrait_asset = AssetId::new();
        let landscape_asset = AssetId::new();
        cache
            .cache_mut()
            .admit(portrait_asset, resident(portrait, [255, 0, 0, 255]))
            .unwrap();
        cache
            .cache_mut()
            .admit(landscape_asset, resident(landscape, [0, 255, 0, 255]))
            .unwrap();

        assert_eq!(
            sources.prepare(0, portrait_asset, 0, portrait, &mut cache),
            Ok(true)
        );
        assert_eq!(
            sources.prepare(1, landscape_asset, 0, landscape, &mut cache),
            Ok(true)
        );
        assert_eq!(sources.texture(0).unwrap().size(), portrait);
        assert_eq!(sources.texture(1).unwrap().size(), landscape);
    }

    #[test]
    fn an_odd_portrait_source_fits_a_landscape_output_without_damage() {
        let gpu = Gpu::off_screen().expect("an adapter is available");
        let output_size = Size::new(80, 40);
        let source_size = Size::new(13, 29);
        let asset = AssetId::new();
        let mut sources = LayerSources::new(&gpu, output_size);
        let mut cache = ClipLoader::new(1024 * 1024);
        cache
            .cache_mut()
            .admit(asset, resident(source_size, [255, 0, 0, 255]))
            .unwrap();
        assert_eq!(
            sources.prepare(0, asset, 0, source_size, &mut cache),
            Ok(true)
        );

        let state = LayerState {
            address: media_domain::MediaAddress::new(1, 1),
            source_status: media_domain::SourceStatus::Ready,
            scaling_mode: ScalingMode::Fit,
            ..Default::default()
        };
        let mut renderer = OutputRenderer::off_screen(
            &gpu,
            OutputId::new(),
            output_size,
            PresentationMode::DisplaySynchronized,
        )
        .expect("an output opens");
        renderer.present(
            &[LayerDraw {
                state: &state,
                source: sources.texture(0).unwrap(),
                mask: None,
            }],
            &MasterState::default(),
            None,
            Timestamp::ZERO,
            None,
        );
        let image = renderer.read_image();
        let pixel = |x: usize, y: usize| &image[(y * output_size.width as usize + x) * 4..][..4];

        assert_eq!(pixel(40, 20), [255, 0, 0, 255], "source fills the centre");
        assert_eq!(pixel(0, 20), [0, 0, 0, 255], "Fit keeps side bars");
    }

    #[test]
    fn a_streamed_clip_uploads_when_it_exceeds_the_resident_budget() {
        use media_codec::container::{ClipHeader, ClipWriter};
        let gpu = Gpu::off_screen().expect("an adapter is available");
        let size = Size::new(16, 16);
        let asset = AssetId::new();
        let path = std::env::temp_dir().join(format!("media-streamed-upload-{asset}.toskclip"));
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = ClipWriter::new(
            file,
            ClipHeader {
                width: size.width,
                height: size.height,
                frame_count: 0,
                frame_rate: (30, 1),
                intrinsic_bpm: None,
            },
        )
        .unwrap();
        for (index, color) in [[255, 0, 0, 255], [0, 255, 0, 255]].iter().enumerate() {
            let payload = media_codec::encode(size.width, size.height, &color.repeat(256)).unwrap();
            writer.write_frame(&payload, index as u64 * 33_333).unwrap();
        }
        writer.finish().unwrap();
        let mut loader = ClipLoader::new(1);
        loader.load(asset, &path, &mut |_| {}).unwrap();
        assert_eq!(loader.cache().used(), 0);
        let mut sources = LayerSources::new(&gpu, size);
        for frame in [0, 1, 0] {
            assert_eq!(
                sources.prepare(0, asset, frame, size, &mut loader),
                Ok(true)
            );
            assert_eq!(sources.uploaded[&0].frame, frame);
        }
        loader.release(asset);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn one_bad_asset_is_logged_once_instead_of_once_per_render_frame() {
        let gpu = Gpu::off_screen().expect("an adapter is available");
        let mut sources = LayerSources::new(&gpu, Size::new(1280, 720));
        let mut cache = ClipLoader::new(1024 * 1024);
        let asset = AssetId::new();
        let actual = Size::new(16, 16);
        let wrong = Size::new(17, 16);
        cache
            .cache_mut()
            .admit(asset, resident(actual, [255, 255, 255, 255]))
            .unwrap();

        for _ in 0..3 {
            assert_eq!(
                sources.prepare(0, asset, 0, wrong, &mut cache),
                Err(SourceFailure::DecodeFailed)
            );
        }
        assert_eq!(sources.warned_decode.len(), 1);
    }
}
