//! One output's path from addresses to textures.
//!
//! This is where the product's core loop closes. A desk sets an address; the reducer stores it;
//! this resolves it — through the catalog for library media, through the generated catalog for a
//! visualizer — makes it resident, picks the frame, and hands the compositor something to draw.
//!
//! Masks go through exactly the same path as media, because a mask *is* media: a second set of
//! sessions selecting the mask's own address, so a mask loads, fails, and reports like any other
//! source instead of being a special case that silently does nothing.

use media_application::MediaConfiguration;
use media_domain::audio::Analysis;
use media_domain::catalog::CatalogSnapshot;
use media_domain::geometry::Size;
use media_domain::{AddressClass, LayerState, MediaAddress, OutputState, PlayMode, Timestamp};
use media_playback::{ClipLoader, LayerSessions};
use media_render::{Gpu, LayerDraw, SourceTexture, VisualizerFrame, VisualizerRenderer};

use crate::layer_sources::LayerSources;

/// Where the mask sessions keep the output-level mask, above every layer index.
const MASTER_MASK_SLOT: usize = media_render::MAX_LAYERS;

/// Which texture a prepared layer draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// An uploaded video or still frame.
    Media(usize),
    /// A mask, uploaded through its own sessions.
    Mask(usize),
    /// A visualizer rendered into this layer's generated target.
    Generated(usize),
}

/// One layer that will draw this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedLayer {
    pub index: usize,
    pub source: Slot,
    pub mask: Option<Slot>,
}

/// What every layer of one output is resolved against this frame.
///
/// Grouped rather than passed one by one, because these five always travel together: they are one
/// instant, and mixing an old catalog with a new timestamp would resolve an address against a
/// library that no longer exists.
#[derive(Clone, Copy)]
pub struct FrameContext<'a> {
    pub catalog: &'a CatalogSnapshot,
    pub configuration: &'a MediaConfiguration,
    pub analysis: &'a Analysis,
    /// Seconds since the process started, for time-driven generated sources.
    pub seconds: f32,
    pub now: Timestamp,
}

/// Everything one output needs to present this frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prepared {
    pub layers: Vec<PreparedLayer>,
    pub master_mask: Option<Slot>,
    /// What to tell the reducer about each layer's source, so the API, the UI, and CITP all see
    /// the same lifecycle the renderer saw.
    pub statuses: Vec<(usize, media_domain::SourceStatus)>,
}

/// One output's sessions, uploads, and generated sources.
pub struct LayerPipeline {
    media: LayerSessions,
    uploads: LayerSources,
    masks: LayerSessions,
    mask_uploads: LayerSources,
    visualizers: VisualizerRenderer,
}

impl LayerPipeline {
    pub fn new(
        gpu: &Gpu,
        output: media_domain::OutputId,
        storage: media_library::LibraryStorage,
        size: Size,
    ) -> Self {
        Self {
            media: LayerSessions::new(output, storage.clone()),
            uploads: LayerSources::new(gpu, size),
            masks: LayerSessions::new(output, storage),
            mask_uploads: LayerSources::new(gpu, size),
            visualizers: VisualizerRenderer::new(gpu, size),
        }
    }

    /// Compiles every visualizer, so a backend that cannot build one says so at startup rather
    /// than when an operator selects it mid-show.
    pub fn validate_visualizers(&mut self) {
        for (kind, outcome) in self.visualizers.validate() {
            if let Err(error) = outcome {
                tracing::error!(visualizer = kind.label(), %error, "this machine cannot draw a visualizer");
            }
        }
    }

    /// Resolves every layer of one output and prepares its textures.
    pub fn prepare(
        &mut self,
        output: &OutputState,
        frame: FrameContext<'_>,
        loader: &mut ClipLoader,
    ) -> Prepared {
        let (catalog, now) = (frame.catalog, frame.now);
        let mut prepared = Prepared::default();

        for (index, layer) in output
            .layers
            .iter()
            .enumerate()
            .take(media_render::MAX_LAYERS)
        {
            let source = match layer.address.classify() {
                AddressClass::Blank => {
                    // Releasing here is what unpins a clip nothing is showing any more.
                    self.media.reconcile(index, layer, catalog, loader, now);
                    None
                }
                AddressClass::GeneratedVisualizer => {
                    self.generated(index, layer, frame, &mut prepared)
                }
                // Text banks resolve once text sources have a renderer; until then the address is
                // kept and the layer reports that it selected something that is not there, which
                // is the truth rather than a silent blank.
                AddressClass::TextBank | AddressClass::Library => {
                    self.media(index, layer, catalog, loader, now, &mut prepared)
                }
            };

            let Some(source) = source else { continue };
            prepared.layers.push(PreparedLayer {
                index,
                source,
                mask: self.mask(index, layer, catalog, loader, now),
            });
        }

        prepared.master_mask = self.master_mask(output, catalog, loader, now);
        prepared
    }

    /// A texture a prepared slot points at.
    pub fn texture(&self, slot: Slot) -> Option<&SourceTexture> {
        match slot {
            Slot::Media(layer) => self.uploads.texture(layer),
            Slot::Mask(layer) => self.mask_uploads.texture(layer),
            Slot::Generated(layer) => self.visualizers.target(layer),
        }
    }

