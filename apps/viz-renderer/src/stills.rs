//! One immutable invocation, one worker, one bounded latest observation. Never draws live state.
#[cfg(test)]
mod cancel_tests;
#[cfg(test)]
mod fog_tests;
#[cfg(test)]
mod front_wash_tests;
mod native;
#[cfg(test)]
mod native_tests;
#[cfg(test)]
mod optics_tests;
#[cfg(test)]
mod projector_tests;
mod storage;
#[cfg(test)]
mod tests;
mod worker;

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub use viz_helper::still::{Engine, Phase, Status, StillJobId};
use viz_render::still::{FrozenStill, STILL_HEIGHT, STILL_WIDTH};
use viz_snapshot::image::{ImageError, RenderedImage};
static NEXT_CONTROLLER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct StillOptions {
    pub request_id: Option<String>,
    pub destination: Option<PathBuf>,
    pub native_timeout: Duration,
    pub cancel_cleanup_timeout: Duration,
}
impl Default for StillOptions {
    fn default() -> Self {
        Self {
            request_id: None,
            destination: None,
            native_timeout: Duration::from_secs(120),
            cancel_cleanup_timeout: Duration::from_secs(30),
        }
    }
}
type Latest = Arc<Mutex<Option<Status>>>;
struct Worker {
    id: StillJobId,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

pub struct StillJobs {
    worker: Option<Worker>,
    latest: Latest,
    session: String,
    sequence: u64,
}
impl Default for StillJobs {
    fn default() -> Self {
        Self {
            worker: None,
            latest: Arc::new(Mutex::new(None)),
            session: format!(
                "{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                NEXT_CONTROLLER.fetch_add(1, Ordering::Relaxed)
            ),
            sequence: 0,
        }
    }
}
impl StillJobs {
    pub fn start(
        &mut self,
        frozen: Arc<FrozenStill>,
        options: StillOptions,
    ) -> Result<StillJobId, String> {
        self.launch(frozen, options, worker::run)
    }
    fn launch(
        &mut self,
        frozen: Arc<FrozenStill>,
        options: StillOptions,
        run: impl FnOnce(&Job) -> Result<RenderedImage, ImageError> + Send + 'static,
    ) -> Result<StillJobId, String> {
        self.reap();
        if self.worker.is_some() {
            return Err("An HQ image job or its GPU cleanup is still active; finish or cancel it before starting another".into());
        }
        validate(&frozen, &options)?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Still job IDs exhausted")?;
        let id = StillJobId(format!("{}-{}", self.session, self.sequence));
        let cancel = Arc::new(AtomicBool::new(false));
        let status = Status { job_id: id.clone(), request_id: options.request_id.clone(),
            pane_id: frozen.origin().pane_id.clone(), follow_preload: frozen.origin().follow_preload,
            scene_revision: frozen.scene().revision, values_frame: frozen.values().frame,
            time_seconds: frozen.time_seconds(), width: STILL_WIDTH, height: STILL_HEIGHT,
            framing: "16:9 framing preserves camera pose and vertical field; side framing may differ from the live pane".into(),
            engine: Some(Engine::Ultra), notice: None, phase: Phase::Queued };
        *self
            .latest
            .lock()
            .map_err(|_| "Still job status lock failed")? = Some(status);
        let job = Job {
            id: id.clone(),
            frozen,
            options,
            latest: self.latest.clone(),
            cancel: cancel.clone(),
        };
        let thread = std::thread::Builder::new().name("hq-image-job".into()).spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&job)))
                .unwrap_or_else(|_| Err(ImageError::Failed("HQ image worker stopped unexpectedly; live quality and show state are unchanged".into())));
            job.finish(outcome);
        }).map_err(|error| {
            if let Ok(mut latest) = self.latest.lock() { if let Some(status) = latest.as_mut() { status.phase = Phase::Failed { detail: format!("Cannot start image worker: {error}") }; } }
            format!("Cannot start image worker: {error}")
        })?;
        self.worker = Some(Worker {
            id: id.clone(),
            cancel,
            thread,
        });
        Ok(id)
    }
    pub fn cancel(&self, id: &StillJobId) -> Result<(), String> {
        let worker = self.matching(id)?;
        let mut latest = self
            .latest
            .lock()
            .map_err(|_| "Still job status lock failed")?;
        let status = latest
            .as_mut()
            .filter(|status| &status.job_id == id)
            .ok_or("Unknown or stale HQ image job")?;
        if status.phase.is_terminal() {
            return Err("This image job has already finished".into());
        }
        worker.cancel.store(true, Ordering::Release);
        status.phase = Phase::Cancelling { detail: "Cancelling; already submitted GPU work must finish cleanup before another image can start".into() };
        Ok(())
    }
    pub fn status(&self) -> Option<Status> {
        self.latest.lock().ok().and_then(|status| status.clone())
    }
    pub fn is_busy(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|worker| !worker.thread.is_finished())
    }
    fn matching(&self, id: &StillJobId) -> Result<&Worker, String> {
        self.worker
            .as_ref()
            .filter(|worker| &worker.id == id)
            .ok_or_else(|| "Unknown or stale HQ image job".into())
    }
    fn reap(&mut self) {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.thread.is_finished())
        {
            if let Some(worker) = self.worker.take() {
                let _ = worker.thread.join();
            }
        }
    }
}
impl Drop for StillJobs {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.cancel.store(true, Ordering::Release);
            // Dropping JoinHandle detaches cleanup; never wait for GPU on the UI thread.
        }
    }
}

