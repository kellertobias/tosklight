mod display;
mod layout;
mod phases;
mod picture;

use serde::Serialize;
use serde_json::Value;
use std::{
    fs::{OpenOptions, create_dir_all},
    io::Write,
    path::PathBuf,
};
use tauri::Manager;

const REPORT_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_REPORT";
const DURATION_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_DURATION_SECONDS";
const CONTROL_DURATION_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_CONTROL_DURATION_SECONDS";
const PROFILE_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_PROFILE";
const PREPARED_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_PREPARED";
const ADDITIONAL_STAGE_WINDOW_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_ADDITIONAL_STAGE_WINDOW";
const STAGE_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_STAGE";
const FIXTURE_SHEET_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_FIXTURE_SHEET";
const EXPECTED_FIXTURE_RECORDS_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_EXPECTED_FIXTURE_RECORDS";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackagedStageBenchmarkConfig {
    #[serde(flatten)]
    baseline: layout::BaselineOptions,
    duration_seconds: u64,
    control_duration_seconds: u64,
    profile: String,
    additional_stage_window: bool,
    stage_enabled: bool,
    fixture_sheet: bool,
    expected_fixture_records: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_phases: Option<String>,
}

#[tauri::command]
pub(crate) async fn packaged_stage_benchmark_config(
    window: tauri::WebviewWindow,
) -> Result<Option<PackagedStageBenchmarkConfig>, String> {
    if window.label() != "main" {
        return Ok(None);
    }
    if report_path().is_none() {
        return Ok(None);
    }
    let stage_enabled = stage_enabled(std::env::var(STAGE_ENV).ok().as_deref())?;
    let baseline = layout::BaselineOptions::from_env()?;
    let fixed_pixels = baseline.baseline_layout.is_some();
    let baseline_phases = phases::configured(std::env::var(phases::PHASES_ENV).ok().as_deref())?;
    if baseline_phases.is_some() {
        if !stage_enabled {
            return Err("Baseline phases require Stage enabled".into());
        }
        phases::validate(
            window.label(),
            report_path().as_deref(),
            baseline.baseline_layout.as_deref(),
            baseline_phases.as_deref(),
            &std::env::var(PROFILE_ENV).unwrap_or_default(),
            std::env::var("LIGHT_PACKAGED_STAGE_DYNAMICS")
                .ok()
                .as_deref()
                == Some("1"),
            std::env::var(DURATION_ENV)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(30),
        )?;
    }
    display::place(&window, fixed_pixels).await?;
    window.show().map_err(|error| error.to_string())?;
    if !fixed_pixels {
        window.set_focus().map_err(|error| error.to_string())?;
    }
    Ok(Some(PackagedStageBenchmarkConfig {
        baseline,
        baseline_phases,
        duration_seconds: std::env::var(DURATION_ENV)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(30),
        control_duration_seconds: std::env::var(CONTROL_DURATION_ENV)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(30),
        profile: std::env::var(PROFILE_ENV).unwrap_or_else(|_| "default-stage".to_string()),
        additional_stage_window: stage_enabled
            && !fixed_pixels
            && env_enabled_by_default(ADDITIONAL_STAGE_WINDOW_ENV),
        stage_enabled,
        fixture_sheet: !fixed_pixels && env_enabled_by_default(FIXTURE_SHEET_ENV),
        expected_fixture_records: std::env::var(EXPECTED_FIXTURE_RECORDS_ENV)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0),
    }))
}

fn stage_enabled(value: Option<&str>) -> Result<bool, String> {
    match value {
        None | Some("1") => Ok(true),
        Some("0") => Ok(false),
        Some(_) => Err(format!("{STAGE_ENV} must be 0 or 1")),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stage_mode_defaults_enabled_and_rejects_unknown_values() {
        assert_eq!(super::stage_enabled(None), Ok(true));
        assert_eq!(super::stage_enabled(Some("1")), Ok(true));
        assert_eq!(super::stage_enabled(Some("0")), Ok(false));
        for value in ["", "false", "yes", "2"] {
            assert!(super::stage_enabled(Some(value)).is_err());
        }
    }
}

fn env_enabled_by_default(name: &str) -> bool {
    std::env::var(name).map_or(true, |value| value != "0")
}

#[tauri::command]
pub(crate) fn packaged_stage_benchmark_prepared() -> bool {
    report_path().is_some_and(|_| {
        std::env::var_os(PREPARED_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .is_some_and(|path| path.is_file())
    })
}

#[tauri::command]
pub(crate) async fn focus_packaged_stage_benchmark_window(
    app: tauri::AppHandle,
) -> Result<(), String> {
    report_path().ok_or_else(|| format!("{REPORT_ENV} is not configured"))?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "packaged Stage benchmark window is unavailable".to_string())?;
    window.show().map_err(|error| error.to_string())?;
    for _ in 0..8 {
        window.set_focus().map_err(|error| error.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        if window.is_focused().map_err(|error| error.to_string())? {
            return Ok(());
        }
    }
    Err("packaged Stage benchmark main window did not regain focus".to_string())
}

#[tauri::command]
pub(crate) fn append_packaged_stage_benchmark_sample(sample: Value) -> Result<(), String> {
    let path = report_path().ok_or_else(|| format!("{REPORT_ENV} is not configured"))?;
    let parent = path
        .parent()
        .ok_or_else(|| "packaged Stage report path has no parent".to_string())?;
    create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(&mut output, &sample).map_err(|error| error.to_string())?;
    output.write_all(b"\n").map_err(|error| error.to_string())
}

fn report_path() -> Option<PathBuf> {
    std::env::var_os(REPORT_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// Only the private fixed baseline may request a photorealistic Control test pane.
pub(crate) fn baseline_picture(message: &mut viz_helper::protocol::ToHelper) -> Result<(), String> {
    if report_path().is_some() && std::env::var_os(layout::LAYOUT_ENV).is_some() {
        picture::apply(message, &layout::BaselineOptions::from_env()?);
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn exercise_packaged_stage_benchmark_visibility(
    window: tauri::WebviewWindow,
) -> Result<phases::VisibilityEvidence, String> {
    phases::exercise(&window).await
}
