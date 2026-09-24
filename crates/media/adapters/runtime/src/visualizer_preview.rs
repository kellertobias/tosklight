//! Live frames of one configured visualizer, for its editor.
//!
//! The Visualizers page asks for frame after frame while an operator tunes a visualizer. Each is
//! drawn off-screen from the stored parameters and the room as the audio worker last heard it, so
//! an accepted edit shows on the very next frame and no output is touched.
//!
//! The worker keeps one renderer, and within it one layer per previewed address, for as long as
//! it runs: a visualizer's memory of the beat -- the streaks a beat sent, the eased levels, the
//! clocks the music drives -- carries from one frame to the next exactly as it does on an output.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use media_domain::MediaAddress;
use media_domain::geometry::Size;
use media_http::{OutputPreviewFrame, SnapshotFailure};
use media_render::{Gpu, VisualizerFrame, VisualizerRenderer};

use crate::presentation::Shared;
use crate::shutdown::Shutdown;

type Reply = tokio::sync::oneshot::Sender<Result<OutputPreviewFrame, SnapshotFailure>>;

struct Job {
    address: MediaAddress,
    size: Size,
    reply: Reply,
}

/// Starts the preview worker and returns the API's handle to it.
///
/// The graphics device opens on the first request, so a server nobody tunes a visualizer on pays
/// for an idle thread and nothing else.
pub(crate) fn start(shared: Shared, shutdown: Shutdown) -> media_http::RenderVisualizerPreview {
    let (sender, jobs) = mpsc::channel::<Job>();
    let spawned = std::thread::Builder::new()
        .name("media-visualizer-preview".into())
        .spawn(move || run(&shared, &shutdown, &jobs));
    if let Err(error) = spawned {
        tracing::error!(%error, "the visualizer preview worker could not start");
        return media_http::previews_no_visualizer();
    }
    let sender = std::sync::Mutex::new(sender);
    Arc::new(move |address, width, height| {
        let (reply, answer) = tokio::sync::oneshot::channel();
        let job = Job {
            address,
            size: Size::new(u32::from(width), u32::from(height)),
            reply,
        };
        let queued = sender
            .lock()
            .map_err(|_| ())
            .and_then(|sender| sender.send(job).map_err(|_| ()));
        Box::pin(async move {
            if queued.is_err() {
                return Err(SnapshotFailure::Unavailable(
                    "the visualizer preview worker has stopped".to_owned(),
                ));
            }
            answer.await.unwrap_or_else(|_| {
                Err(SnapshotFailure::Unavailable(
                    "the visualizer preview worker stopped before it answered".to_owned(),
                ))
            })
        })
    })
}

struct Previewer {
    gpu: Gpu,
    renderer: VisualizerRenderer,
    size: Size,
    /// The renderer layer each previewed visualizer draws into, so each keeps its own memory.
    layers: HashMap<MediaAddress, usize>,
    sequence: u64,
}

fn run(shared: &Shared, shutdown: &Shutdown, jobs: &mpsc::Receiver<Job>) {
    let started = Instant::now();
    let mut previewer: Option<Previewer> = None;
    while shutdown.reason().is_none() {
        let job = match jobs.recv_timeout(Duration::from_millis(250)) {
            Ok(job) => job,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if previewer.is_none() {
            match Gpu::off_screen() {
                Ok(gpu) => {
                    previewer = Some(Previewer {
                        renderer: VisualizerRenderer::new(&gpu, job.size),
                        gpu,
                        size: job.size,
                        layers: HashMap::new(),
                        sequence: 0,
                    });
                }
                Err(error) => {
                    tracing::error!(%error, "no graphics device; visualizer previews cannot render");
                    let _ = job.reply.send(Err(SnapshotFailure::Unavailable(
                        "this Media Server has no graphics device for previews".to_owned(),
                    )));
                    continue;
                }
            }
        }
        let previewer = previewer.as_mut().expect("opened immediately above");
        let answer = previewer.render(shared, job.address, job.size, started);
        let _ = job.reply.send(answer);
    }
}

impl Previewer {
    fn render(
        &mut self,
        shared: &Shared,
        address: MediaAddress,
        size: Size,
        started: Instant,
    ) -> Result<OutputPreviewFrame, SnapshotFailure> {
        let configuration = shared.configuration.load();
        let visualizer = configuration.visualizers.resolve(address).ok_or_else(|| {
            SnapshotFailure::Invalid(format!("no visualizer answers at {address}"))
        })?;
        if size != self.size {
            self.renderer.resize(size);
            self.size = size;
        }
        let next = self.layers.len();
        let layer = *self.layers.entry(address).or_insert(next);

        let heard = shared.analysis.load();
        let frame = VisualizerFrame {
            seconds: started.elapsed().as_secs_f32(),
            analysis: &heard.analysis,
            beat: heard.beat,
            bpm: heard.bpm,
            beat_phase: heard.beat_phase,
            instruments: heard.instruments,
        };
        let texture = self
            .renderer
            .render(layer, visualizer.kind, &visualizer.parameters, &frame)
            .map_err(|error| SnapshotFailure::Unavailable(error.to_string()))?;
        let pixels = texture
            .read_rgba8(&self.gpu)
            .map_err(|error| SnapshotFailure::Unavailable(error.to_string()))?;
        // A visualizer's colour is already weighted by its coverage, so its colour channels alone
        // are the picture over black -- which is how the editor shows it.
        let thumbnail = crate::preview::encode(&pixels, size, size)
            .map_err(|error| SnapshotFailure::Unavailable(error.to_string()))?;
        self.sequence += 1;
        Ok(OutputPreviewFrame {
            sequence: self.sequence,
            width: thumbnail.width,
            height: thumbnail.height,
            content_type: "image/jpeg",
            bytes: thumbnail.jpeg,
        })
    }
}
