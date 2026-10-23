//! Opt-in visibility exercise for the isolated fixed-pixel measurement window only.
use serde::Serialize;
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

pub(super) const PHASES_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_PHASES";
const PHASES: &str = "static-camera-hidden";
static RUNNING: AtomicBool = AtomicBool::new(false);

pub(super) fn configured(value: Option<&str>) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(PHASES) => Ok(Some(PHASES.to_owned())),
        _ => Err(format!("{PHASES_ENV} must be {PHASES}")),
    }
}

pub(super) fn validate(
    label: &str,
    report: Option<&Path>,
    layout: Option<&str>,
    phases: Option<&str>,
    profile: &str,
    dynamics: bool,
    duration: u64,
) -> Result<(), String> {
    if label != "main"
        || !report.is_some_and(Path::is_absolute)
        || !matches!(layout, Some("single" | "dual"))
        || phases != Some(PHASES)
        || profile != "stage-500"
        || dynamics
        || duration < 40
    {
        return Err("Baseline phases require an isolated main report, fixed static Stage-500, explicit opt-in and at least 40 seconds".into());
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VisibilityEvidence {
    hidden_epoch_micros: u64,
    revealed_epoch_micros: u64,
    hidden_verified: bool,
    revealed_verified: bool,
    observations: Vec<HiddenObservations>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HiddenObservations {
    observed_epoch_micros: u64,
    window_visible: bool,
    samples: serde_json::Value,
}

async fn collect_hidden(window: &tauri::WebviewWindow) -> Result<Vec<HiddenObservations>, String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut observations = Vec::new();
    let mut bytes = 0usize;
    loop {
        if window.is_visible().map_err(|error| error.to_string())? {
            return Err("Benchmark became visible during the hidden measurement".into());
        }
        let samples = crate::stage_pane::take_stage_pane_benchmark_samples(
            window.clone(),
            window.state::<crate::stage_pane::StagePanes>(),
        )?;
        let samples = serde_json::to_value(samples).map_err(|error| error.to_string())?;
        bytes += serde_json::to_vec(&samples)
            .map_err(|error| error.to_string())?
            .len();
        if observations.len() >= 120 || bytes > 16 * 1024 * 1024 {
            return Err("Bounded hidden measurement collection overflowed".into());
        }
        observations.push(HiddenObservations {
            observed_epoch_micros: epoch_micros()?,
            window_visible: false,
            samples,
        });
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        tokio::time::sleep((deadline - now).min(std::time::Duration::from_millis(100))).await;
    }
    Ok(observations)
}

struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
    }
}
fn epoch_micros() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_micros() as u64)
        .map_err(|error| error.to_string())
}

async fn settle_visibility(window: &tauri::WebviewWindow, expected: bool) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if window.is_visible().map_err(|error| error.to_string())? == expected {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!("Benchmark visibility did not settle to {expected}"));
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

pub(super) async fn exercise(window: &tauri::WebviewWindow) -> Result<VisibilityEvidence, String> {
    let layout = super::layout::BaselineOptions::from_env()?;
    if !super::stage_enabled(std::env::var(super::STAGE_ENV).ok().as_deref())? {
        return Err("Baseline phases require Stage enabled".into());
    }
    let duration = std::env::var(super::DURATION_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(30);
    validate(
        window.label(),
        super::report_path().as_deref(),
        layout.baseline_layout.as_deref(),
        std::env::var(PHASES_ENV).ok().as_deref(),
        &std::env::var(super::PROFILE_ENV).unwrap_or_default(),
        std::env::var("LIGHT_PACKAGED_STAGE_DYNAMICS")
            .ok()
            .as_deref()
            == Some("1"),
        duration,
    )?;
    if RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("A baseline visibility exercise is already running".into());
    }
    let _guard = Guard;
    window.hide().map_err(|error| error.to_string())?;
    settle_visibility(window, false).await?;
    let hidden = epoch_micros()?;
    super::append_packaged_stage_benchmark_sample(serde_json::json!({
        "kind": "baseline-native-visibility", "phase": "hidden", "epochMicros": hidden,
        "visibilityVerified": true, "retainsPaneAttachments": true,
    }))?;
    // Rust owns the reveal deadline: a hidden WKWebView may throttle JavaScript timers.
    let observations = collect_hidden(window).await;
    super::display::place(window, true).await?;
    window
        .set_focusable(false)
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    settle_visibility(window, true).await?;
    let revealed = epoch_micros()?;
    super::append_packaged_stage_benchmark_sample(serde_json::json!({
        "kind": "baseline-native-visibility", "phase": "revealed", "epochMicros": revealed,
        "visibilityVerified": true, "retainsPaneAttachments": true,
    }))?;
    Ok(VisibilityEvidence {
        hidden_epoch_micros: hidden,
        revealed_epoch_micros: revealed,
        hidden_verified: true,
        revealed_verified: true,
        observations: observations?,
    })
}

#[cfg(test)]
mod tests {
    use super::{configured, validate};
    use std::path::Path;
    #[test]
    fn requires_explicit_opt_in() {
        assert_eq!(configured(None).unwrap(), None);
        assert!(configured(Some("true")).is_err());
        assert!(configured(Some("static-camera-hidden")).unwrap().is_some());
    }
    #[test]
    fn visibility_guard_rejects_live_or_ambiguous_windows() {
        let absolute = std::env::temp_dir().join("baseline-phases-report.jsonl");
        let valid = |label, report, layout, phase, profile, dynamics, duration| {
            validate(label, report, layout, phase, profile, dynamics, duration)
        };
        assert!(
            valid(
                "main",
                Some(absolute.as_path()),
                Some("dual"),
                Some("static-camera-hidden"),
                "stage-500",
                false,
                40
            )
            .is_ok()
        );
        assert!(
            valid(
                "screen-1",
                Some(absolute.as_path()),
                Some("dual"),
                Some("static-camera-hidden"),
                "stage-500",
                false,
                40
            )
            .is_err()
        );
        assert!(
            valid(
                "main",
                Some(Path::new("relative.jsonl")),
                Some("dual"),
                Some("static-camera-hidden"),
                "stage-500",
                false,
                40
            )
            .is_err()
        );
        assert!(
            valid(
                "main",
                Some(absolute.as_path()),
                None,
                Some("static-camera-hidden"),
                "stage-500",
                false,
                40
            )
            .is_err()
        );
        assert!(
            valid(
                "main",
                Some(absolute.as_path()),
                Some("dual"),
                None,
                "stage-500",
                false,
                40
            )
            .is_err()
        );
        assert!(
            valid(
                "main",
                Some(absolute.as_path()),
                Some("dual"),
                Some("static-camera-hidden"),
                "stage-500",
                true,
                40
            )
            .is_err()
        );
        assert!(
            valid(
                "main",
                Some(absolute.as_path()),
                Some("dual"),
                Some("static-camera-hidden"),
                "canonical-demo",
                false,
                40
            )
            .is_err()
        );
        assert!(
            valid(
                "main",
                Some(absolute.as_path()),
                Some("dual"),
                Some("static-camera-hidden"),
                "stage-500",
                false,
                39
            )
            .is_err()
        );
    }
}
