use super::*;
use std::{fs::OpenOptions, time::Instant};
use viz_snapshot::ultimate::publish_png;

pub(super) fn render(
    job: &Job,
    workspace: &storage::Workspace,
) -> Result<RenderedImage, UltimateError> {
    job.check_cancel()?;
    job.phase(Phase::Rendering { detail: "Native Ultra 3840×2160; this separate GPU job may temporarily reduce live Stage FPS, while desk/output work stays independent".into() });
    let deadline = Instant::now()
        .checked_add(job.options.native_timeout)
        .ok_or_else(|| UltimateError::Failed("Invalid native image deadline".into()))?;
    let mut capture = job.frozen.begin_native().map_err(UltimateError::Failed)?;
    let mut cancel_deadline = None;
    let image = loop {
        if job.cancel.load(Ordering::Acquire) && cancel_deadline.is_none() {
            cancel_deadline = Instant::now().checked_add(job.options.cancel_cleanup_timeout);
            job.phase(Phase::Cancelling { detail: "Native GPU work is already submitted; waiting for its resources to finish before releasing the capture".into() });
        }
        if let Some(image) = capture
            .poll()
            .map_err(|error| UltimateError::Failed(error.to_string()))?
        {
            break image;
        }
        let now = Instant::now();
        if now >= deadline || cancel_deadline.is_some_and(|deadline| now >= deadline) {
            job.phase(Phase::Cancelling { detail: "Native readback deadline reached; releasing the worker-owned GPU resources before another capture can start".into() });
            return Err(if job.cancel.load(Ordering::Acquire) {
                UltimateError::Cancelled
            } else {
                UltimateError::Failed(
                    "Native 4K image readback exceeded its deadline; retry after GPU cleanup"
                        .into(),
                )
            });
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let stats = capture.stats();
    if (image.width, image.height) != (STILL_WIDTH, STILL_HEIGHT)
        || stats.degraded
        || stats.render_scale != 1.0
    {
        return Err(UltimateError::Failed("Native capture did not produce undegraded full-resolution 3840×2160 output; no image was published".into()));
    }
    // Keep the device on this worker through completion/destruction. A final Queue drop can
    // otherwise wait implicitly; it must never move onto the live/UI thread.
    drop(capture);
    job.check_cancel()?;
    job.phase(Phase::Writing);
    let path = workspace.directory.join("native.png");
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let mut encoder = ::png::Encoder::new(file, image.width, image.height);
    encoder.set_color(::png::ColorType::Rgba);
    encoder.set_depth(::png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| UltimateError::Failed(format!("Cannot write PNG header: {e}")))?;
    writer
        .write_image_data(&image.rgba)
        .map_err(|e| UltimateError::Failed(format!("Cannot encode PNG: {e}")))?;
    writer
        .finish()
        .map_err(|e| UltimateError::Failed(format!("Cannot finish PNG: {e}")))?;
    job.check_cancel()?;
    publish_png(&path, &workspace.destination, &job.cancel)
}
