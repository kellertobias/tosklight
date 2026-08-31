//! Outputs with no window.
//!
//! An off-screen output is a real output: it composites every frame on its own clock, reports its
//! own cadence, and can be previewed over CITP. It simply has nowhere to present. That matters for
//! more than tests — a rack server driving a capture card, a machine whose display has not been
//! configured yet, and every automated check of multi-output behaviour all run this way.
//!
//! It runs on its own thread with its own device, so it never competes with the platform event
//! loop a windowed output needs on the main thread.

use std::sync::Arc;

use media_application::MediaConfiguration;
use media_application::configuration::{OutputConfiguration, OutputTarget};
use media_domain::geometry::Size;
use media_domain::{MediaState, Timestamp};
use media_playback::ClipLoader;
use media_render::{Gpu, LayerDraw, OutputRenderer};

use crate::layer_pipeline::{FrameContext, LayerPipeline};
use crate::presentation::Shared;
use crate::shutdown::Shutdown;

/// Whether this configuration has any output to run here.
pub fn any(configuration: &MediaConfiguration) -> bool {
    configuration
        .outputs
        .iter()
        .any(|output| output.enabled && matches!(output.target, OutputTarget::OffScreen))
}

/// One off-screen output and everything it needs to composite.
struct Hosted {
    renderer: OutputRenderer,
    pipeline: LayerPipeline,
    configuration: OutputConfiguration,
    last_preview_millis: Option<u64>,
}

/// Runs every off-screen output until shutdown.
///
/// Returns immediately when there are none, and reports rather than fails when this machine has no
/// GPU at all: an output that cannot render must not stop the ones that can, or the API.
/// The slice of the canvas this output's surface shows.
fn shown_region(
    configuration: &OutputConfiguration,
) -> Option<&media_domain::display_region::DisplayRegion> {
    configuration
        .pixel_map
        .regions
        .iter()
        .find(|region| region.enabled)
}

