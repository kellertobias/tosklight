//! Off-screen snapshots of a supplied DMX state.
//!
//! The desk asks what a stored Cue would look like. This worker answers without touching any live
//! output: it decodes the supplied slots through the same reducer Art-Net and sACN use, builds a
//! fresh pipeline and renderer for the output's resolution on its own graphics device, waits a
//! bounded time for the selected sources to become drawable, and reads back one frame.
//!
//! A fresh pipeline per snapshot is deliberate. A reused one keeps showing its previous clip while
//! a new selection loads, and a snapshot must never show another Cue's picture.

use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use media_domain::geometry::Size;
use media_domain::{
    AddressClass, Command, CommandKind, CommandSource, MediaAddress, MediaState, OutputState,
    SourceStatus, Timestamp,
};
use media_http::{SnapshotFailure, SnapshotImage, SnapshotRequest};
use media_playback::AsyncClipLoader;
use media_render::{Gpu, LayerDraw, OutputRenderer};

use crate::layer_pipeline::{FrameContext, LayerPipeline, Slot};
use crate::presentation::Shared;
use crate::shutdown::Shutdown;

/// How long a snapshot waits for its clips, masks, and generated sources to become drawable.
const LOAD_BUDGET: Duration = Duration::from_secs(4);
/// Frames rendered after everything is drawable, so a clip's first decoded frame and any
/// time-based effect settle before the read-back.
const SETTLE_FRAMES: u32 = 3;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// Clip memory the snapshot worker may hold. Snapshots are rare and small.
const CLIP_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

type Reply = tokio::sync::oneshot::Sender<Result<SnapshotImage, SnapshotFailure>>;

struct Job {
    request: SnapshotRequest,
    reply: Reply,
}

/// Starts the snapshot worker and returns the API's handle to it.
///
/// The worker opens its graphics device on the first request, so a process that is never asked
/// for a snapshot pays nothing but an idle thread.
pub(crate) fn start(shared: Shared, shutdown: Shutdown) -> media_http::RenderSnapshot {
    let (sender, jobs) = mpsc::channel::<Job>();
    let spawned = std::thread::Builder::new()
        .name("media-snapshot".into())
        .spawn(move || run(&shared, &shutdown, &jobs));
    if let Err(error) = spawned {
        tracing::error!(%error, "the snapshot worker could not start");
        return media_http::renders_nothing();
    }
    let sender = std::sync::Mutex::new(sender);
    Arc::new(move |request| {
        let (reply, answer) = tokio::sync::oneshot::channel();
        let queued = sender
            .lock()
            .map_err(|_| ())
            .and_then(|sender| sender.send(Job { request, reply }).map_err(|_| ()));
        Box::pin(async move {
            if queued.is_err() {
                return Err(SnapshotFailure::Unavailable(
                    "the snapshot worker has stopped".to_owned(),
                ));
            }
            answer.await.unwrap_or_else(|_| {
                Err(SnapshotFailure::Unavailable(
                    "the snapshot worker stopped before it answered".to_owned(),
                ))
            })
        })
    })
}

