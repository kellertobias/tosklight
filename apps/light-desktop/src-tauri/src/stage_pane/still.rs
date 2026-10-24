//! Pane-scoped capture intent and pushed job observations; never a live renderer quality change.
use super::*;
use tauri::Emitter;
use viz_helper::still::{Engine, Phase, Status, StillJobId};

#[derive(Default)]
pub(super) struct CaptureState {
    sink: Option<(tauri::WebviewWindow, String)>,
    request: Option<String>,
    latest: Option<Status>,
}
impl CaptureState {
    fn pending(&self) -> bool {
        self.request.is_some() && self.latest.as_ref().is_none_or(|s| !s.phase.is_terminal())
    }
}

impl StagePanes {
    /// Serialize admission across all native panes. Never hold the registry while touching a
    /// pane's capture/renderer locks: close and tick already own their own lock ordering.
    fn with_capture_start<T>(
        &self,
        start: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _admission = self
            .capture_start_gate
            .lock()
            .map_err(|_| "capture admission")?;
        let panes: Vec<_> = self
            .panes
            .lock()
            .map_err(|_| "the Stage pane registry")?
            .values()
            .map(|registered| Arc::clone(&registered.pane))
            .collect();
        for pane in panes {
            if pane.is_drawing() && pane.capture.lock().map_err(|_| "capture state")?.pending() {
                return Err("Another Stage image job is active; finish or cancel it before rendering another 4K image".into());
            }
        }
        start()
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureEvent {
    pane_id: String,
    request_id: String,
    status: Option<Status>,
    error: Option<String>,
}
impl StagePane {
    pub(super) fn register_capture(
        &self,
        window: tauri::WebviewWindow,
        pane_id: String,
    ) -> Result<(), String> {
        self.capture.lock().map_err(|_| "capture state")?.sink = Some((window, pane_id));
        Ok(())
    }
    fn capture_start(&self, request_id: String, pane_id: String) -> Result<(), String> {
        if request_id.is_empty()
            || request_id.len() > 128
            || pane_id.is_empty()
            || pane_id.len() > 128
        {
            return Err("Invalid capture request identity".into());
        }
        let mut inner = self.inner.lock().map_err(|_| "the Stage pane")?;
        let running = inner.as_mut().ok_or("Stage renderer is not running")?;
        // Close owns this same renderer lock before marking the pane non-drawing and taking
        // its writer. A start racing with close must fail, rather than queue an orphan request.
        if !self.is_drawing() || running.to_helper.is_none() {
            return Err("Stage renderer is closing or its capture channel is unavailable".into());
        }
        if running.identity.protocol.1 < 2 {
            return Err("Rebuild/reopen the renderer to enable 4K capture".into());
        }
        let mut capture = self.capture.lock().map_err(|_| "capture state")?;
        if capture.pending() {
            return Err("This Stage already has an image job; finish or cancel it first".into());
        }
        capture.request = Some(request_id.clone());
        capture.latest = None;
        running.send(&ToHelper::CaptureStart {
            request_id,
            pane_id,
        });
        if let Some(error) = running.channel_error.as_ref() {
            capture.request = None;
            return Err(error.clone());
        }
        Ok(())
    }
    fn capture_control(&self, job_id: StillJobId, engine: Option<Engine>) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "the Stage pane")?;
        let running = inner.as_mut().ok_or("Stage renderer is not running")?;
        let capture = self.capture.lock().map_err(|_| "capture state")?;
        let latest = capture
            .latest
            .as_ref()
            .ok_or("The capture has not acknowledged its frozen state yet")?;
        if latest.job_id != job_id || latest.phase.is_terminal() {
            return Err("That capture job is no longer active".into());
        }
        if engine.is_some() && !matches!(latest.phase, Phase::NeedsChoice { .. }) {
            return Err("This job is not awaiting a quality choice".into());
        }
        running.send(&match engine {
            Some(engine) => ToHelper::CaptureChoose { job_id, engine },
            None => ToHelper::CaptureCancel { job_id },
        });
        running.channel_error.clone().map_or(Ok(()), Err)
    }
    pub(super) fn accept_still(&self, status: Status) -> Result<(), String> {
        let mut capture = self.capture.lock().map_err(|_| "capture state")?;
        let Some((window, pane_id)) = capture.sink.as_ref() else {
            return Ok(());
        };
        if !accept_status(
            capture.request.as_deref(),
            pane_id,
            capture.latest.as_ref(),
            &status,
        )? {
            return Ok(());
        }
        let event = CaptureEvent {
            pane_id: pane_id.clone(),
            request_id: capture.request.clone().unwrap_or_default(),
            status: Some(status.clone()),
            error: None,
        };
        window
            .emit("stage-still", event)
            .map_err(|e| e.to_string())?;
        capture.latest = Some(status);
        Ok(())
    }
    pub(super) fn capture_interrupted(&self, detail: &str) {
        let Ok(mut capture) = self.capture.lock() else {
            return;
        };
        let Some(request_id) = capture.request.clone() else {
            return;
        };
        if capture
            .latest
            .as_ref()
            .is_some_and(|s| s.phase.is_terminal())
        {
            return;
        }
        if let Some((window, pane_id)) = capture.sink.as_ref() {
            let _ = window.emit(
                "stage-still",
                CaptureEvent {
                    pane_id: pane_id.clone(),
                    request_id,
                    status: None,
                    error: Some(format!("Capture interrupted: {detail}")),
                },
            );
        }
        capture.request = None;
        capture.latest = None;
    }
}