pub fn run(configuration: &MediaConfiguration, shared: Shared, shutdown: Shutdown) {
    if !any(configuration) {
        return;
    }
    let gpu = match Gpu::off_screen() {
        Ok(gpu) => gpu,
        Err(error) => {
            tracing::error!(%error, "no graphics device; off-screen outputs cannot render");
            return;
        }
    };

    let mut hosted: Vec<Hosted> = configuration
        .outputs
        .iter()
        .filter(|output| output.enabled && matches!(output.target, OutputTarget::OffScreen))
        .filter_map(|output| open(&gpu, output, configuration))
        .collect();
    if hosted.is_empty() {
        return;
    }
    tracing::info!(outputs = hosted.len(), "off-screen outputs presenting");

    let started = std::time::Instant::now();
    let mut loader = ClipLoader::new(configuration.playback.cache_budget_bytes);
    let mut pixels = crate::pixel_output::PixelOutputs::default();
    let cid = crate::pixel_output::instance_cid(configuration.instance_id.as_str());
    while shutdown.reason().is_none() {
        let now = Timestamp::from_micros(started.elapsed().as_micros() as u64);
        let state = shared.state.load();
        let catalog = shared.catalog.load();
        // Read once per pass, so an accepted edit reaches every output on the same frame.
        let live = shared.configuration.load();
        let heard = shared.analysis.load();
        let mut reports = Vec::new();

        for output in &mut hosted {
            // Each output keeps its own clock, so two outputs at different refresh rates present
            // at their own rates rather than at the slower one's.
            if !output.renderer.should_present(now) {
                continue;
            }
            let Some(state) = state.output(output.configuration.id) else {
                continue;
            };

            let prepared = output.pipeline.prepare(
                state,
                FrameContext {
                    catalog: &catalog,
                    configuration: &live,
                    analysis: &heard.analysis,
                    now_unix_millis: unix_millis(),
                    beat: heard.beat,
                    bpm: heard.bpm,
                    beat_phase: heard.beat_phase,
                    seconds: started.elapsed().as_secs_f32(),
                    now,
                },
                &mut loader,
            );
            reports.extend(
                prepared
                    .statuses
                    .iter()
                    .map(|(layer, status)| (state.id, *layer, *status)),
            );

            let draws: Vec<LayerDraw<'_>> = output.pipeline.draws(state, &prepared);
            let mask = prepared
                .master_mask
                .and_then(|slot| output.pipeline.texture(slot));
            let region = shown_region(&output.configuration);
            output
                .renderer
                .present(&draws, &state.master, mask, now, region);
            // Pixel mapping is output rather than a preview, so it is decided before the
            // preview cadence and does not depend on anyone watching.
            if pixels.wants(&output.configuration, now.as_millis()) {
                let size = output.renderer.size();
                let frame = output.renderer.read_image();
                pixels.send(
                    &output.configuration,
                    media_domain::pixel_map::CanvasImage {
                        width: size.width,
                        height: size.height,
                        rgba: &frame,
                    },
                    now.as_millis(),
                    cid,
                    &shared.universe_inputs,
                );
            }
            let program = shared.previews.for_output(state.id);
            let wanted = program.is_some_and(|preview| preview.wanted())
                || state.layers.iter().enumerate().any(|(layer, _)| {
                    shared
                        .previews
                        .for_layer(state.id, layer)
                        .is_some_and(|preview| preview.wanted())
                });
            if wanted && crate::preview::due(output.last_preview_millis, now.as_millis()) {
                output.last_preview_millis = Some(now.as_millis());
                if let Some(preview) = program.filter(|preview| preview.wanted()) {
                    let from = output.renderer.size();
                    let to = preview.requested_size();
                    let pixels = output.renderer.read_image();
                    preview.publish_pixels(&pixels, from, to, false);
                }
                for (layer_index, layer_state) in state.layers.iter().enumerate() {
                    let Some(preview) = shared
                        .previews
                        .for_layer(state.id, layer_index)
                        .filter(|preview| preview.wanted())
                    else {
                        continue;
                    };
                    let draw = draws
                        .iter()
                        .find(|draw| std::ptr::eq(draw.state, layer_state))
                        .copied();
                    let size = preview.requested_size();
                    if let Some(draw) = draw {
                        let pixels = output.renderer.capture_layer_preview(size, draw, now);
                        preview.publish_pixels(&pixels, size, size, true);
                    } else {
                        preview.publish_pixels(
                            &vec![0; size.width as usize * size.height as usize * 4],
                            size,
                            size,
                            true,
                        );
                    }
                }
            }
        }

        publish(&shared, reports, now);
        // A short sleep rather than a spin: the clocks decide when to present, and burning a core
        // between frames would take it from the decoders.
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    tracing::info!("off-screen outputs stopped");
}

fn open(
    gpu: &Gpu,
    configuration: &OutputConfiguration,
    whole: &MediaConfiguration,
) -> Option<Hosted> {
    let size = Size::new(
        configuration.resolution.width,
        configuration.resolution.height,
    );
    let renderer =
        match OutputRenderer::off_screen(gpu, configuration.id, size, configuration.presentation) {
            Ok(renderer) => renderer,
            Err(error) => {
                tracing::error!(output = %configuration.name, %error, "cannot render this output");
                return None;
            }
        };

    let mut pipeline = LayerPipeline::new(
        gpu,
        configuration.id,
        media_library::LibraryStorage::new(whole.library.root.clone()),
        size,
    );
    pipeline.validate_visualizers();
    tracing::info!(
        output = %configuration.name,
        width = size.width,
        height = size.height,
        presentation = ?configuration.presentation,
        "off-screen output presenting"
    );
    Some(Hosted {
        renderer,
        pipeline,
        configuration: configuration.clone(),
        last_preview_millis: None,
    })
}

fn publish(
    shared: &Shared,
    reports: Vec<(media_domain::OutputId, usize, media_domain::SourceStatus)>,
    now: Timestamp,
) {
    if reports.is_empty() {
        return;
    }
    if let Some(next) = crate::presentation::with_reports(&shared.state.load(), &reports, now) {
        shared.state.store(Arc::new(next));
    }
}

fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

/// The state an off-screen run starts from. Exposed so a caller can assert what it built.
pub fn initial(configuration: &MediaConfiguration) -> MediaState {
    crate::initial_state(configuration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::PresentationMode;

    fn with(targets: Vec<OutputTarget>) -> MediaConfiguration {
        MediaConfiguration {
            outputs: targets
                .into_iter()
                .enumerate()
                .map(|(index, target)| OutputConfiguration {
                    target,
                    ..OutputConfiguration::new(format!("Output {index}"))
                })
                .collect(),
            ..MediaConfiguration::default()
        }
    }

    #[test]
    fn only_a_configuration_with_an_off_screen_output_runs_here() {
        assert!(!any(&with(vec![])));
        assert!(any(&with(vec![OutputTarget::OffScreen])));

        let windowed = with(vec![OutputTarget::Monitor {
            monitor: media_domain::output::MonitorSelector::Index(0),
            fullscreen: false,
        }]);
        assert!(
            !any(&windowed),
            "a windowed output belongs to the event loop"
        );
    }

    #[test]
    fn a_disabled_off_screen_output_does_not_run() {
        let mut configuration = with(vec![OutputTarget::OffScreen]);
        configuration.outputs[0].enabled = false;
        assert!(!any(&configuration));
    }

    #[test]
    fn every_enabled_output_gets_its_own_state_whatever_its_presentation() {
        let mut configuration = with(vec![OutputTarget::OffScreen, OutputTarget::OffScreen]);
        configuration.outputs[0].presentation = PresentationMode::DisplaySynchronized;
        configuration.outputs[1].presentation = PresentationMode::Unlocked;

        let state = initial(&configuration);
        assert_eq!(state.outputs.len(), 2);
        assert_ne!(
            state.outputs[0].id, state.outputs[1].id,
            "two outputs are two outputs, never one shared piece of state"
        );
    }
}
