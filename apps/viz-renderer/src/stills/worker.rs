use super::*;
use std::time::Instant;
use viz_snapshot::ultimate::{
    BlenderCapabilities, ImageManifest, UltimateEligibility, UltimateProgress, UltimateRequest,
    render_ultimate,
};

pub(super) fn run(job: &Job) -> Result<RenderedImage, UltimateError> {
    job.check_cancel()?;
    job.phase(Phase::CheckingBlender);
    if let Some(notice) = job.frozen.media_notice() {
        job.notice(notice);
    }
    let found = viz_snapshot::find_blender(job.options.configured_blender.as_deref());
    let capabilities = if found.is_some() {
        let result = job
            .cache
            .lock()
            .map_err(|_| UltimateError::Failed("Blender probe cache lock failed".into()))?
            .probe(
                job.options.configured_blender.as_deref(),
                job.options.probe_timeout,
                &job.cancel,
            );
        match result {
            Ok(capabilities) => Some(capabilities),
            Err(UltimateError::Cancelled) => return Err(UltimateError::Cancelled),
            Err(error) => {
                job.notice(format!("{error}; rendering with native Ultra instead"));
                None
            }
        }
    } else {
        if job.options.configured_blender.is_some() {
            job.notice("Configured Blender was not found; rendering with native Ultra without changing live quality");
        }
        None
    };
    job.check_cancel()?;
    // Build once from invocation-time state, before offering Ultimate. Packaging later consumes
    // this exact manifest and never re-reads the live scene or a media source.
    let (manifest, mut eligibility) = prepare_image(job, capabilities.is_some());
    job.check_cancel()?;
    if capabilities
        .as_ref()
        .is_some_and(|cap| !supports_image_nodes(&cap.version))
    {
        eligibility.require_unsupported("Ultimate image lighting requires Blender 4.5 or newer");
    }
    let engine = match capabilities.as_ref() {
        Some(capabilities) => wait_choice(job, capabilities, &eligibility)?,
        None => Engine::Ultra,
    };
    job.update(|status| status.engine = Some(engine));
    job.check_cancel()?;
    let workspace = storage::Workspace::new(job)?;
    match engine {
        Engine::Ultra => native::render(job, &workspace),
        Engine::Ultimate => {
            let capabilities = capabilities.ok_or_else(|| {
                UltimateError::Failed("Blender became unavailable; choose Ultra".into())
            })?;
            if !eligibility.is_supported() {
                return Err(UltimateError::Unsupported(eligibility.reasons().to_vec()));
            }
            let manifest = manifest.as_ref().ok_or_else(|| {
                UltimateError::Failed("Frozen image manifest is unavailable".into())
            })?;
            let frozen_directory = storage::package(job, &workspace, manifest)?;
            let request = UltimateRequest {
                frozen_directory,
                destination: workspace.destination.clone(),
                eligibility,
                timeout: job.options.ultimate_timeout,
            };
            render_ultimate(
                &request,
                &capabilities,
                job.cancel.clone(),
                |progress| match progress {
                    UltimateProgress::Validating => job.phase(Phase::Rendering {
                        detail: "Preparing frozen Blender scene".into(),
                    }),
                    UltimateProgress::Rendering(detail) => job.phase(Phase::Rendering { detail }),
                    UltimateProgress::Writing => job.phase(Phase::Writing),
                    UltimateProgress::Saved(_) => {} // Terminal state follows backend cleanup.
                },
            )
        }
    }
}

fn prepare_image(
    job: &Job,
    blender_available: bool,
) -> (Option<ImageManifest>, UltimateEligibility) {
    let legacy =
        || UltimateEligibility::inspect(job.frozen.scene(), job.frozen.values(), job.frozen.view());
    if !blender_available {
        return (None, legacy());
    }
    let prepared = ImageManifest::build(
        job.frozen.scene(),
        job.frozen.values(),
        job.frozen.view(),
        job.frozen.time_seconds(),
        job.frozen.media(),
    )
    .and_then(|manifest| {
        UltimateEligibility::inspect_image(&manifest).map(|eligibility| (manifest, eligibility))
    });
    match prepared {
        Ok((manifest, eligibility)) => (Some(manifest), eligibility),
        Err(error) => {
            // A transfer failure disables Ultimate only. Native Ultra still consumes the same
            // frozen scene, and the operator gets an actionable reason before choosing.
            let mut eligibility = legacy();
            eligibility.require_unsupported(format!(
                "Cannot prepare frozen Blender image: {error}; choose Ultra"
            ));
            (None, eligibility)
        }
    }
}

fn supports_image_nodes(version: &str) -> bool {
    let Some(version) = version
        .split_whitespace()
        .find(|part| part.starts_with(|c: char| c.is_ascii_digit()))
    else {
        return false;
    };
    let mut parts = version.split('.');
    let major = parts.next().and_then(|s| s.parse::<u32>().ok());
    let minor = parts.next().and_then(|s| s.parse::<u32>().ok());
    matches!((major, minor), (Some(major), Some(minor)) if major > 4 || (major == 4 && minor >= 5))
}

pub(super) fn wait_choice(
    job: &Job,
    capabilities: &BlenderCapabilities,
    eligibility: &UltimateEligibility,
) -> Result<Engine, UltimateError> {
    let deadline = Instant::now()
        .checked_add(job.options.choice_timeout)
        .ok_or_else(|| UltimateError::Failed("Invalid quality-choice deadline".into()))?;
    let mut choice = job
        .choice
        .0
        .lock()
        .map_err(|_| UltimateError::Failed("Image quality choice lock failed".into()))?;
    choice.accepting = true;
    choice.ultimate_reasons = eligibility.reasons().to_vec();
    job.phase(Phase::NeedsChoice {
        blender_version: capabilities.version.clone(),
        ultimate_reasons: choice.ultimate_reasons.clone(),
        ultra_explanation: Engine::Ultra.explanation().into(),
        ultimate_explanation: Engine::Ultimate.explanation().into(),
    });
    loop {
        if let Err(error) = job.check_cancel() {
            choice.accepting = false;
            return Err(error);
        }
        if let Some(engine) = choice.selected.take() {
            choice.accepting = false;
            return Ok(engine);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            choice.accepting = false;
            return Err(UltimateError::Failed(
                "Quality choice expired; invoke Render 4K HQ Image again to freeze a new moment"
                    .into(),
            ));
        }
        choice = job
            .choice
            .1
            .wait_timeout(choice, remaining.min(Duration::from_millis(100)))
            .map_err(|_| UltimateError::Failed("Image quality choice lock failed".into()))?
            .0;
    }
}