struct Job {
    id: StillJobId,
    frozen: Arc<FrozenStill>,
    options: StillOptions,
    latest: Latest,
    cancel: Arc<AtomicBool>,
}
impl Job {
    fn update(&self, apply: impl FnOnce(&mut Status)) {
        if let Ok(mut latest) = self.latest.lock() {
            if let Some(status) = latest.as_mut().filter(|status| status.job_id == self.id) {
                apply(status);
            }
        }
    }
    fn phase(&self, phase: Phase) {
        self.update(|status| {
            if !status.phase.is_terminal()
                && (!self.cancel.load(Ordering::Acquire)
                    || phase.is_terminal()
                    || matches!(phase, Phase::Cancelling { .. }))
            {
                status.phase = phase;
            }
        });
    }
    fn notice(&self, notice: impl Into<String>) {
        let notice: String = notice.into().chars().take(1024).collect();
        self.update(|status| {
            status.notice = Some(match status.notice.as_ref() {
                Some(previous) if previous != &notice => {
                    format!("{previous}; {notice}").chars().take(4096).collect()
                }
                _ => notice,
            })
        });
    }
    fn check_cancel(&self) -> Result<(), ImageError> {
        if self.cancel.load(Ordering::Acquire) {
            Err(ImageError::Cancelled)
        } else {
            Ok(())
        }
    }
    fn finish(&self, outcome: Result<RenderedImage, ImageError>) {
        self.phase(match outcome {
            Ok(image) => Phase::Saved { path: image.path },
            Err(ImageError::Cancelled) => Phase::Cancelled,
            Err(error) => Phase::Failed {
                detail: error.to_string().chars().take(4096).collect(),
            },
        });
    }
}
fn validate(frozen: &FrozenStill, options: &StillOptions) -> Result<(), String> {
    if !frozen.scene().bounds.min.is_finite() || !frozen.scene().bounds.max.is_finite() {
        return Err("No finite scene bounds are available for an HQ image yet".into());
    }
    if options
        .request_id
        .as_ref()
        .is_some_and(|id| id.is_empty() || id.len() > 128)
    {
        return Err("Invalid capture request ID".into());
    }
    if options.native_timeout.is_zero() || options.cancel_cleanup_timeout.is_zero() {
        return Err("Image job deadlines must be finite and positive".into());
    }
    if options.native_timeout > Duration::from_secs(120)
        || options.cancel_cleanup_timeout > Duration::from_secs(30)
    {
        return Err("Native readback/cleanup deadlines cannot exceed 120/30 seconds".into());
    }
    Ok(())
}

#[cfg(test)]
mod people_tests;
