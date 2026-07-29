use serde::Serialize;
use serde_json::Value;
use std::{
    fs::{OpenOptions, create_dir_all},
    io::Write,
    path::PathBuf,
};

const REPORT_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_REPORT";
const DURATION_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_DURATION_SECONDS";
const PROFILE_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_PROFILE";
const PREPARED_ENV: &str = "LIGHT_STAGE_PACKAGED_BENCH_PREPARED";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackagedStageBenchmarkConfig {
    duration_seconds: u64,
    profile: String,
}

#[tauri::command]
pub(crate) fn packaged_stage_benchmark_config(
    window: tauri::WebviewWindow,
) -> Option<PackagedStageBenchmarkConfig> {
    if window.label() != "main" {
        return None;
    }
    report_path()?;
    let _ = window.show();
    let _ = window.set_focus();
    Some(PackagedStageBenchmarkConfig {
        duration_seconds: std::env::var(DURATION_ENV)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(30),
        profile: std::env::var(PROFILE_ENV).unwrap_or_else(|_| "default-stage".to_string()),
    })
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
