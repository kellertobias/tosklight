//! A separate native Ultra job, consuming only invocation-time frozen state.
use super::*;
pub(super) fn run(job: &Job) -> Result<RenderedImage, ImageError> {
    job.check_cancel()?;
    if let Some(notice) = job.frozen.media_notice() {
        job.notice(notice);
    }
    let workspace = storage::Workspace::new(job)?;
    native::render(job, &workspace)
}