fn run(shared: &Shared, shutdown: &Shutdown, jobs: &mpsc::Receiver<Job>) {
    let mut gpu: Option<Gpu> = None;
    let started = Instant::now();
    while shutdown.reason().is_none() {
        let job = match jobs.recv_timeout(Duration::from_millis(250)) {
            Ok(job) => job,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if gpu.is_none() {
            match Gpu::off_screen() {
                Ok(device) => gpu = Some(device),
                Err(error) => {
                    tracing::error!(%error, "no graphics device; snapshots cannot render");
                    let _ = job.reply.send(Err(SnapshotFailure::Unavailable(
                        "this Media Server has no graphics device for snapshots".to_owned(),
                    )));
                    continue;
                }
            }
        }
        let device = gpu.as_ref().expect("opened immediately above");
        let answer = render(device, shared, &job.request, started);
        if let Err(failure) = &answer {
            tracing::warn!(output = %job.request.output, ?failure, "a snapshot could not be drawn");
        }
        let _ = job.reply.send(answer);
    }
}

/// The state a snapshot draws: the live output's configuration-owned values with the supplied
/// slots applied exactly as a DMX frame would be. A layer snapshot blanks every other layer so
/// nothing else is loaded.
pub(crate) fn snapshot_state(
    live: &OutputState,
    request: &SnapshotRequest,
    at: Timestamp,
) -> Result<OutputState, SnapshotFailure> {
    let frame = media_domain::personality::decode::frame(live.personality, 1, &request.slots)
        .map_err(|error| SnapshotFailure::Invalid(error.to_string()))?;
    let mut fresh = live.clone();
    fresh.ownership = Default::default();
    for layer in &mut fresh.layers {
        layer.source_status = SourceStatus::default();
    }
    let mut state = MediaState::with_outputs(vec![fresh]);
    let applied = media_domain::state::apply(
        &mut state,
        &Command::new(
            CommandKind::SetDmxFrame {
                output: live.id,
                frame: Box::new(frame),
            },
            CommandSource::Recovery,
            at,
        ),
    );
    if !applied.is_accepted() {
        return Err(SnapshotFailure::Invalid(format!(
            "the supplied slots were not accepted ({applied:?})"
        )));
    }
    let mut output = state.outputs.remove(0);
    if let Some(only) = request.layer {
        if only >= output.layers.len() {
            return Err(SnapshotFailure::Invalid(format!(
                "output {} has no layer {}",
                live.id,
                only + 1
            )));
        }
        for (index, layer) in output.layers.iter_mut().enumerate() {
            if index != only {
                layer.address = MediaAddress::new(0, 0);
                layer.dimmer = 0.0;
            }
        }
    }
    Ok(output)
}

/// Whether nothing in scope can draw.
pub(crate) fn nothing_draws(output: &OutputState, layer: Option<usize>) -> bool {
    let visible = |index: usize| {
        output.layers.get(index).is_some_and(|layer| {
            layer.dimmer > 0.0 && !matches!(layer.address.classify(), AddressClass::Blank)
        })
    };
    match layer {
        Some(index) => !visible(index),
        None => output.master.dimmer <= 0.0 || !(0..output.layers.len()).any(visible),
    }
}

/// The largest size inside `bounds` that keeps the output's aspect ratio.
pub(crate) fn fitted(output: Size, bounds: Size) -> Size {
    let output_width = u64::from(output.width.max(1));
    let output_height = u64::from(output.height.max(1));
    let by_width = u64::from(bounds.width) * output_height / output_width;
    if by_width <= u64::from(bounds.height) {
        Size::new(bounds.width.max(1), (by_width as u32).max(1))
    } else {
        let by_height = u64::from(bounds.height) * output_width / output_height;
        Size::new((by_height as u32).max(1), bounds.height.max(1))
    }
}

fn render(
    gpu: &Gpu,
    shared: &Shared,
    request: &SnapshotRequest,
    started: Instant,
) -> Result<SnapshotImage, SnapshotFailure> {
    let clock = || Timestamp::from_micros(started.elapsed().as_micros() as u64);
    let media = shared.state.load();
    let live_output = media
        .output(request.output)
        .ok_or_else(|| SnapshotFailure::Invalid(format!("no output {}", request.output)))?;
    let configuration = shared.configuration.load();
    let output_configuration = configuration
        .output(request.output)
        .ok_or_else(|| SnapshotFailure::Invalid(format!("no output {}", request.output)))?
        .clone();
    let state = snapshot_state(live_output, request, clock())?;
    let mut state = crate::effect_banks::resolve_output(&state, &configuration);
    let empty = nothing_draws(&state, request.layer);

    let size = Size::new(
        output_configuration.resolution.width,
        output_configuration.resolution.height,
    );
    let mut renderer =
        OutputRenderer::off_screen(gpu, request.output, size, output_configuration.presentation)
            .map_err(|error| {
                SnapshotFailure::Unavailable(format!("cannot render this output: {error}"))
            })?;
    let mut pipeline = LayerPipeline::new(
        gpu,
        request.output,
        media_library::LibraryStorage::new(configuration.library.root.clone()),
        size,
    );
    let mut loader = AsyncClipLoader::new(CLIP_BUDGET_BYTES);
    let models = shared.models.resolve(&configuration.models);
    for (slot, detail) in renderer.set_models(&models.geometries) {
        tracing::warn!(slot, %detail, "a 3D model cannot be drawn in a snapshot");
    }

    let catalog = shared.catalog.load();
    let heard = shared.analysis.load();
    let deadline = Instant::now() + LOAD_BUDGET;
    let mut settled = 0;
    let prepared = loop {
        let now = clock();
        let prepared = pipeline.prepare(
            &state,
            FrameContext::heard(
                &catalog,
                &configuration,
                &heard,
                unix_millis(),
                started.elapsed().as_secs_f32(),
                now,
            ),
            &mut loader,
        );
        // The compositor draws only layers whose source reports ready; a live output learns that
        // through the reducer, a snapshot applies it to its own copy.
        for (index, status) in &prepared.statuses {
            if let Some(layer) = state.layers.get_mut(*index) {
                layer.source_status = *status;
            }
        }
        if drawable(&state, request.layer, &prepared) {
            settled += 1;
            if settled > SETTLE_FRAMES {
                break prepared;
            }
        } else if Instant::now() >= deadline {
            return Err(SnapshotFailure::NotReady(
                "the selected media did not finish loading in time".to_owned(),
            ));
        }
        // Rendering while waiting keeps time-based sources and feedback moving exactly as a
        // presented output would.
        let draws: Vec<LayerDraw<'_>> = pipeline.draws(&state, &prepared);
        renderer.present(&draws, &state.master, None, now, None);
        std::thread::sleep(FRAME_INTERVAL);
    };

    let now = clock();
    let draws: Vec<LayerDraw<'_>> = pipeline.draws(&state, &prepared);
    let bounds = Size::new(u32::from(request.width), u32::from(request.height));
    let (rgba, width, height) = match request.layer {
        None => {
            let mask = prepared.master_mask.and_then(|slot| pipeline.texture(slot));
            let region = output_configuration
                .pixel_map
                .regions
                .iter()
                .find(|region| region.enabled);
            renderer.present(&draws, &state.master, mask, now, region);
            let pixels = renderer.read_image();
            let (mut rgba, width, height) =
                crate::preview::straight_rgba(&pixels, renderer.size(), fitted(size, bounds));
            // The Program is what a projector shows: opaque.
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel[3] = 255;
            }
            (rgba, width, height)
        }
        Some(index) => {
            let target = fitted(size, bounds);
            let layer_state = &state.layers[index];
            let pixels = match draws
                .iter()
                .find(|draw| std::ptr::eq(draw.state, layer_state))
                .copied()
            {
                Some(draw) => renderer.capture_layer_preview(target, draw, now),
                None => vec![0; target.width as usize * target.height as usize * 4],
            };
            crate::preview::straight_rgba(&pixels, target, target)
        }
    };
    let png = crate::preview::encode_png(&rgba, width, height)
        .map_err(|error| SnapshotFailure::Unavailable(format!("cannot encode: {error}")))?;
    Ok(SnapshotImage {
        png: Arc::new(png),
        width: width as u16,
        height: height as u16,
        empty,
    })
}

/// Every source in scope that should draw has a texture, or has definitively failed.
fn drawable(
    state: &OutputState,
    layer: Option<usize>,
    prepared: &crate::layer_pipeline::Prepared,
) -> bool {
    let in_scope = |index: usize| layer.is_none_or(|only| only == index);
    state
        .layers
        .iter()
        .enumerate()
        .filter(|(index, layer)| {
            in_scope(*index) && !matches!(layer.address.classify(), AddressClass::Blank)
        })
        .all(|(index, _)| {
            let failed = prepared
                .statuses
                .iter()
                .any(|(layer, status)| *layer == index && status.is_failed());
            failed
                || prepared.layers.iter().any(|prepared| {
                    prepared.index == index && !matches!(prepared.source, Slot::Outgoing(_))
                })
        })
}

fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

#[cfg(test)]
#[path = "snapshot_render_tests.rs"]
mod tests;
