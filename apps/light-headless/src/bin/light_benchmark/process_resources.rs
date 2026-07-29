use crate::light_benchmark::report::ProcessResourceReport;

pub(super) fn capture() -> ProcessResourceReport {
    #[cfg(target_os = "linux")]
    {
        return linux_status();
    }
    #[cfg(target_os = "macos")]
    {
        return macos_ps();
    }
    #[allow(unreachable_code)]
    ProcessResourceReport {
        resident_bytes: None,
        peak_resident_bytes: None,
        measurement: "unavailable",
        unavailable_reason: Some("process RSS sampling is not implemented on this platform".into()),
    }
}

#[cfg(target_os = "linux")]
fn linux_status() -> ProcessResourceReport {
    let status = match std::fs::read_to_string("/proc/self/status") {
        Ok(status) => status,
        Err(error) => return unavailable(format!("read /proc/self/status: {error}")),
    };
    ProcessResourceReport {
        resident_bytes: status_kib(&status, "VmRSS:"),
        peak_resident_bytes: status_kib(&status, "VmHWM:"),
        measurement: "post-run /proc/self/status; outside timed pipeline",
        unavailable_reason: None,
    }
}

#[cfg(target_os = "linux")]
fn status_kib(status: &str, key: &str) -> Option<u64> {
    status
        .lines()
        .find(|line| line.starts_with(key))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u64>().ok())
        .map(|kib| kib * 1_024)
}

#[cfg(target_os = "macos")]
fn macos_ps() -> ProcessResourceReport {
    let pid = std::process::id().to_string();
    let output = match std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(output) => return unavailable(format!("ps exited with {}", output.status)),
        Err(error) => return unavailable(format!("run ps: {error}")),
    };
    let resident_bytes = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok()
        .map(|kib| kib * 1_024);
    ProcessResourceReport {
        resident_bytes,
        peak_resident_bytes: None,
        measurement: "post-run ps resident set; outside timed pipeline",
        unavailable_reason: resident_bytes
            .is_none()
            .then(|| "ps returned no RSS value".into()),
    }
}

fn unavailable(reason: String) -> ProcessResourceReport {
    ProcessResourceReport {
        resident_bytes: None,
        peak_resident_bytes: None,
        measurement: "unavailable",
        unavailable_reason: Some(reason),
    }
}
