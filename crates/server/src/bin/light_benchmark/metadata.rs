use chrono::Utc;
use serde::Serialize;
use std::{fs, process::Command};

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceMetadata {
    pub captured_at_utc: String,
    pub hardware_label: String,
    pub cpu_model: Option<String>,
    pub logical_cpus: usize,
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub rustc_version: Option<String>,
    pub package_version: &'static str,
    pub build_profile: &'static str,
    pub git_revision: Option<String>,
    pub git_dirty: Option<bool>,
}

pub fn capture(hardware_label: Option<&str>) -> ReferenceMetadata {
    let cpu_model = cpu_model();
    ReferenceMetadata {
        captured_at_utc: Utc::now().to_rfc3339(),
        hardware_label: hardware_label
            .map(str::to_owned)
            .or_else(|| cpu_model.clone())
            .unwrap_or_else(|| "unspecified hardware".into()),
        cpu_model,
        logical_cpus: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        operating_system: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        rustc_version: command_output("rustc", &["--version"]),
        package_version: env!("CARGO_PKG_VERSION"),
        build_profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        git_revision: command_output("git", &["rev-parse", "--short", "HEAD"]),
        git_dirty: command_output("git", &["status", "--porcelain"]).map(|value| !value.is_empty()),
    }
}

fn cpu_model() -> Option<String> {
    command_output("sysctl", &["-n", "machdep.cpu.brand_string"]).or_else(linux_cpu_model)
}

fn linux_cpu_model() -> Option<String> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;
    cpuinfo.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        matches!(key.trim(), "model name" | "Hardware")
            .then(|| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