fn owned(
    panes: &StagePanes,
    window: &tauri::WebviewWindow,
    pane_id: &str,
) -> Result<Arc<StagePane>, String> {
    panes
        .pane(&format!("{}:{pane_id}", window.label()))?
        .ok_or_else(|| "Stage pane is unavailable".into())
}
#[tauri::command]
pub(crate) fn start_stage_pane_capture(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
    request_id: String,
) -> Result<(), String> {
    let pane = owned(&panes, &window, &pane_id)?;
    panes.with_capture_start(|| pane.capture_start(request_id, pane_id))
}
#[tauri::command]
pub(crate) fn choose_stage_pane_capture(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
    job_id: StillJobId,
    engine: Engine,
) -> Result<(), String> {
    owned(&panes, &window, &pane_id)?.capture_control(job_id, Some(engine))
}
#[tauri::command]
pub(crate) fn cancel_stage_pane_capture(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
    job_id: StillJobId,
) -> Result<(), String> {
    owned(&panes, &window, &pane_id)?.capture_control(job_id, None)
}

#[tauri::command]
pub(crate) async fn open_stage_pane_capture(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
    reveal: bool,
) -> Result<(), String> {
    let pane = owned(&panes, &window, &pane_id)?;
    let path = {
        let capture = pane.capture.lock().map_err(|_| "capture state")?;
        match capture.latest.as_ref().map(|s| &s.phase) {
            Some(Phase::Saved { path }) => path.clone(),
            _ => return Err("No saved image is available for this pane".into()),
        }
    };
    let path = path
        .canonicalize()
        .map_err(|e| format!("Saved image is unavailable: {e}"))?;
    if !path.is_file()
        || !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        return Err("Saved capture is not a PNG image".into());
    }
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = tokio::process::Command::new("open");
        if reveal {
            c.arg("-R");
        }
        c.arg(&path);
        c
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = tokio::process::Command::new("explorer.exe");
        if reveal {
            c.arg(format!("/select,{}", path.display()));
        } else {
            c.arg(&path);
        }
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut c = tokio::process::Command::new("xdg-open");
        c.arg(if reveal {
            path.parent().unwrap_or(&path)
        } else {
            &path
        });
        c
    };
    let status = command
        .status()
        .await
        .map_err(|e| format!("Cannot open saved image: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("The system could not open the saved image".into())
    }
}

/// Only an acknowledged request may establish a job; later observations retain its frozen identity.
fn accept_status(
    request: Option<&str>,
    pane_id: &str,
    previous: Option<&Status>,
    status: &Status,
) -> Result<bool, String> {
    let Some(request) = request else {
        return Ok(false);
    };
    if status.request_id.as_deref() != Some(request) {
        return Ok(false);
    }
    if status.pane_id != pane_id
        || status.width != 3840
        || status.height != 2160
        || !status.time_seconds.is_finite()
        || status.time_seconds < 0.0
        || status.job_id.0.is_empty()
        || status.job_id.0.len() > 128
    {
        return Err("Renderer returned invalid capture ownership or dimensions".into());
    }
    if let Some(previous) = previous {
        if previous.job_id != status.job_id || previous.phase.is_terminal() {
            return Ok(false);
        }
        if previous.scene_revision != status.scene_revision
            || previous.values_frame != status.values_frame
            || previous.time_seconds != status.time_seconds
            || previous.follow_preload != status.follow_preload
        {
            return Err("Renderer changed the frozen capture identity during the job".into());
        }
        if previous == status {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
