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
#[cfg(test)]
use media_playback::ClipLoader;
use media_playback::LayerSessions;
use media_render::{Gpu, LayerDraw, SourceTexture, VisualizerFrame, VisualizerRenderer};

use crate::layer_sources::LayerSources;
use crate::text_sources::TextSources;

/// Where the mask sessions keep the output-level mask, above every layer index.
const MASTER_MASK_SLOT: usize = media_render::MAX_LAYERS;
/// Generated mask targets live above every ordinary layer target. They cannot share an index with
/// the content renderer: a Text/VIS mask on a Text/VIS layer is a second independently updating
/// source, not permission to overwrite the layer's own texture.
const GENERATED_MASK_SLOT_BASE: usize = MASTER_MASK_SLOT + 1;

/// Which texture a prepared layer draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// An uploaded video or still frame.
    Media(usize),
    /// A mask, uploaded through its own sessions.
    Mask(usize),
    /// A visualizer rendered into this layer's generated target.
    Generated(usize),
    /// A text source rasterized for this layer.
    Text(usize),
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
    /// Wall-clock time, which a clock and a target countdown consult. Kept separate from
    /// `now`, so a clock change cannot make a running countdown jump.
    pub now_unix_millis: i64,
    /// `1.0` on the frame a beat landed, fading afterwards.
    pub beat: f32,
    pub bpm: f32,
    pub beat_phase: f32,
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
    text: TextSources,
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
            text: TextSources::new(gpu.clone(), size),
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
        loader: &mut impl media_playback::MediaLoader,
    ) -> Prepared {
        let (catalog, now) = (frame.catalog, frame.now);
        let mut prepared = Prepared::default();

        for (index, layer) in output
            .layers
            .iter()
            .enumerate()
            .take(media_render::MAX_LAYERS)
        {
            if !matches!(layer.address.classify(), AddressClass::Library) {
                self.media.release(index, loader);
                self.uploads.release(index, loader);
            }
            let source = match layer.address.classify() {
                AddressClass::Blank => {
                    // Releasing here is what unpins a clip nothing is showing any more.
                    self.media.reconcile(index, layer, catalog, loader, now);
                    None
                }
                AddressClass::GeneratedVisualizer => {
                    self.generated(index, layer, frame, &mut prepared)
                }
                AddressClass::TextBank => self.text(index, layer, frame, &mut prepared),
                AddressClass::Library => {
                    self.media(index, layer, catalog, loader, now, &mut prepared)
                }
            };

            let Some(source) = source else {
                self.masks.release(index, loader);
                self.mask_uploads.release(index, loader);
                continue;
            };
            prepared.layers.push(PreparedLayer {
                index,
                source,
                mask: self.mask(index, layer, frame, loader),
            });
        }

        prepared.master_mask = self.master_mask(output, frame, loader);
        prepared
    }

    /// A texture a prepared slot points at.
    pub fn texture(&self, slot: Slot) -> Option<&SourceTexture> {
        match slot {
            Slot::Media(layer) => self.uploads.texture(layer),
            Slot::Mask(layer) => self.mask_uploads.texture(layer),
            Slot::Generated(layer) => self.visualizers.target(layer),
            Slot::Text(layer) => self.text.texture(layer),
        }
    }

    /// The draw list, in layer order.
    pub fn draws<'a>(&'a self, output: &'a OutputState, prepared: &Prepared) -> Vec<LayerDraw<'a>> {
        self.draws_from_layers(&output.layers, prepared)
    }

    /// The draw list using effective per-frame layer state. Coordinated effects use this to alter
    /// compositor opacity without mutating the authoritative layer configuration.
    pub fn draws_from_layers<'a>(
        &'a self,
        layers: &'a [LayerState],
        prepared: &Prepared,
    ) -> Vec<LayerDraw<'a>> {
        prepared
            .layers
            .iter()
            .filter_map(|layer| {
                Some(LayerDraw {
                    state: layers.get(layer.index)?,
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
        self.text.resize(size);
    }

    fn media(
        &mut self,
        index: usize,
        layer: &LayerState,
        catalog: &CatalogSnapshot,
        loader: &mut impl media_playback::MediaLoader,
        now: Timestamp,
        prepared: &mut Prepared,
    ) -> Option<Slot> {
        let resolved = self.media.reconcile(index, layer, catalog, loader, now)?;
        let Some(frame) = resolved.frame else {
            prepared.statuses.push((index, resolved.status));
            return None;
        };
        match self.uploads.prepare(
            index,
            resolved.asset,
            frame,
            Size::new(resolved.size.0, resolved.size.1),
            loader,
        ) {
            Ok(true) => {
                prepared.statuses.push((index, resolved.status));
                Some(Slot::Media(index))
            }
            Ok(false) => {
                prepared
                    .statuses
                    .push((index, media_domain::SourceStatus::Loading));
                None
            }
            Err(failure) => {
                prepared
                    .statuses
                    .push((index, media_domain::SourceStatus::Failed { failure }));
                None
            }
        }
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
            beat: context.beat,
            bpm: context.bpm,
            beat_phase: context.beat_phase,
        };
        let parameters = visualizer_parameters(layer, &visualizer.parameters);
        match self
            .visualizers
            .render(index, visualizer.kind, parameters, &frame)
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

    /// Draws a text entry for a layer that selected one.
    fn text(
        &mut self,
        index: usize,
        layer: &LayerState,
        context: FrameContext<'_>,
        prepared: &mut Prepared,
    ) -> Option<Slot> {
        match self.text.prepare(index, layer, context) {
            Ok(drawn) => {
                prepared
                    .statuses
                    .push((index, media_domain::SourceStatus::Ready));
                drawn.then_some(Slot::Text(index))
            }
            Err(failure) => {
                prepared
                    .statuses
                    .push((index, media_domain::SourceStatus::Failed { failure }));
                None
            }
        }
    }

    fn mask(
        &mut self,
        index: usize,
        layer: &LayerState,
        frame: FrameContext<'_>,
        loader: &mut impl media_playback::MediaLoader,
    ) -> Option<Slot> {
        self.load_mask(
            index,
            layer.mask.address,
            layer.mask.is_active(),
            frame,
            loader,
        )
    }

    fn master_mask(
        &mut self,
        output: &OutputState,
        frame: FrameContext<'_>,
        loader: &mut impl media_playback::MediaLoader,
    ) -> Option<Slot> {
        self.load_mask(
            MASTER_MASK_SLOT,
            output.master.mask,
            output.master.has_mask(),
            frame,
            loader,
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
        frame: FrameContext<'_>,
        loader: &mut impl media_playback::MediaLoader,
    ) -> Option<Slot> {
        let class = address.classify();
        if !active || !matches!(class, AddressClass::Library) {
            self.mask_uploads.release(slot, loader);
            let selection = LayerState::default();
            self.masks
                .reconcile(slot, &selection, frame.catalog, loader, frame.now);
        }
        if !active {
            return None;
        }

        let generated_slot = GENERATED_MASK_SLOT_BASE + slot;
        match class {
            AddressClass::GeneratedVisualizer => {
                let Some(visualizer) = frame.configuration.visualizers.resolve(address) else {
                    tracing::warn!(
                        %address,
                        "a visualizer mask points to an unassigned generated address"
                    );
                    return None;
                };
                let visualizer_frame = VisualizerFrame {
                    seconds: frame.seconds,
                    analysis: frame.analysis,
                    beat: frame.beat,
                    bpm: frame.bpm,
                    beat_phase: frame.beat_phase,
                };
                return self
                    .visualizers
                    .render(
                        generated_slot,
                        visualizer.kind,
                        &visualizer.parameters,
                        &visualizer_frame,
                    )
                    .map(|_| Slot::Generated(generated_slot))
                    .map_err(|error| {
                        tracing::warn!(%address, %error, "a visualizer mask could not be drawn");
                    })
                    .ok();
            }
            AddressClass::TextBank => {
                let selection = LayerState {
                    address,
                    source_status: media_domain::SourceStatus::Ready,
                    play_mode: PlayMode::Loop,
                    ..Default::default()
                };
                return self
                    .text
                    .prepare(generated_slot, &selection, frame)
                    .map(|drawn| drawn.then_some(Slot::Text(generated_slot)))
                    .map_err(|failure| {
                        tracing::warn!(%address, ?failure, "a text mask could not be drawn");
                    })
                    .ok()
                    .flatten();
            }
            AddressClass::Blank => return None,
            AddressClass::Library => {}
        }

        let selection = LayerState {
            address,
            // A mask is a still or a loop; it has no transport of its own to be stopped by the
            // layer's play mode.
            play_mode: PlayMode::Loop,
            ..Default::default()
        };
        let resolved = self
            .masks
            .reconcile(slot, &selection, frame.catalog, loader, frame.now)?;
        let frame = resolved.frame?;
        self.mask_uploads
            .prepare(
                slot,
                resolved.asset,
                frame,
                Size::new(resolved.size.0, resolved.size.1),
                loader,
            )
            .unwrap_or(false)
            .then_some(Slot::Mask(slot))
    }
}

fn visualizer_parameters<'a>(
    layer: &'a LayerState,
    configured: &'a media_domain::VisualizerParameters,
) -> &'a media_domain::VisualizerParameters {
    layer.effects[0]
        .visualizer_parameters
        .as_ref()
        .unwrap_or(configured)
}

#[cfg(test)]
mod tests {
    use media_domain::audio::{Analysis, BANDS, WAVEFORM_POINTS};
    use media_domain::personality::LayerPersonality;
    use media_domain::{
        AnalogTvParameters, EffectSlot, OutputId, OutputState, PresentationMode, ScalingMode,
        SourceStatus, Tint,
    };
    use media_render::OutputRenderer;

    use super::*;

    // Masks and layers share one index space in the upload maps, so the output-level mask must sit
    // above every layer index a personality can produce. A layer that overwrote the master mask's
    // texture would show the wrong shape on the whole output.
    const _: () = assert!(MASTER_MASK_SLOT >= media_render::MAX_LAYERS);

    #[test]
    fn a_layer_visualizer_override_is_the_parameter_block_sent_to_the_renderer() {
        let configured = media_domain::VisualizerParameters::default();
        let mut overridden = configured;
        overridden.size = 0.2;
        let mut layer = LayerState::default();
        layer.effects[0].visualizer_parameters = Some(overridden);

        assert_eq!(visualizer_parameters(&layer, &configured), &overridden);
        layer.effects[0].visualizer_parameters = None;
        assert_eq!(visualizer_parameters(&layer, &configured), &configured);
    }

    #[test]
    fn a_slot_names_exactly_one_texture_store() {
        // Three stores share one layer index; a slot is what keeps them apart. Two slots for the
        // same layer must not be equal, or a mask would be drawn as its own layer's source.
        assert_ne!(Slot::Media(0), Slot::Mask(0));
        assert_ne!(Slot::Media(0), Slot::Generated(0));
        assert_ne!(Slot::Mask(0), Slot::Generated(0));
        assert_ne!(
            Slot::Generated(0),
            Slot::Generated(GENERATED_MASK_SLOT_BASE),
            "a generated layer and its generated mask keep independent targets"
        );
    }

    fn driven_analysis() -> Analysis {
        Analysis {
            waveform: (0..WAVEFORM_POINTS)
                .map(|index| (index as f32 * 0.05).sin())
                .collect(),
            spectrum: (0..BANDS)
                .map(|index| 0.9 - index as f32 / BANDS as f32 * 0.5)
                .collect(),
            bass: 0.9,
            mid: 0.8,
            treble: 0.7,
            energy: 0.85,
            peak: 1.0,
        }
    }

    fn changed_pixels(first: &[u8], second: &[u8]) -> usize {
        first
            .as_chunks::<4>()
            .0
            .iter()
            .zip(second.as_chunks::<4>().0.iter())
            .filter(|(left, right)| left != right)
            .count()
    }

    fn assert_generated_controls(address: MediaAddress, label: &str) {
        let size = Size::new(64, 64);
        let gpu = Gpu::off_screen().expect("an adapter is available");
        let output_id = OutputId::new();
        let mut pipeline = LayerPipeline::new(
            &gpu,
            output_id,
            media_library::LibraryStorage::new(std::path::PathBuf::from("unused")),
            size,
        );
        let configuration = MediaConfiguration::default();
        let catalog = CatalogSnapshot::default();
        let analysis = driven_analysis();
        let mut loader = ClipLoader::new(1024 * 1024);
        let mut output = OutputState::new(output_id, LayerPersonality::TwoLayers);
        output.layers[0] = LayerState {
            address,
            source_status: SourceStatus::Ready,
            scaling_mode: ScalingMode::Stretch,
            ..Default::default()
        };
        let context = FrameContext {
            catalog: &catalog,
            configuration: &configuration,
            analysis: &analysis,
            now_unix_millis: 1_700_000_000_000,
            beat: 1.0,
            bpm: 128.0,
            beat_phase: 0.25,
            seconds: 1.25,
            now: Timestamp::from_millis(1_250),
        };
        let prepared = pipeline.prepare(&output, context, &mut loader);
        let prepared_layer = prepared.layers.first().unwrap_or_else(|| {
            panic!(
                "{label} produced no compositor source: {:?}",
                prepared.statuses
            )
        });
        let source = pipeline
            .texture(prepared_layer.source)
            .unwrap_or_else(|| panic!("{label} produced no texture"));
        let black_mask =
            SourceTexture::solid(&gpu, Size::new(1, 1), [0, 0, 0, 255]).expect("a mask uploads");
        let mut renderer = OutputRenderer::off_screen(
            &gpu,
            output_id,
            size,
            PresentationMode::DisplaySynchronized,
        )
        .expect("an output opens");
        let mut render = |state: &LayerState, mask: Option<&SourceTexture>| {
            renderer.present(
                &[LayerDraw {
                    state,
                    source,
                    mask,
                }],
                &media_domain::MasterState::default(),
                None,
                Timestamp::from_millis(1_250),
                None,
            );
            renderer.read_image()
        };

        let baseline = render(&output.layers[0], None);
        assert!(
            baseline
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[..3] != [0, 0, 0]),
            "{label} baseline is empty"
        );

        let framed = LayerState {
            scale_x: 0.35,
            position_x: 0.6,
            ..output.layers[0].clone()
        };
        assert!(
            changed_pixels(&baseline, &render(&framed, None)) > 100,
            "Frame controls do not affect {label}"
        );

        let coloured = LayerState {
            tint: Tint::new(0.0, 0.0, 0.0),
            grayscale: 1.0,
            ..output.layers[0].clone()
        };
        assert!(
            changed_pixels(&baseline, &render(&coloured, None)) > 100,
            "Colour controls do not affect {label}"
        );

        let dimmed = LayerState {
            dimmer: 0.0,
            ..output.layers[0].clone()
        };
        assert!(
            changed_pixels(&baseline, &render(&dimmed, None)) > 100,
            "Dimmer does not affect {label}"
        );

        let masked = LayerState {
            mask: media_domain::MaskState {
                address: MediaAddress::new(1, 1),
                opacity: 1.0,
                ..Default::default()
            },
            ..output.layers[0].clone()
        };
        assert!(
            changed_pixels(&baseline, &render(&masked, Some(&black_mask))) > 100,
            "Mask controls do not affect {label}"
        );

        let mut effect = EffectSlot::analog_tv();
        effect.parameters = AnalogTvParameters {
            curvature: 1.0,
            distortion: 0.0,
            image_grain: 0.0,
            glitching: 0.0,
        }
        .as_array()
        .to_vec();
        let mut effects: [EffectSlot; 4] = Default::default();
        effects[0] = effect;
        let effected = LayerState {
            effects,
            ..output.layers[0].clone()
        };
        assert!(
            changed_pixels(&baseline, &render(&effected, None)) > 100,
            "Effects do not affect {label}"
        );
    }

    #[test]
    fn text_and_visualizers_share_the_complete_layer_compositor() {
        assert_generated_controls(MediaAddress::new(200, 1), "Text");
        assert_generated_controls(MediaAddress::new(250, 1), "Visualizer");
    }

    #[test]
    fn text_and_visualizers_resolve_as_independent_layer_and_master_masks() {
        let size = Size::new(64, 64);
        let gpu = Gpu::off_screen().expect("an adapter is available");
        let output_id = OutputId::new();
        let mut pipeline = LayerPipeline::new(
            &gpu,
            output_id,
            media_library::LibraryStorage::new(std::path::PathBuf::from("unused")),
            size,
        );
        let configuration = MediaConfiguration::default();
        let catalog = CatalogSnapshot::default();
        let analysis = driven_analysis();
        let mut loader = ClipLoader::new(1024 * 1024);
        let context = FrameContext {
            catalog: &catalog,
            configuration: &configuration,
            analysis: &analysis,
            now_unix_millis: 1_700_000_000_000,
            beat: 1.0,
            bpm: 128.0,
            beat_phase: 0.25,
            seconds: 1.25,
            now: Timestamp::from_millis(1_250),
        };
        let mut output = OutputState::new(output_id, LayerPersonality::TwoLayers);
        output.layers[0] = LayerState {
            address: MediaAddress::new(250, 1),
            source_status: SourceStatus::Ready,
            scaling_mode: ScalingMode::Stretch,
            mask: media_domain::MaskState {
                address: MediaAddress::new(200, 1),
                opacity: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let prepared = pipeline.prepare(&output, context, &mut loader);
        let layer = &prepared.layers[0];
        assert!(matches!(layer.source, Slot::Generated(0)));
        assert!(matches!(layer.mask, Some(Slot::Text(index)) if index >= GENERATED_MASK_SLOT_BASE));
        assert!(layer.mask.and_then(|slot| pipeline.texture(slot)).is_some());

        output.layers[0].address = MediaAddress::new(200, 1);
        output.layers[0].mask.address = MediaAddress::new(250, 1);
        output.master.mask = MediaAddress::new(250, 1);
        let prepared = pipeline.prepare(&output, context, &mut loader);
        let layer = &prepared.layers[0];
        assert!(matches!(layer.source, Slot::Text(0)));
        assert!(
            matches!(layer.mask, Some(Slot::Generated(index)) if index >= GENERATED_MASK_SLOT_BASE)
        );
        assert!(
            matches!(prepared.master_mask, Some(Slot::Generated(index)) if index > GENERATED_MASK_SLOT_BASE)
        );
        assert!(
            prepared
                .master_mask
                .and_then(|slot| pipeline.texture(slot))
                .is_some()
        );

        output.master.mask = MediaAddress::new(200, 1);
        let prepared = pipeline.prepare(&output, context, &mut loader);
        assert!(
            matches!(prepared.master_mask, Some(Slot::Text(index)) if index > GENERATED_MASK_SLOT_BASE)
        );
        assert!(
            prepared
                .master_mask
                .and_then(|slot| pipeline.texture(slot))
                .is_some()
        );
    }
}
