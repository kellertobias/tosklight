//! Worker-owned Blender image rendering. This is independent of legacy scene-snapshot retention.
//! All entry points that start a process block their calling worker, never a live/UI thread.
mod background_policy;
mod eligibility;
mod image_manifest;
mod optics_manifest;
mod output;
mod probe;
mod process;
#[cfg(test)]
mod tests;

pub use eligibility::UltimateEligibility;
pub use image_manifest::{IMAGE_MANIFEST_FILE, ImageManifest};
pub use optics_manifest::OpticsManifest;
pub use output::{RenderedImage, publish_png};
pub use probe::{BlenderCapabilities, BlenderProbeCache};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UltimateError {
    Cancelled,
    TimedOut,
    Unsupported(Vec<String>),
    Failed(String),
}
impl std::fmt::Display for UltimateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Image rendering cancelled"),
            Self::TimedOut => f.write_str("Blender exceeded the image job deadline; choose Ultra or retry with a longer deadline"),
            Self::Unsupported(reasons) => write!(f, "Ultimate cannot preserve this frozen look: {}. Choose Ultra", reasons.join("; ")),
            Self::Failed(detail) => f.write_str(detail),
        }
    }
}
impl std::error::Error for UltimateError {}
impl From<std::io::Error> for UltimateError {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(error.to_string())
    }
}

#[derive(Clone, Debug)]
pub enum UltimateProgress {
    Validating,
    Rendering(String),
    Writing,
    Saved(PathBuf),
}

pub struct UltimateRequest {
    /// A job-owned, already frozen snapshot folder. It must outlive this call and must not be
    /// inside a rotating snapshot store that another capture can prune while Blender reads it.
    pub frozen_directory: PathBuf,
    /// Desired PNG filename. Existing files are never replaced; a numerical suffix is selected.
    pub destination: PathBuf,
    pub eligibility: UltimateEligibility,
    pub timeout: Duration,
}

/// Render the supplied frozen package, without reading any live provider or changing live quality.
/// Probe/engine choice happens before this call; the caller keeps this same package across choice.
/// Unsupported fidelity is an explicit refusal, not an implicit native/Blender fallback.
pub fn render_ultimate(
    request: &UltimateRequest,
    blender: &BlenderCapabilities,
    cancel: Arc<AtomicBool>,
    mut progress: impl FnMut(UltimateProgress),
) -> Result<RenderedImage, UltimateError> {
    check_cancel(&cancel)?;
    progress(UltimateProgress::Validating);
    request.eligibility.validate(&request.frozen_directory)?;
    blender.verify_identity()?;
    let stage = output::Staging::new(&request.destination, &request.frozen_directory)?;
    std::fs::write(
        stage.directory.join("to_blend.py"),
        crate::blender::IMPORT_SCRIPT,
    )?;
    std::fs::write(
        stage.directory.join("parent_guard.py"),
        include_str!("parent_guard.py"),
    )?;
    let script = stage.directory.join("render_image.py");
    std::fs::write(&script, include_str!("render_image.py"))?;
    for (name, source) in [
        ("image_scene.py", include_str!("image_scene.py")),
        ("image_media.py", include_str!("image_media.py")),
        ("image_optics.py", include_str!("image_optics.py")),
        ("image_fog.py", include_str!("image_fog.py")),
        ("background_policy.py", include_str!("background_policy.py")),
    ] {
        std::fs::write(stage.directory.join(name), source)?;
    }
    let staged_png = stage.directory.join("image.png");
    let policy = background_policy::BackgroundPolicy::discover();
    let mut command = policy.command(&blender.executable);
    command
        .args(["--python-exit-code", "13", "--python"])
        .arg(&script)
        .arg("--")
        .arg(if request.eligibility.image_backend() {
            "image"
        } else {
            "legacy"
        })
        .arg(&request.frozen_directory)
        .arg(&staged_png);
    progress(UltimateProgress::Rendering(format!(
        "Blender {} — preparing frozen 4K image; {}",
        blender.version,
        policy.description()
    )));
    let result = process::run(command, request.timeout, &cancel, |line| {
        progress(UltimateProgress::Rendering(line.to_owned()));
    })?;
    if !result.status.success() {
        return Err(UltimateError::Failed(format!(
            "Blender image render failed: {}",
            result.diagnostic()
        )));
    }
    check_cancel(&cancel)?;
    progress(UltimateProgress::Writing);
    let image = publish_png(&staged_png, &request.destination, &cancel)?;
    progress(UltimateProgress::Saved(image.path.clone()));
    Ok(image)
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), UltimateError> {
    if cancel.load(Ordering::Acquire) {
        Err(UltimateError::Cancelled)
    } else {
        Ok(())
    }
}
