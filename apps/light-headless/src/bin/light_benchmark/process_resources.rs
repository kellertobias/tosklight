use crate::light_benchmark::report::{MeasurementResourceReport, ProcessResourceReport};
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::time::Instant;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct MeasurementSampler {
    #[cfg(target_os = "linux")]
    state: Result<LinuxMeasurementState, String>,
}

impl MeasurementSampler {
    pub(super) fn start() -> Self {
        #[cfg(target_os = "linux")]
        {
            let state = LinuxMeasurementState::start();
            return Self { state };
        }
        #[allow(unreachable_code)]
        Self {}
    }

    pub(super) fn sample_if_due(&mut self) {
        #[cfg(target_os = "linux")]
        if let Ok(state) = &mut self.state {
            state.sample_if_due();
        }
    }

    pub(super) fn finish(self) -> MeasurementResourceReport {
        #[cfg(target_os = "linux")]
        {
            return match self.state {
                Ok(mut state) => state.finish(),
                Err(reason) => unavailable_measurement(reason),
            };
        }
        #[allow(unreachable_code)]
        unavailable_measurement("timed CPU and RAM sampling is currently available on Linux".into())
    }
}

#[cfg(target_os = "linux")]
struct LinuxMeasurementState {
    ticks_per_second: u64,
    previous: LinuxSample,
    next_sample_at: Instant,
    cpu_total: f64,
    cpu_max: f64,
    cpu_samples: u64,
    peak_resident: u64,
    error: Option<String>,
}

#[cfg(target_os = "linux")]
impl LinuxMeasurementState {
    fn start() -> Result<Self, String> {
        let ticks_per_second = clock_ticks_per_second()
            .ok_or_else(|| "getconf CLK_TCK returned no clock rate".to_owned())?;
        let previous = linux_sample()?;
        Ok(Self {
            ticks_per_second,
            next_sample_at: previous.at + SAMPLE_INTERVAL,
            peak_resident: previous.resident_bytes,
            previous,
            cpu_total: 0.0,
            cpu_max: 0.0,
            cpu_samples: 0,
            error: None,
        })
    }

    fn sample_if_due(&mut self) {
        if self.error.is_some() || Instant::now() < self.next_sample_at {
            return;
        }
        let current = match linux_sample() {
            Ok(sample) => sample,
            Err(reason) => {
                self.error = Some(reason);
                return;
            }
        };
        let elapsed = current.at.duration_since(self.previous.at).as_secs_f64();
        if elapsed > 0.0 {
            let cpu = current.cpu_ticks.saturating_sub(self.previous.cpu_ticks) as f64
                / self.ticks_per_second as f64
                / elapsed
                * 100.0;
            self.cpu_total += cpu;
            self.cpu_max = self.cpu_max.max(cpu);
            self.cpu_samples += 1;
        }
        self.peak_resident = self.peak_resident.max(current.resident_bytes);
        self.next_sample_at = current.at + SAMPLE_INTERVAL;
        self.previous = current;
    }

    fn finish(&mut self) -> MeasurementResourceReport {
        self.sample_if_due();
        if let Some(reason) = self.error.take() {
            return unavailable_measurement(reason);
        }
        MeasurementResourceReport {
            application_cpu_average_percent: (self.cpu_samples > 0)
                .then_some(self.cpu_total / self.cpu_samples as f64),
            application_cpu_max_percent: (self.cpu_samples > 0).then_some(self.cpu_max),
            application_peak_resident_bytes: Some(self.peak_resident),
            samples: self.cpu_samples,
            sample_interval_milliseconds: SAMPLE_INTERVAL.as_millis() as u64,
            measurement: "Light benchmark process only, sampled from /proc during the timed window",
            unavailable_reason: None,
        }
    }
}

#[cfg(target_os = "linux")]
struct LinuxSample {
    at: Instant,
    cpu_ticks: u64,
    resident_bytes: u64,
}

#[cfg(target_os = "linux")]
fn linux_sample() -> Result<LinuxSample, String> {
    let stat = std::fs::read_to_string("/proc/self/stat")
        .map_err(|error| format!("read /proc/self/stat: {error}"))?;
    let fields = stat
        .rsplit_once(") ")
        .ok_or_else(|| "parse /proc/self/stat process name".to_owned())?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let user_ticks = fields
        .get(11)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| "parse /proc/self/stat user ticks".to_owned())?;
    let system_ticks = fields
        .get(12)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| "parse /proc/self/stat system ticks".to_owned())?;
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("read /proc/self/status: {error}"))?;
    Ok(LinuxSample {
        at: Instant::now(),
        cpu_ticks: user_ticks + system_ticks,
        resident_bytes: status_kib(&status, "VmRSS:").unwrap_or(0),
    })
}

#[cfg(target_os = "linux")]
fn clock_ticks_per_second() -> Option<u64> {
    let output = std::process::Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
        .flatten()
}

fn unavailable_measurement(reason: String) -> MeasurementResourceReport {
    MeasurementResourceReport {
        application_cpu_average_percent: None,
        application_cpu_max_percent: None,
        application_peak_resident_bytes: None,
        samples: 0,
        sample_interval_milliseconds: SAMPLE_INTERVAL.as_millis() as u64,
        measurement: "unavailable",
        unavailable_reason: Some(reason),
    }
}

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