    /// The draw list, in layer order.
    pub fn draws<'a>(&'a self, output: &'a OutputState, prepared: &Prepared) -> Vec<LayerDraw<'a>> {
        prepared
            .layers
            .iter()
            .filter_map(|layer| {
                Some(LayerDraw {
                    state: output.layer(layer.index)?,
                    source: self.texture(layer.source)?,
                    mask: layer.mask.and_then(|slot| self.texture(slot)),
                })
            })
            .collect()
    }

    pub fn resize(&mut self, size: Size) {
        self.uploads.resize(size);
        self.mask_uploads.resize(size);
        self.visualizers.resize(size);
    }

    fn media(
        &mut self,
        index: usize,
        layer: &LayerState,
        catalog: &CatalogSnapshot,
        loader: &mut ClipLoader,
        now: Timestamp,
        prepared: &mut Prepared,
    ) -> Option<Slot> {
        let resolved = self.media.reconcile(index, layer, catalog, loader, now)?;
        prepared.statuses.push((index, resolved.status));

        let frame = resolved.frame?;
        self.uploads
            .prepare(index, resolved.asset, frame, loader.cache_mut())
            .then_some(Slot::Media(index))
    }

    fn generated(
        &mut self,
        index: usize,
        layer: &LayerState,
        context: FrameContext<'_>,
        prepared: &mut Prepared,
    ) -> Option<Slot> {
        let Some(visualizer) = context.configuration.visualizers.resolve(layer.address) else {
            // An address inside the generated range with nothing assigned to it is a missing
            // source, exactly as a missing file is.
            prepared.statuses.push((
                index,
                media_domain::SourceStatus::Failed {
                    failure: media_domain::SourceFailure::MissingFile,
                },
            ));
            return None;
        };

        let frame = VisualizerFrame {
            seconds: context.seconds,
            analysis: context.analysis,
            // Audio capture is product- and platform-owned and arrives with its own slice. Until
            // it does, the analysis is silence: time-driven visualizers still run, and the
            // audio-driven ones sit still rather than inventing movement.
            beat: 0.0,
            bpm: 0.0,
            beat_phase: 0.0,
        };
        match self
            .visualizers
            .render(index, visualizer.kind, &visualizer.parameters, &frame)
        {
            Ok(_) => {
                prepared
                    .statuses
                    .push((index, media_domain::SourceStatus::Ready));
                Some(Slot::Generated(index))
            }
            Err(error) => {
                tracing::warn!(address = %layer.address, %error, "a visualizer could not be drawn");
                prepared.statuses.push((
                    index,
                    media_domain::SourceStatus::Failed {
                        failure: media_domain::SourceFailure::GpuUploadFailed,
                    },
                ));
                None
            }
        }
    }

    fn mask(
        &mut self,
        index: usize,
        layer: &LayerState,
        catalog: &CatalogSnapshot,
        loader: &mut ClipLoader,
        now: Timestamp,
    ) -> Option<Slot> {
        self.load_mask(
            index,
            layer.mask.address,
            layer.mask.is_active(),
            catalog,
            loader,
            now,
        )
    }

    fn master_mask(
        &mut self,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        loader: &mut ClipLoader,
        now: Timestamp,
    ) -> Option<Slot> {
        self.load_mask(
            MASTER_MASK_SLOT,
            output.master.mask,
            output.master.has_mask(),
            catalog,
            loader,
            now,
        )
    }

    /// Loads a mask through the same path as media, or releases the slot when there is none.
    ///
    /// A mask that has not loaded returns nothing, and a layer with no mask texture draws
    /// unmasked. A missing mask means no mask — never a black layer.
    fn load_mask(
        &mut self,
        slot: usize,
        address: MediaAddress,
        active: bool,
        catalog: &CatalogSnapshot,
        loader: &mut ClipLoader,
        now: Timestamp,
    ) -> Option<Slot> {
        let selection = LayerState {
            address: if active { address } else { MediaAddress::BLANK },
            // A mask is a still or a loop; it has no transport of its own to be stopped by the
            // layer's play mode.
            play_mode: PlayMode::Loop,
            ..Default::default()
        };
        let resolved = self
            .masks
            .reconcile(slot, &selection, catalog, loader, now)?;
        let frame = resolved.frame?;
        self.mask_uploads
            .prepare(slot, resolved.asset, frame, loader.cache_mut())
            .then_some(Slot::Mask(slot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Masks and layers share one index space in the upload maps, so the output-level mask must sit
    // above every layer index a personality can produce. A layer that overwrote the master mask's
    // texture would show the wrong shape on the whole output.
    const _: () = assert!(MASTER_MASK_SLOT >= media_render::MAX_LAYERS);

    #[test]
    fn a_slot_names_exactly_one_texture_store() {
        // Three stores share one layer index; a slot is what keeps them apart. Two slots for the
        // same layer must not be equal, or a mask would be drawn as its own layer's source.
        assert_ne!(Slot::Media(0), Slot::Mask(0));
        assert_ne!(Slot::Media(0), Slot::Generated(0));
        assert_ne!(Slot::Mask(0), Slot::Generated(0));
    }
}
